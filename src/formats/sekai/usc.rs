#![warn(clippy::pedantic)]
//! `.usc` reader: JSON to a [`Score`] (beats, columns), then to a ROX chart through `score::build`. See `mod.rs` for the decisions.

use std::path::Path;

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, RoxChart};
use serde_json::{Map, Value};

use super::score::{
    BpmChange, MAX_OBJECTS, Score, ScoreNote, SekaiReport, build, column_of, valid_bpm,
};

/// Decoder of `.usc` files (Universal Sekai Chart, JSON): one chart per file, bare or wrapped in `{"usc": ..., "version": n}`.
pub struct UscDecoder;
crate::impl_format!(UscDecoder, ["usc"]);

/// Files above this size are refused: a JSON value takes several times its text in memory, and a real USC is a few megabytes at most.
const MAX_FILE_SIZE: usize = 16 * 1024 * 1024;
/// Beats per measure of a timing point (a USC has no measures).
const SIGNATURE: u8 = 4;
/// The leftmost lane of the stage from the centre of the stage: `lane - size + 6` is the column of the note's left edge.
const HALF_STAGE: f64 = 6.0;
/// A position this close to a whole lane is that lane (`rg_converters` snaps within 1e-4, read not copied).
const LANE_TOLERANCE: f64 = 1e-4;

impl Decoder for UscDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode(data, "").map(|(chart, _)| chart)
    }
}

impl UscDecoder {
    /// Decodes a `.usc`; a `.usc` has no title, `title` becomes the chart's.
    ///
    /// # Errors
    /// Returns an error if the file is too large or not a USC, has no valid BPM or no playable note.
    pub fn decode_with_title(data: &[u8], title: &str) -> RoxResult<RoxChart> {
        decode(data, title).map(|(chart, _)| chart)
    }

    /// Decodes a `.usc` and says what was dropped or repaired.
    ///
    /// # Errors
    /// Returns an error if the file is too large or not a USC, has no valid BPM or no playable note.
    pub fn decode_with_report(data: &[u8]) -> RoxResult<(RoxChart, SekaiReport)> {
        decode(data, "")
    }

    /// Decodes the `.usc` at `path`; the file stem is the title.
    ///
    /// # Errors
    /// Returns an error if the file can not be read, is too large or not a USC, has no valid BPM or no playable note.
    pub fn decode_file(path: &Path) -> RoxResult<RoxChart> {
        Self::decode_with_title(
            &std::fs::read(path)?,
            &crate::formats::ssq::decoder::title_of(path),
        )
    }
}

fn decode(data: &[u8], title: &str) -> RoxResult<(RoxChart, SekaiReport)> {
    let mut report = SekaiReport::default();
    let score = parse(data, title, &mut report)?;
    let chart = build(score, &mut report, "usc")?;
    chart.validate()?;
    Ok((chart, report))
}

fn invalid(reason: &str) -> RoxError {
    RoxError::InvalidFormat(format!("usc: {reason}"))
}

fn parse(data: &[u8], title: &str, report: &mut SekaiReport) -> RoxResult<Score> {
    if data.len() > MAX_FILE_SIZE {
        return Err(invalid(&format!(
            "file too large: {} bytes (max {MAX_FILE_SIZE})",
            data.len()
        )));
    }
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let root: Value = serde_json::from_slice(data).map_err(|e| invalid(&e.to_string()))?;
    let root = root
        .as_object()
        .ok_or_else(|| invalid("the root is not an object"))?;
    // The editors' wrapper `{"usc": {...}, "version": 1|2}` (`rg_converters`) or the bare `{offset, objects}` of the engine.
    let chart = match root.get("usc") {
        Some(Value::Object(inner)) => inner,
        _ => root,
    };
    let objects = chart
        .get("objects")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("no `objects` array"))?;
    if objects.len() > MAX_OBJECTS {
        return Err(invalid(&format!("more than {MAX_OBJECTS} objects")));
    }
    let shift = match chart.get("offset") {
        None | Some(Value::Null) => 0.0,
        Some(value) => value
            .as_f64()
            .filter(|offset| offset.is_finite())
            .ok_or_else(|| invalid("`offset` is not a number"))?,
    };

    let mut bpm = Vec::new();
    let mut notes = Vec::new();
    for object in objects {
        let Some(object) = object.as_object() else {
            report.unsupported_objects += 1;
            continue;
        };
        match object.get("type").and_then(Value::as_str) {
            Some("bpm") => match (number(object, "beat"), number(object, "bpm")) {
                (Some(beat), Some(value)) if valid_bpm(value) => bpm.push(BpmChange {
                    beat,
                    bpm: value,
                    signature: SIGNATURE,
                }),
                _ => report.invalid_bpm += 1,
            },
            Some("timeScale" | "timeScaleGroup") => report.time_scale_changes += 1,
            Some("single") => single(object, &mut notes, report),
            Some("slide") => slide(object, &mut notes, report),
            Some("damage") => report.damage_notes += 1,
            Some("guide") => report.guides += 1,
            Some("skill") => report.skill_markers += 1,
            Some("feverChance" | "feverStart") => report.fever_markers += 1,
            _ => report.unsupported_objects += 1,
        }
    }
    Ok(Score {
        metadata: Metadata {
            title: title.into(),
            ..Metadata::default()
        },
        shift_seconds: shift,
        bpm,
        notes,
    })
}

