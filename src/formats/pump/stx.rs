//! Pump It Up `STF4` step files (`.stx`): one file per song, one chart per mode.
//!
//! Format notes: `vsrg-format-docs/vsrg/pump-it-up/stx`, from `UCSConverterTool` (`stx_file.dart`) and STX2DM (read for facts), checked on
//! the corpus files (Prex 3 PC).
//!
//! - **Header** (little-endian): `STF4`, title, artist and step author as 64-byte strings at 60, 124 and 188, then the offsets of the
//!   nine charts at 252: practice, normal, hard, nightmare, crazy, freestyle, half-double, division, lightmap. The lightmap (stage
//!   lights) is not a chart.
//! - **Chart**: an `i32` meter, the division counts of 50 blocks (a 0 ends the chart), then per division a `u32` length and a zlib
//!   stream. Each block is one stretch of the song; Division mode gives a block several alternative divisions, of which the first is
//!   read (the branches are chosen while playing).
//! - **Division**: `f32` BPM, `u32` beats per measure, `u32` lines per beat, `i32` delay (centiseconds), then at 128 the line count
//!   and the 13-byte lines. The first division's delay places the chart; the others' are not applied (as in a converted `.ucs`).
//! - **Lines**: 5 cells (practice, normal, hard, crazy, division) or 10 (nightmare, freestyle, half-double, whose 10 cells hold its 6
//!   panels). 1 a step; 10, 11, 12 a hold's start, body and end; 2 to 6 (groove, wild, A, B, C steps) are steps, counted.

use std::io::Read;

use flate2::read::ZlibDecoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, RoxChart};

use super::{Cell, Rows};

/// Decoder of `.stx` files: several charts per file (see [`StxDecoder::variants`]).
pub struct StxDecoder;
crate::impl_format!(StxDecoder, ["stx"]);

/// Files above this size are refused (a song is a few hundred kilobytes).
const MAX_FILE_SIZE: usize = 64 * 1024 * 1024;
/// A division above this size once inflated is refused (STX2DM's buffer).
const MAX_DIVISION: u64 = 16 * 1024 * 1024;
const HEADER: usize = 288;
const BLOCKS: usize = 50;
const LINE: usize = 13;
/// The modes in file order, with their cells per line. The ninth chart, the lightmap, is not read.
const MODES: [(&str, u8); 8] = [("Practice", 5), ("Normal", 5), ("Hard", 5), ("Nightmare", 10), ("Crazy", 5), ("Freestyle", 10), ("Half-Double", 10), ("Division", 5)];

/// One playable chart of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StxVariant {
    pub index: usize,
    /// The mode (`Normal`, `Crazy`, `Freestyle`…).
    pub mode: &'static str,
    /// The meter written in the file (often 0).
    pub meter: i32,
}

/// What the decoder dropped or changed. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StxReport {
    /// Special steps (codes 2 to 6) read as steps.
    pub special_steps: usize,
    /// Hold parts without their start or end, and unknown codes: dropped.
    pub invalid_notes: usize,
    /// Alternative divisions of Division mode not read (only the first of each block is).
    pub alternative_divisions: usize,
}

fn invalid(reason: impl std::fmt::Display) -> RoxError {
    RoxError::InvalidFormat(format!("stx: {reason}"))
}

fn u32_at(data: &[u8], at: usize) -> RoxResult<u32> {
    let bytes = data.get(at..at + 4).ok_or_else(|| invalid("cut short"))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4 bytes")))
}

fn text_at(data: &[u8], at: usize) -> String {
    let field = &data[at..at + 64];
    let end = field.iter().position(|&byte| byte == 0).unwrap_or(64);
    String::from_utf8_lossy(&field[..end]).trim().to_owned()
}

/// The byte range of chart `index`.
fn chart_range(data: &[u8], index: usize) -> RoxResult<(usize, usize)> {
    if data.len() > MAX_FILE_SIZE || data.len() < HEADER || !data.starts_with(b"STF4") {
        return Err(invalid("not an STF4 file"));
    }
    let start = u32_at(data, 252 + 4 * index)? as usize;
    let end = if index + 1 < 9 { u32_at(data, 252 + 4 * (index + 1))? as usize } else { data.len() };
    if start < HEADER || start > end || end > data.len() {
        return Err(invalid(format!("chart {index} out of the file")));
    }
    Ok((start, end))
}

