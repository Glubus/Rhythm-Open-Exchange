#![warn(clippy::pedantic)]
//! DDR STEP (`.step`): the step data of DDR 2nd Mix to 3rd Mix Plus and the Solo mixes, as "DDR Utility" exports it. Decode only.
//!
//! The format notes (`vsrg-format-docs/vsrg/ddr/step`) were a stub; the layout here is the one `RhythmCodex` reads (MIT, `Charts.Step1`,
//! read for facts, nothing copied), checked on its three fixtures:
//!
//! - **Chunks**: `{i32 length, length - 4 bytes}`, little-endian, until a length below 4. The first chunk is the tempo, every other a chart.
//! - **Tempo**: pairs `{i32 measure tick, i32 time}`: 4096 ticks per measure, time in 1/75 s. The same timeline as an `.ssq` tempo chunk
//!   (`TempoMap`: interpolated between the entries, a stop where two ticks are equal).
//! - **Chart**: an `i32` code (low byte: `0` single or solo, `1` couple, `2` double; next byte: `0` Basic, `1` Another, `2` Maniac), then
//!   rows `{i32 tick, i32 panels}`; a row of `0` or `-1` is empty. Each nibble of `panels` is one panel (bit `k` of the eight).
//! - **Columns**: a single chart is written twice, as both players (both nibbles equal): 4 columns. Otherwise the panels used choose the
//!   layout, the smallest that holds them: the Solo mixes' 3 panels (bits 4, 1, 6: up-left, down, up-right), 4 (left, down, up, right), Solo
//!   6 (left, up-left, down, up, up-right, right: bits 0, 4, 1, 2, 6, 3), or 8 (both pads: double). A chart that uses panels of the second
//!   player (bits 5 or 7) is a two-player chart: a double (code `2`) is 8 columns, a couple (code `1`, each player their own steps) is
//!   skipped, as the `.ssq`, `.dwi` and `.sm` readers skip couples.
//! - No freezes, no mines, no title, no audio (the file stem is the title; the music is in the game's own archives).

use std::path::Path;

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, Note, RoxChart, TimingPoint, drop_stacked};

use super::tempo::TempoMap;

/// Decoder of DDR STEP files: the first playable chart (see [`StepDecoder::variants`] for the others).
pub struct StepDecoder;
crate::impl_format!(StepDecoder, ["step"]);

/// Files above this size are refused, as `RhythmCodex` does (real ones are a few tens of kilobytes).
const MAX_FILE_SIZE: usize = 64 * 1024;
/// Time units of the tempo chunk per second.
const TIME_UNITS: u16 = 75;
/// Times further than this from the start of the audio are not read.
const MAX_US: i64 = 10_000_000 * 1_000_000;

/// A playable chart of a STEP file. `index` counts the playable ones from 0, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepVariant {
    pub index: usize,
    /// `Basic`, `Another`, `Maniac`, or `Edit` for a slot the notes do not name.
    pub difficulty: &'static str,
    /// `Single`, `Solo` (6 panels), `Solo 3` (3 panels) or `Double`.
    pub style: &'static str,
    pub key_count: u8,
    pub notes: usize,
}

impl StepVariant {
    /// What identifies the chart in a message: `Single Maniac`.
    #[must_use]
    pub fn name(&self) -> String {
        format!("{} {}", self.style, self.difficulty)
    }
}

impl Decoder for StepDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        Self::decode_with_title(data, 0, "")
    }
}

impl StepDecoder {
    /// The playable charts of the file, in file order (couples and charts without a note are not).
    ///
    /// # Errors
    /// Returns an error if the file is not a STEP file or its tempo can not be read.
    pub fn variants(data: &[u8]) -> RoxResult<Vec<StepVariant>> {
        Ok(charts(data, "")?
            .into_iter()
            .enumerate()
            .map(|(index, (chart, style))| StepVariant {
                index,
                difficulty: style.difficulty,
                style: style.name,
                key_count: chart.key_count,
                notes: chart.notes.len(),
            })
            .collect())
    }

