#![warn(clippy::pedantic)]
//! Flash Flash Revolution legacy levels (`level_<id>.swf`): a Flash movie that holds the song's MP3 and the chart. Decode only.
//!
//! Format notes: `vsrg-format-docs/vsrg/flash-flash-revolution/swf`, from R^3 (`Beatbox.parseBeatbox`, `ChartFFRLegacy.parseChart`;
//! rCubed, read for facts), checked on the corpus levels.
//!
//! - **Container**: an `FWS` movie or a zlib-compressed `CWS` one (`ZWS`, LZMA, is refused). The chart is the array that a `DoAction`
//!   tag assigns to `_root.beatBox`, run by the small `ActionScript` subset of [`swf`] (constant pool, push, init array, get/set variable
//!   and member, registers), as R^3 does without Flash.
//! - **Notes**: each entry `[frame, direction, colour, ms?]` is a tap on 4 columns, `L D U R`; another direction is dropped and counted.
//!   The colour is the quantization of the note (red 4th, blue 8th…): drawing only.
//! - **Time**: `frame / 30` seconds, or `ms / 1000` when the entry has a fourth element. The per-level `arc_sync` frame offset lives in the
//!   site's playlist, not in the file: it is not applied.
//! - **Tempo**: the file has none, except the `_root.bpmBox` some levels add (`[position, BPM × 1000]`, the first one is used as the chart's
//!   BPM; the unit of its positions is not documented, so the changes are not read). Without one, 120 BPM stands in for the beat lines and
//!   [`FfrReport::tempo_in_file`] says so.
//! - **Metadata**: none in the file (title, artist and stepper are in the playlist); the audio is the movie's own MP3, not read here.

mod swf;

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Note, RoxChart, TimingPoint, drop_stacked};

use swf::Value;

/// Decoder of FFR legacy `.swf` levels.
pub struct SwfDecoder;
crate::impl_format!(SwfDecoder, ["swf"]);

/// Frames per second of the legacy engine.
const FRAME_RATE: f64 = 30.0;
/// The beat lines' tempo of a level that has none.
const STAND_IN_BPM: f32 = 120.0;
const DIRECTIONS: [&str; 4] = ["L", "D", "U", "R"];
/// Times further than this many seconds from the start are not read.
const MAX_SECONDS: f64 = 1e7;

/// What the decoder dropped or changed. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FfrReport {
    /// Whether the level holds a tempo (`_root.bpmBox`); without one the chart's 120 BPM only draws the beat lines.
    pub tempo_in_file: bool,
    /// Entries of another direction than `L D U R`, or without a frame (or a time) that is a number in range: dropped.
    pub invalid_notes: usize,
    /// Notes on another note of their column at the same time: dropped.
    pub overlapping_notes_dropped: usize,
}

impl Decoder for SwfDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode(data).map(|(chart, _)| chart)
    }
}

impl SwfDecoder {
    /// Decodes a level and says what was dropped.
    ///
    /// # Errors
    /// Returns an error if the file is not a readable Flash movie, holds no `_root.beatBox`, or no playable note.
    pub fn decode_with_report(data: &[u8]) -> RoxResult<(RoxChart, FfrReport)> {
        decode(data)
    }
}

/// Whether the start of a file is a Flash movie this reader can open (`FWS` or `CWS`): the extension alone is any Flash movie's.
#[must_use]
pub fn looks_like_swf(head: &[u8]) -> bool {
    head.starts_with(b"FWS") || head.starts_with(b"CWS")
}

fn invalid(reason: impl std::fmt::Display) -> RoxError {
    RoxError::InvalidFormat(format!("swf: {reason}"))
}

fn decode(data: &[u8]) -> RoxResult<(RoxChart, FfrReport)> {
    let body = swf::body(data).map_err(invalid)?;
    let tags = swf::do_actions(&body).map_err(invalid)?;
    // As R^3: the first tag that sets the array wins.
    let beats = tags.iter().find_map(|tag| swf::root_array(tag, "beatBox")).ok_or_else(|| invalid("no _root.beatBox: not an FFR level"))?;
    let bpm = tags.iter().find_map(|tag| swf::root_array(tag, "bpmBox")).and_then(|tempo| first_bpm(&tempo));
    let mut report = FfrReport { tempo_in_file: bpm.is_some(), ..FfrReport::default() };
    let mut notes: Vec<Note> = beats.iter().filter_map(|beat| note_of(beat, &mut report)).collect();
    if notes.is_empty() {
        return Err(invalid("no playable note"));
    }
    notes.sort_by_key(|note| (note.time_us, note.column));
    let mut chart = RoxChart::new(4);
    chart.notes = notes;
    report.overlapping_notes_dropped = drop_stacked(&mut chart.notes, 4, |note| note).total();
    let first = chart.notes.first().map_or(0, |note| note.time_us);
    chart.timing_points.push(TimingPoint::bpm(first.min(0), bpm.unwrap_or(STAND_IN_BPM)));
    chart.validate()?;
    Ok((chart, report))
}

/// The tap of one `[frame, direction, colour, ms?]` entry.
fn note_of(beat: &Value, report: &mut FfrReport) -> Option<Note> {
    let Value::Array(beat) = beat else {
        report.invalid_notes += 1;
        return None;
    };
    let column = beat.get(1).and_then(Value::text).and_then(|direction| DIRECTIONS.iter().position(|known| *known == direction));
    // R^3 truncates the frame to an integer; the time in milliseconds, when there is one, wins.
    let seconds = match beat.get(3) {
        Some(ms) => ms.number().map(|ms| ms / 1000.0),
        None => beat.first().and_then(Value::number).map(|frame| frame.trunc() / FRAME_RATE),
    };
    let (Some(column), Some(seconds)) = (column, seconds.filter(|seconds| seconds.is_finite() && seconds.abs() <= MAX_SECONDS)) else {
        report.invalid_notes += 1;
        return None;
    };
    #[allow(clippy::cast_possible_truncation)] // bounded above; 4 columns
    Some(Note::tap((seconds * 1e6).round() as i64, column as u8))
}

/// The BPM of the first `[position, BPM × 1000]` entry of a `bpmBox`.
fn first_bpm(tempo: &[Value]) -> Option<f32> {
    let Some(Value::Array(first)) = tempo.first() else { return None };
    let bpm = first.get(1)?.number()? / 1000.0;
    #[allow(clippy::cast_possible_truncation)] // a BPM fits f32
    (bpm.is_finite() && bpm > 0.0 && bpm < 1e6).then_some(bpm as f32)
}

#[cfg(test)]
mod tests;