/// The divisions of a chart, inflated: the first of each block, and how many alternatives were left.
fn divisions(chart: &[u8]) -> RoxResult<(Vec<Vec<u8>>, usize)> {
    let mut at = 4 + 4 * BLOCKS;
    let mut read = Vec::new();
    let mut skipped = 0;
    for block in 0..BLOCKS {
        let count = u32_at(chart, 4 + 4 * block)? as usize;
        if count == 0 {
            break;
        }
        for alternative in 0..count {
            let length = u32_at(chart, at)? as usize;
            let stream = chart.get(at + 4..(at + 4).saturating_add(length)).ok_or_else(|| invalid("division cut short"))?;
            at += 4 + length;
            if alternative > 0 {
                skipped += 1;
                continue;
            }
            let mut inflated = Vec::new();
            ZlibDecoder::new(stream).take(MAX_DIVISION + 1).read_to_end(&mut inflated).map_err(|e| invalid(format!("division: {e}")))?;
            if inflated.len() as u64 > MAX_DIVISION {
                return Err(invalid("division too large"));
            }
            read.push(inflated);
        }
    }
    Ok((read, skipped))
}

fn cell(code: u8, report: &mut StxReport) -> Cell {
    match code {
        1 => Cell::Tap,
        2..=6 => {
            report.special_steps += 1;
            Cell::Tap
        }
        10 => Cell::HoldStart,
        11 => Cell::HoldBody,
        12 => Cell::HoldEnd,
        _ => Cell::Empty,
    }
}

impl StxDecoder {
    /// The playable charts of the file, in file order (the modes without a note are left out).
    ///
    /// # Errors
    /// Returns an error if the file is not an `STF4` file.
    pub fn variants(data: &[u8]) -> RoxResult<Vec<StxVariant>> {
        chart_range(data, 0)?;
        let playable = (0..MODES.len()).filter(|&mode| decode_mode(data, mode).is_ok());
        Ok(playable.enumerate().map(|(index, mode)| {
            let meter = chart_range(data, mode).ok().and_then(|(start, _)| u32_at(data, start).ok()).map_or(0, u32::cast_signed);
            StxVariant { index, mode: MODES[mode].0, meter }
        }).collect())
    }

    /// Decodes playable chart `index` (the numbering of [`Self::variants`]: the `index`-th chart that has a note).
    ///
    /// # Errors
    /// Returns an error if the file is not an `STF4` file or has no such chart.
    pub fn decode_variant(data: &[u8], index: usize) -> RoxResult<RoxChart> {
        Self::decode_with_report(data, index).map(|(chart, _)| chart)
    }

    /// [`Self::decode_variant`] and what was dropped.
    ///
    /// # Errors
    /// Same as [`Self::decode_variant`].
    pub fn decode_with_report(data: &[u8], index: usize) -> RoxResult<(RoxChart, StxReport)> {
        chart_range(data, 0)?;
        let mode = (0..MODES.len()).filter(|&mode| decode_mode(data, mode).is_ok()).nth(index).ok_or_else(|| invalid(format!("no playable chart {index}")))?;
        decode_mode(data, mode)
    }
}

impl rox::codec::Decoder for StxDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        StxDecoder::decode_variant(data, 0)
    }
}

/// The chart of mode `mode` (an index of [`MODES`]).
fn decode_mode(data: &[u8], mode: usize) -> RoxResult<(RoxChart, StxReport)> {
    let (start, end) = chart_range(data, mode)?;
    let (name, cells) = MODES[mode];
    let (divisions, skipped) = divisions(&data[start..end])?;
    let mut report = StxReport { alternative_divisions: skipped, ..StxReport::default() };
    let mut rows: Option<Rows> = None;
    for division in &divisions {
        let bpm = f64::from(f32::from_le_bytes(division.get(..4).ok_or_else(|| invalid("division cut short"))?.try_into().expect("4 bytes")));
        let (beat, split, delay) = (u32_at(division, 4)?, u32_at(division, 8)?, u32_at(division, 12)?.cast_signed());
        if !(bpm.is_finite() && bpm > 0.0) || split == 0 {
            return Err(invalid(format!("division of BPM {bpm} and {split} lines per beat")));
        }
        let beat = u8::try_from(beat).ok().filter(|&beat| beat > 0).unwrap_or(4);
        let count = u32_at(division, 128)? as usize;
        let lines = division.get(132..132 + count.saturating_mul(LINE)).ok_or_else(|| invalid("lines cut short"))?;
        let walk = rows.get_or_insert_with(|| Rows::new(cells, f64::from(delay) / 100.0));
        for line in lines.as_chunks::<LINE>().0 {
            let row: Vec<Cell> = line[..usize::from(cells)].iter().map(|&code| cell(code, &mut report)).collect();
            if !walk.row(row.into_iter(), bpm, f64::from(split), beat) {
                return Err(invalid("time out of range"));
            }
        }
    }
    let (chart, invalid_notes) = rows.map_or((None, 0), Rows::finish);
    report.invalid_notes = invalid_notes;
    let mut chart = chart.ok_or_else(|| invalid(format!("{name}: no note")))?;
    chart.metadata = Metadata {
        title: text_at(data, 60).into(),
        artist: text_at(data, 124).into(),
        creator: text_at(data, 188).into(),
        difficulty_name: name.into(),
        ..Metadata::default()
    };
    chart.validate()?;
    Ok((chart, report))
}
