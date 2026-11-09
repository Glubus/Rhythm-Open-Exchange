#![warn(clippy::pedantic)]
//! DJ Hero charts: `.xmk` (DJ Hero 2) and `.fsgmub` (DJ Hero), the one binary layout of `FreeStyleGames`. Decode only.
//!
//! Format notes: `vsrg-format-docs/vsrg/dj-hero/xmk`, from the `AnyDir` wiki ("XMK and FSGMUB"), checked on the corpus customs.
//!
//! - **Layout** (big-endian): `u32 version` (2), `u32 crc32` (not checked), `u32 entries`, `u32 string length`, then the entries
//!   `{f32 measure, u32 type, f32 length in measures, u32 extra}`, then NUL-separated strings (the charter's name and section names,
//!   pointed at by an entry's `extra` counted from the first entry).
//! - **Time**: positions and lengths are in measures of 4 beats. `0x0B000001` entries set the beat length in microseconds (its `extra`) from
//!   their measure on; the first one also holds before it (a chart without any is refused). `0x0B000002` (a float BPM) is unused by the game.
//! - **Columns**: 5, the turntable as it plays: crossfader left, green, red, blue, crossfader right. Taps (types 0 green, 1 blue, 2 red)
//!   are taps (their nominal 1/32-measure length is ignored); scratches up or down (3, 5 green; 4, 6 blue) are taps on their lane, a
//!   scratch in any direction (7, 8) is a hold over its length; the crossfader moves (11 left, 9 right) are holds on the side column over
//!   their length, a spike (27 green, 28 blue) is a tap there; the centre (10, 23, 29) is where the crossfader rests: no note. Lossy: the
//!   direction of a scratch and the effects dial are lost.
//! - **Dropped and counted**: effectors (12 to 14, 22, 50 and the effect types `0x05FFFFFF`/`0x06xxxxxx`), euphoria (15), sample and
//!   scratch zones (16, 20, 21, 48, 49), freestyle zones and markers (17, 31, 32), battle and megamix events (26, 30), sections, anything
//!   else (the visual markup files hold only those: no playable note, refused).
//! - **Metadata**: title and difficulty from the file name (`<song>_DJ_<Difficulty>.xmk`; the caller gives them), the charter's name
//!   (`0x0AFFFFFF`) as the creator.

use std::path::Path;

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, Note, RoxChart, TimingPoint, drop_stacked};

/// Decoder of DJ Hero `.xmk` / `.fsgmub` charts: one chart per file.
pub struct XmkDecoder;
crate::impl_format!(XmkDecoder, ["xmk", "fsgmub"]);

/// Files above this size are refused (a real chart is a few tens of kilobytes; the game caps a chart at 2500 notes).
const MAX_FILE_SIZE: usize = 16 * 1024 * 1024;
const HEADER: usize = 16;
const ENTRY: usize = 16;
const COLUMNS: u8 = 5;
const BEATS_PER_MEASURE: f64 = 4.0;
/// Times further than this from the start are not read.
const MAX_US: f64 = 1e13;

const BEAT_LENGTH: u32 = 0x0B00_0001;
const BPM_DISPLAY: u32 = 0x0B00_0002;
const CHARTER: u32 = 0x0AFF_FFFF;

/// What the decoder dropped. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XmkReport {
    /// Scratches up or down read as taps (their direction is lost).
    pub directed_scratches: usize,
    /// Effectors and effect-type changes: dropped.
    pub effects: usize,
    /// Euphoria, zones, sections, freestyle markers, battle and megamix events, start markers and the like: dropped.
    pub markers: usize,
    /// Entries of a type the notes do not name: dropped.
    pub unknown_entries: usize,
    /// Notes inside a hold of their column, or on another note of their column at the same time: dropped.
    pub overlapping_notes_dropped: usize,
}

impl Decoder for XmkDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode(data, "", "").map(|(chart, _)| chart)
    }
}

impl XmkDecoder {
    /// Decodes a chart with the title and difficulty the file name gives (the file has neither).
    ///
    /// # Errors
    /// Returns an error if the file is too large, not a DJ Hero chart, has no tempo or no playable note.
    pub fn decode_named(data: &[u8], title: &str, difficulty: &str) -> RoxResult<RoxChart> {
        decode(data, title, difficulty).map(|(chart, _)| chart)
    }

