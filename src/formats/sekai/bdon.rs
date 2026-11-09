#![warn(clippy::pedantic)]
//! BDON reader (BanG Dream! Our Notes, `.bdon` or a `.json` told by its content): JSON to a [`Score`] on the 12 lanes of the Project SEKAI
//! model, then to a ROX chart through `score::build`.
//!
//! The board is 24 units wide; `rg_converters` maps it onto 12 lanes (`lane = (left + width / 2 - 12) / 2`, `size = width / 4`), so a note's
//! leftmost lane is `pos / 2` and its width `size / 2` lanes: the `.usc` column rule then applies (a half lane is floored and counted).
//! Ticks are 480 per beat. Taps and flicks are taps; a `long` note is one hold from its first node to its last (the nodes between are
//! dropped and counted, the path across lanes is lost); traces, hidden notes (`visible: false`) and guides are dropped and counted.
//! A gzipped file (`.json.gz`) is refused: gunzip it first.

use std::path::Path;

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, RoxChart};
use serde_json::{Map, Value};

use super::score::{BpmChange, MAX_OBJECTS, Score, ScoreNote, SekaiReport, build, valid_bpm};
use super::usc::{column, flag, number};

/// Decoder of BDON charts: one chart per file.
pub struct BdonDecoder;
crate::impl_format!(BdonDecoder, ["bdon"]);

/// Files above this size are refused (a real chart is a few tens of kilobytes).
const MAX_FILE_SIZE: usize = 16 * 1024 * 1024;
const TICKS_PER_BEAT: f64 = 480.0;
/// The BPM before the first change, and of a change that gives none.
const DEFAULT_BPM: f64 = 120.0;
const SIGNATURE: u8 = 4;
/// Width of a note that gives none, in board units.
const DEFAULT_SIZE: f64 = 6.0;

impl Decoder for BdonDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode(data, "").map(|(chart, _)| chart)
    }
}

impl BdonDecoder {
    /// Decodes a BDON chart; it has no title, `title` becomes the chart's.
    ///
    /// # Errors
    /// Returns an error if the file is too large, gzipped or not a BDON chart, or has no playable note.
    pub fn decode_with_title(data: &[u8], title: &str) -> RoxResult<RoxChart> {
        decode(data, title).map(|(chart, _)| chart)
    }

    /// Decodes a BDON chart and says what was dropped or repaired.
    ///
    /// # Errors
    /// Returns an error if the file is too large, gzipped or not a BDON chart, or has no playable note.
    pub fn decode_with_report(data: &[u8]) -> RoxResult<(RoxChart, SekaiReport)> {
        decode(data, "")
    }

    /// Decodes the BDON chart at `path`; the file stem is the title.
    ///
    /// # Errors
    /// Returns an error if the file can not be read, is too large, gzipped or not a BDON chart, or has no playable note.
    pub fn decode_file(path: &Path) -> RoxResult<RoxChart> {
        Self::decode_with_title(&std::fs::read(path)?, &crate::formats::ssq::decoder::title_of(path))
    }
}

/// Whether `head` (the first bytes of a file, or all of it) looks like a BDON chart: a JSON object with a `"score"` key. A USC, an FNF
/// chart, a V-Slice file or a Bestdori chart has none.
#[must_use]
pub fn looks_like_bdon(head: &[u8]) -> bool {
    let head = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head).trim_ascii_start();
    head.first() == Some(&b'{') && head.windows(b"\"score\"".len()).any(|window| window == b"\"score\"")
}

fn decode(data: &[u8], title: &str) -> RoxResult<(RoxChart, SekaiReport)> {
    let mut report = SekaiReport::default();
    let score = parse(data, title, &mut report)?;
    let chart = build(score, &mut report, "bdon")?;
    chart.validate()?;
    Ok((chart, report))
}

fn invalid(reason: &str) -> RoxError {
    RoxError::InvalidFormat(format!("bdon: {reason}"))
}

fn parse(data: &[u8], title: &str, report: &mut SekaiReport) -> RoxResult<Score> {
    if data.len() > MAX_FILE_SIZE {
        return Err(invalid(&format!("file too large: {} bytes (max {MAX_FILE_SIZE})", data.len())));
    }
    if data.starts_with(&[0x1F, 0x8B]) {
        return Err(invalid("gzipped: gunzip the file first"));
    }
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let root: Value = serde_json::from_slice(data).map_err(|e| invalid(&e.to_string()))?;
    let score = root.get("score").and_then(Value::as_object).ok_or_else(|| invalid("no `score` object"))?;
    let events = score.get("events").and_then(Value::as_object);
    let notes = match score.get("notes") {
        None | Some(Value::Null) => &[][..],
        Some(Value::Array(notes)) => notes.as_slice(),
        Some(_) => return Err(invalid("`score.notes` is not an array")),
    };
    if notes.len() > MAX_OBJECTS {
        return Err(invalid(&format!("more than {MAX_OBJECTS} notes")));
    }
    let metadata = Metadata { title: title.into(), ..Metadata::default() };
    let mut read = Score { metadata, shift_seconds: 0.0, bpm: bpm_changes(events, report), notes: Vec::new() };
    if let Some(events) = events {
        report.skill_markers += events.get("skill").and_then(Value::as_array).map_or(0, Vec::len);
        report.fever_markers += events.get("fever").and_then(Value::as_array).map_or(0, Vec::len);
    }
    for note in notes {
        read_note(note, &mut read.notes, report);
    }
    Ok(read)
}