/// A finite number field.
fn number(object: &Map<String, Value>, key: &str) -> Option<f64> {
    object
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
}

/// A boolean field, `false` when absent.
fn flag(object: &Map<String, Value>, key: &str) -> bool {
    object.get(key).and_then(Value::as_bool) == Some(true)
}

/// The column of a note from its centre and half width, as `susToUSC` writes them (`lane = x - 8 + width / 2`, `size = width / 2`,
/// so the left edge is `lane - size + 6` lanes from the left of the stage); `None` (counted) when it is not entirely on the stage.
fn column(lane: f64, size: f64, report: &mut SekaiReport) -> Option<u8> {
    if size <= 0.0 {
        report.zero_width_notes += 1;
        return None;
    }
    let edge = lane - size + HALF_STAGE;
    let whole = edge.round();
    let left = if (edge - whole).abs() < LANE_TOLERANCE {
        whole
    } else {
        report.fractional_lanes += 1;
        edge.floor()
    };
    let width = (size * 2.0).round().max(1.0);
    #[allow(clippy::cast_possible_truncation)] // a float cast saturates; the stage check follows
    let found = column_of(left as i64, width as i64);
    if found.is_none() {
        report.notes_outside_lanes += 1;
    }
    found
}

/// A `single`: a tap, a flick, or a trace (dropped).
fn single(object: &Map<String, Value>, notes: &mut Vec<ScoreNote>, report: &mut SekaiReport) {
    let (Some(beat), Some(lane), Some(size)) = (
        number(object, "beat"),
        number(object, "lane"),
        number(object, "size"),
    ) else {
        report.invalid_notes += 1;
        return;
    };
    if flag(object, "trace") {
        report.traces += 1;
        return;
    }
    let Some(column) = column(lane, size, report) else {
        return;
    };
    if flag(object, "critical") {
        report.critical_flags += 1;
    }
    if object.get("direction").is_some_and(|d| !d.is_null()) {
        report.flicks += 1;
    }
    notes.push(ScoreNote {
        beat,
        end_beat: None,
        column,
    });
}

/// The `type` of a connection.
fn kind(connection: &Value) -> Option<&str> {
    connection.get("type").and_then(Value::as_str)
}

/// A `slide`: a hold from its first connection to its last; `active: false` is a guide (decorative, dropped).
fn slide(object: &Map<String, Value>, notes: &mut Vec<ScoreNote>, report: &mut SekaiReport) {
    if object.get("active").and_then(Value::as_bool) == Some(false) {
        report.guides += 1;
        return;
    }
    let connections = match object.get("connections").and_then(Value::as_array) {
        Some(connections) if connections.len() >= 2 => connections,
        _ => {
            report.invalid_slides += 1;
            return;
        }
    };
    let (first, last) = (&connections[0], &connections[connections.len() - 1]);
    if !matches!(kind(first), Some("start" | "ignore")) {
        report.slide_points_without_start += connections.len();
        return;
    }
    if !matches!(kind(last), Some("end" | "ignore")) {
        report.unterminated_slides += 1;
        return;
    }
    let (Some(beat), Some(lane), Some(size), Some(end_beat)) = (
        first.as_object().and_then(|c| number(c, "beat")),
        first.as_object().and_then(|c| number(c, "lane")),
        first.as_object().and_then(|c| number(c, "size")),
        last.as_object().and_then(|c| number(c, "beat")),
    ) else {
        report.invalid_slides += 1;
        return;
    };
    if end_beat < beat {
        report.invalid_slides += 1;
        return;
    }
    let Some(column) = column(lane, size, report) else {
        return;
    };
    for joint in &connections[1..connections.len() - 1] {
        if matches!(kind(joint), Some("tick" | "hidden" | "attach" | "ignore")) {
            report.slide_joints_dropped += 1;
        } else {
            report.unsupported_notes += 1;
        }
    }
    if flag(object, "critical") {
        report.critical_flags += 1;
    }
    if last.get("direction").is_some_and(|d| !d.is_null()) {
        report.flicks += 1;
    }
    notes.push(ScoreNote {
        beat,
        end_beat: Some(end_beat),
        column,
    });
}