    /// Decodes a chart and says what was dropped.
    ///
    /// # Errors
    /// Returns an error if the file is too large, not a DJ Hero chart, has no tempo or no playable note.
    pub fn decode_with_report(data: &[u8]) -> RoxResult<(RoxChart, XmkReport)> {
        decode(data, "", "")
    }

    /// Decodes the chart at `path`; the title and the difficulty come from the file name (see [`names_of`]).
    ///
    /// # Errors
    /// Returns an error if the file can not be read or does not decode.
    pub fn decode_file(path: &Path) -> RoxResult<RoxChart> {
        let (title, difficulty) = names_of(path);
        Self::decode_named(&std::fs::read(path)?, &title, &difficulty)
    }
}

/// The title and difficulty of a chart from its file name: `CUS_BINTA_DJ_Expert.xmk` is `CUS_BINTA`, `Expert`; a name without `_DJ_`
/// is all title (DJ Hero's `Markup_Main_P1_4.fsgmub` files say their difficulty only by a number).
#[must_use]
pub fn names_of(path: &Path) -> (String, String) {
    let stem = crate::formats::ssq::decoder::title_of(path);
    let upper = stem.to_ascii_uppercase();
    match upper.rfind("_DJ_") {
        Some(at) => (stem[..at].to_owned(), title_case(&stem[at + 4..])),
        None => (stem, String::new()),
    }
}

fn title_case(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect())
}

/// One entry of the file.
#[derive(Clone, Copy)]
struct Entry {
    measure: f64,
    kind: u32,
    length: f64,
    extra: u32,
}

fn invalid(reason: impl std::fmt::Display) -> RoxError {
    RoxError::InvalidFormat(format!("xmk: {reason}"))
}