/// `events.bpm`: `{t, bpm}` by tick; a BPM of 120 at tick 0 when the first change is later (or there is none).
fn bpm_changes(events: Option<&Map<String, Value>>, report: &mut SekaiReport) -> Vec<BpmChange> {
    let mut changes = Vec::new();
    for change in events.and_then(|e| e.get("bpm")).and_then(Value::as_array).into_iter().flatten() {
        let Some(change) = change.as_object() else {
            report.invalid_bpm += 1;
            continue;
        };
        let bpm = number(change, "bpm").unwrap_or(DEFAULT_BPM);
        match tick(change) {
            Some(beat) if valid_bpm(bpm) => changes.push(BpmChange { beat, bpm, signature: SIGNATURE }),
            _ => report.invalid_bpm += 1,
        }
    }
    if changes.iter().all(|change| change.beat > 0.0) {
        changes.push(BpmChange { beat: 0.0, bpm: DEFAULT_BPM, signature: SIGNATURE });
    }
    changes
}

/// The beat of an object's tick `t`: a whole number of ticks, 0 or more.
fn tick(object: &Map<String, Value>) -> Option<f64> {
    #[allow(clippy::cast_precision_loss)] // a tick of a real chart is far below 2^53
    object.get("t").and_then(Value::as_u64).map(|t| t as f64 / TICKS_PER_BEAT)
}

/// The column of a note or node from its left edge and width on the 24-unit board, through the `.usc` rule (centre and half width on
/// the 12 lanes).
fn column_of_board(object: &Map<String, Value>, report: &mut SekaiReport) -> Option<u8> {
    let Some(left) = number(object, "pos") else {
        report.invalid_notes += 1;
        return None;
    };
    let width = number(object, "size").unwrap_or(DEFAULT_SIZE);
    column((left + width / 2.0 - 12.0) * 0.5, width * 0.25, report)
}

fn visible(object: &Map<String, Value>) -> bool {
    object.get("visible").and_then(Value::as_bool) != Some(false)
}

fn read_note(note: &Value, notes: &mut Vec<ScoreNote>, report: &mut SekaiReport) {
    let Some(note) = note.as_object() else {
        report.unsupported_objects += 1;
        return;
    };
    match note.get("type").and_then(Value::as_str).unwrap_or("tap") {
        "tap" | "flick" => single(note, notes, report),
        "trace" => report.traces += 1,
        "long" => long(note, notes, report),
        "guide" => report.guides += 1,
        _ => report.unsupported_objects += 1,
    }
}

/// A tap or a flick (a tap); a hidden one is not judged: dropped.
fn single(note: &Map<String, Value>, notes: &mut Vec<ScoreNote>, report: &mut SekaiReport) {
    if !visible(note) {
        report.hidden_notes += 1;
        return;
    }
    let Some(beat) = tick(note) else {
        report.invalid_notes += 1;
        return;
    };
    let Some(column) = column_of_board(note, report) else { return };
    report.critical_flags += usize::from(flag(note, "crit"));
    report.flicks += usize::from(note.get("type").and_then(Value::as_str) == Some("flick"));
    notes.push(ScoreNote { beat, end_beat: None, column });
}

/// A slide: one hold from its first node to its last, on the first node's column. The note's own `t`, when given, is its first node.
fn long(note: &Map<String, Value>, notes: &mut Vec<ScoreNote>, report: &mut SekaiReport) {
    let listed = note.get("node").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    let own = note.get("t").is_some().then_some(note);
    let chain: Vec<&Map<String, Value>> = own.into_iter().chain(listed.iter().filter_map(Value::as_object)).collect();
    let (Some(first), Some(last)) = (chain.first(), chain.last()) else {
        report.invalid_slides += 1;
        return;
    };
    let (Some(beat), Some(end_beat)) = (tick(first), tick(last)) else {
        report.invalid_slides += 1;
        return;
    };
    if chain.len() < 2 || end_beat < beat {
        report.invalid_slides += 1;
        return;
    }
    let Some(column) = column_of_board(first, report) else { return };
    report.slide_joints_dropped += chain.len() - 2;
    report.critical_flags += usize::from(flag(first, "crit"));
    notes.push(ScoreNote { beat, end_beat: Some(end_beat), column });
}