    /// Decodes playable chart `index` (the numbering of [`Self::variants`]); `title` becomes its title (the format has none).
    ///
    /// # Errors
    /// Returns an error if the file is not a STEP file or has no such playable chart.
    pub fn decode_with_title(data: &[u8], index: usize, title: &str) -> RoxResult<RoxChart> {
        let mut all = charts(data, title)?;
        if index >= all.len() {
            return Err(RoxError::InvalidFormat(format!("step: {} playable charts, not chart {index}", all.len())));
        }
        let (chart, _) = all.swap_remove(index);
        chart.validate()?;
        Ok(chart)
    }

    /// Decodes playable chart `index` of the file at `path`; the file stem is the title.
    ///
    /// # Errors
    /// Returns an error if the file can not be read, is not a STEP file or has no such playable chart.
    pub fn decode_file(path: &Path, index: usize) -> RoxResult<RoxChart> {
        Self::decode_with_title(&std::fs::read(path)?, index, &super::decoder::title_of(path))
    }
}

/// Whether `data` (the whole file) is laid out as a STEP file: a multiple of 4 bytes, chunks whose lengths are positive multiples of 4
/// that stay inside the file, and a tempo chunk of at least two entries whose ticks and times never go back (`RhythmCodex`'s heuristic).
/// A pydance `.step` is text and fails at once.
#[must_use]
pub fn looks_like_step(data: &[u8]) -> bool {
    if data.len() < 4 || data.len() > MAX_FILE_SIZE || !data.len().is_multiple_of(4) {
        return false;
    }
    let Some(chunks) = chunks_of(data) else { return false };
    let Some(tempo) = chunks.first() else { return false };
    let pairs = pairs(tempo);
    pairs.len() >= 2 && pairs.windows(2).all(|w| w[1].0 >= w[0].0 && w[1].1 >= w[0].1)
}

/// The chunks of the file; `None` when a length is not a multiple of 4 or runs past the end.
fn chunks_of(data: &[u8]) -> Option<Vec<&[u8]>> {
    let mut chunks = Vec::new();
    let mut at = 0;
    while let Some(length) = i32_le(data, at) {
        let Ok(length) = usize::try_from(length) else { return None };
        if length < 4 {
            break;
        }
        if !length.is_multiple_of(4) {
            return None;
        }
        chunks.push(data.get(at + 4..at + length)?);
        at += length;
    }
    Some(chunks)
}