fn u32_be(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

fn entries(data: &[u8]) -> RoxResult<Vec<Entry>> {
    if data.len() > MAX_FILE_SIZE {
        return Err(invalid(format!("file too large: {} bytes", data.len())));
    }
    let (Some(version), Some(count)) = (u32_be(data, 0), u32_be(data, 8)) else {
        return Err(invalid("shorter than its header"));
    };
    if version != 2 {
        return Err(invalid(format!("version {version}, not 2")));
    }
    let count = usize::try_from(count).unwrap_or(usize::MAX);
    if count.checked_mul(ENTRY).and_then(|size| size.checked_add(HEADER)).is_none_or(|end| end > data.len()) {
        return Err(invalid(format!("{count} entries do not fit in {} bytes", data.len())));
    }
    Ok((0..count)
        .filter_map(|index| {
            let at = HEADER + index * ENTRY;
            let float = |at| u32_be(data, at).map(|bits| f64::from(f32::from_bits(bits)));
            Some(Entry { measure: float(at)?, kind: u32_be(data, at + 4)?, length: float(at + 8)?, extra: u32_be(data, at + 12)? })
        })
        .collect())
}

/// Measures to microseconds through the beat-length entries.
struct Tempo {
    /// `(measure, µs at that measure, µs per beat)`, by measure.
    segments: Vec<(f64, f64, f64)>,
}

impl Tempo {
    fn new(entries: &[Entry]) -> RoxResult<Self> {
        let mut marks: Vec<(f64, f64)> = entries
            .iter()
            .filter(|e| e.kind == BEAT_LENGTH && e.extra > 0 && e.measure.is_finite())
            .map(|e| (e.measure, f64::from(e.extra)))
            .collect();
        marks.sort_by(|a, b| a.0.total_cmp(&b.0));
        let Some(&(first_measure, first_beat)) = marks.first() else {
            return Err(invalid("no beat length entry (0x0B000001)"));
        };
        // The first beat length holds before its measure too, so the chart's measure 0 is time 0.
        let mut segments = vec![(first_measure, first_measure * BEATS_PER_MEASURE * first_beat, first_beat)];
        for &(measure, beat) in &marks[1..] {
            let &(start, start_us, per_beat) = segments.last().unwrap_or(&segments[0]);
            segments.push((measure, start_us + (measure - start) * BEATS_PER_MEASURE * per_beat, beat));
        }
        Ok(Self { segments })
    }

    fn us(&self, measure: f64) -> Option<i64> {
        let at = self.segments.partition_point(|s| s.0 <= measure).saturating_sub(1);
        let (start, start_us, per_beat) = self.segments[at];
        let us = start_us + (measure - start) * BEATS_PER_MEASURE * per_beat;
        #[allow(clippy::cast_possible_truncation)] // bounded just above
        (us.is_finite() && us.abs() <= MAX_US).then(|| us.round() as i64)
    }

    fn timing_points(&self) -> Vec<TimingPoint> {
        let mut points: Vec<TimingPoint> = Vec::new();
        let mut last_beat = None;
        for &(_, start_us, per_beat) in &self.segments {
            if last_beat == Some(per_beat.to_bits()) {
                continue;
            }
            last_beat = Some(per_beat.to_bits());
            #[allow(clippy::cast_possible_truncation)] // a BPM of a real chart fits f32; times are bounded by the notes
            points.push(TimingPoint::bpm(start_us.round() as i64, (60_000_000.0 / per_beat) as f32));
        }
        points
    }
}

/// What an entry is in the chart.
enum Kind {
    Tap(u8),
    Hold(u8),
    Dropped,
}

const CROSSFADE_LEFT: u8 = 0;
const GREEN: u8 = 1;
const RED: u8 = 2;
const BLUE: u8 = 3;
const CROSSFADE_RIGHT: u8 = 4;

fn kind_of(kind: u32, report: &mut XmkReport) -> Kind {
    match kind {
        0 => Kind::Tap(GREEN),
        1 => Kind::Tap(BLUE),
        2 => Kind::Tap(RED),
        3 | 5 => {
            report.directed_scratches += 1;
            Kind::Tap(GREEN)
        }
        4 | 6 => {
            report.directed_scratches += 1;
            Kind::Tap(BLUE)
        }
        7 => Kind::Hold(GREEN),
        8 => Kind::Hold(BLUE),
        11 => Kind::Hold(CROSSFADE_LEFT),
        9 => Kind::Hold(CROSSFADE_RIGHT),
        27 => Kind::Tap(CROSSFADE_LEFT),
        28 => Kind::Tap(CROSSFADE_RIGHT),
        10 | 23 | 29 => Kind::Dropped,
        12..=14 | 22 | 50 | 0x05FF_FFFF | 0x0600_0000..=0x06FF_FFFF => {
            report.effects += 1;
            Kind::Dropped
        }
        15..=21 | 26 | 30..=32 | 44 | 45 | 48 | 49 | 51 | 0x09FF_FFFF | CHARTER | BEAT_LENGTH | BPM_DISPLAY | 0xFFFF_FFFF => {
            report.markers += 1;
            Kind::Dropped
        }
        _ => {
            report.unknown_entries += 1;
            Kind::Dropped
        }
    }
}

/// The NUL-terminated string at `extra` bytes after the first entry.
fn string_at(data: &[u8], extra: u32) -> Option<String> {
    let start = HEADER.checked_add(usize::try_from(extra).ok()?)?;
    let bytes = data.get(start..)?;
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    Some(String::from_utf8_lossy(&bytes[..end]).trim().to_owned()).filter(|s| !s.is_empty())
}

fn decode(data: &[u8], title: &str, difficulty: &str) -> RoxResult<(RoxChart, XmkReport)> {
    let entries = entries(data)?;
    let tempo = Tempo::new(&entries)?;
    let mut report = XmkReport::default();
    let mut chart = RoxChart::new(COLUMNS);
    let creator = entries.iter().find(|e| e.kind == CHARTER).and_then(|e| string_at(data, e.extra)).unwrap_or_default();
    chart.metadata = Metadata { title: title.into(), difficulty_name: difficulty.into(), creator: creator.into(), ..Metadata::default() };
    for entry in &entries {
        let (column, hold) = match kind_of(entry.kind, &mut report) {
            Kind::Tap(column) => (column, false),
            Kind::Hold(column) => (column, true),
            Kind::Dropped => continue,
        };
        let Some(start) = tempo.us(entry.measure) else { continue };
        let end = tempo.us(entry.measure + entry.length.max(0.0)).filter(|&end| hold && end > start);
        chart.notes.push(end.map_or_else(|| Note::tap(start, column), |end| Note::hold(start, end - start, column)));
    }
    report.overlapping_notes_dropped = drop_stacked(&mut chart.notes, COLUMNS, |note| note).total();
    if chart.notes.is_empty() {
        return Err(invalid("no playable note"));
    }
    chart.timing_points = tempo.timing_points();
    if let (Some(TimingPoint::Bpm { time_us, .. }), Some(first)) = (chart.timing_points.first_mut(), chart.notes.first()) {
        *time_us = (*time_us).min(first.time_us);
    }
    chart.validate()?;
    Ok((chart, report))
}

#[cfg(test)]
mod tests;