fn i32_le(data: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(data.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

/// The `{i32, i32}` pairs of a chunk body.
fn pairs(body: &[u8]) -> Vec<(i32, i32)> {
    body.as_chunks::<8>().0.iter().filter_map(|pair| Some((i32_le(pair, 0)?, i32_le(pair, 4)?))).collect()
}

/// What a chart chunk is: its name and how its panel bits become columns (`columns[bit]`).
#[derive(Clone, Copy)]
struct Style {
    name: &'static str,
    difficulty: &'static str,
    key_count: u8,
    columns: [Option<u8>; 8],
}

/// The layouts a one-player chart may use, smallest first: the panel bit of each column.
const LAYOUTS: [(&str, &[u8]); 4] = [
    ("Solo 3", &[4, 1, 6]),
    ("Single", &[0, 1, 2, 3]),
    ("Solo", &[0, 4, 1, 2, 6, 3]),
    ("Double", &[0, 1, 2, 3, 4, 5, 6, 7]),
];

/// The style of a chart from its code and the panels its rows use; `Err` for a couple chart.
fn style_of(code: i32, rows: &mut [(i32, u8)]) -> Result<Style, &'static str> {
    let difficulty = match (code >> 8) & 0xFF {
        0 => "Basic",
        1 => "Another",
        2 => "Maniac",
        _ => "Edit",
    };
    let make = |name, bits: &[u8]| {
        let mut columns = [None; 8];
        for (column, &bit) in (0u8..).zip(bits) {
            columns[usize::from(bit)] = Some(column);
        }
        #[allow(clippy::cast_possible_truncation)] // at most 8
        Style { name, difficulty, key_count: bits.len() as u8, columns }
    };
    if rows.iter().all(|&(_, panels)| panels & 0x0F == panels >> 4) {
        // A single chart written as both players.
        for (_, panels) in rows.iter_mut() {
            *panels &= 0x0F;
        }
        return Ok(make("Single", &[0, 1, 2, 3]));
    }
    let used = rows.iter().fold(0u8, |used, &(_, panels)| used | panels);
    if used & 0xA0 != 0 && code & 0xFF == 1 {
        return Err("couple chart (each player their own steps): skipped");
    }
    let fits = |bits: &[u8]| (0..8u8).filter(|bit| used & (1 << bit) != 0).all(|bit| bits.contains(&bit));
    let (name, bits) = LAYOUTS.into_iter().find(|(_, bits)| fits(bits)).unwrap_or(LAYOUTS[3]);
    Ok(make(name, bits))
}

/// One bit per panel: a nibble that is not 0 is that panel.
fn panels_of(raw: i32) -> u8 {
    (0..8).fold(0, |panels, nibble| if (raw >> (4 * nibble)) & 0xF != 0 { panels | (1 << nibble) } else { panels })
}

/// The tempo of the file as an `.ssq` tempo chunk body (`N` ticks, then `N` times).
fn tempo_of(chunk: &[u8]) -> RoxResult<TempoMap> {
    let entries = pairs(chunk);
    let body: Vec<u8> = entries
        .iter()
        .map(|&(tick, _)| tick)
        .chain(entries.iter().map(|&(_, time)| time))
        .flat_map(i32::to_le_bytes)
        .collect();
    TempoMap::parse(TIME_UNITS, entries.len(), &body)
}

/// Every playable chart of the file, with its style.
fn charts(data: &[u8], title: &str) -> RoxResult<Vec<(RoxChart, Style)>> {
    if !looks_like_step(data) {
        return Err(RoxError::InvalidFormat("step: not a DDR STEP file (chunk lengths or tempo entries)".into()));
    }
    let chunks = chunks_of(data).unwrap_or_default();
    let tempo = tempo_of(chunks[0])?;
    let points = tempo.bpm_points();
    let mut out = Vec::new();
    for chunk in &chunks[1..] {
        let Some(code) = i32_le(chunk, 0) else { continue };
        let mut rows: Vec<(i32, u8)> =
            pairs(&chunk[4..]).into_iter().filter(|&(_, raw)| raw != 0 && raw != -1).map(|(tick, raw)| (tick, panels_of(raw))).collect();
        let Ok(style) = style_of(code, &mut rows) else { continue };
        let mut chart = RoxChart::new(style.key_count);
        chart.metadata = Metadata { title: title.into(), difficulty_name: style.difficulty.into(), ..Metadata::default() };
        for &(tick, panels) in &rows {
            let time_us = tempo.time_us(i64::from(tick));
            if tick < 0 || time_us.abs() > MAX_US {
                continue;
            }
            for bit in (0..8).filter(|bit| panels & (1 << bit) != 0) {
                if let Some(column) = style.columns[bit] {
                    chart.notes.push(Note::tap(time_us, column));
                }
            }
        }
        drop_stacked(&mut chart.notes, chart.key_count, |note| note);
        let Some(first) = chart.notes.first().map(|note| note.time_us) else { continue };
        chart.timing_points = points
            .iter()
            .enumerate()
            .map(|(index, &(time, bpm))| TimingPoint::bpm(if index == 0 { time.min(first) } else { time }, bpm))
            .collect();
        out.push((chart, style));
    }
    Ok(out)
}
