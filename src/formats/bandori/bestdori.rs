#![warn(clippy::pedantic)]
//! Bestdori chart (JSON array) reader: JSON to a [`Score`] (beats, columns), then to a ROX chart through `score::build`. See `mod.rs` for
//! the decisions.

use std::path::Path;

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, RoxChart};
use serde_json::{Map, Value};

use super::score::{
    BandoriReport, BpmChange, MAX_OBJECTS, Score, ScoreNote, build, column_of, valid_bpm,
};

/// Decoder of Bestdori charts (the JSON array of `BPM`, `Single`, `Long`, `Slide` and `Directional` elements): one chart per file.
pub struct BestdoriDecoder;
crate::impl_format!(BestdoriDecoder, ["bestdori"]);

/// Files above this size are refused: a JSON value takes several times its text in memory, and a real chart is a few hundred kilobytes.
const MAX_FILE_SIZE: usize = 16 * 1024 * 1024;
/// The number of the leftmost lane of a Bestdori chart (lanes are 0 to 6: `BanGConverter` writes `lane + 1` into the simulator's 1 to 7, and the
/// `bangbangboom` editor's exporter documents "from 0 to 6").
const FIRST_LANE: i64 = 0;
/// A lane this close to the head's lane is the same lane, for the count of slides that change lane.
const SAME_LANE: f64 = 1e-4;

impl Decoder for BestdoriDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode(data, "").map(|(chart, _)| chart)
    }
}

impl BestdoriDecoder {
    /// Decodes a Bestdori chart; it has no title, `title` becomes the chart's.
    ///
    /// # Errors
    /// Returns an error if the file is too large or not a Bestdori chart or has no playable note.
    pub fn decode_with_title(data: &[u8], title: &str) -> RoxResult<RoxChart> {
        decode(data, title).map(|(chart, _)| chart)
    }

    /// Decodes a Bestdori chart and says what was dropped or repaired.
    ///
    /// # Errors
    /// Returns an error if the file is too large or not a Bestdori chart or has no playable note.
    pub fn decode_with_report(data: &[u8]) -> RoxResult<(RoxChart, BandoriReport)> {
        decode(data, "")
    }

    /// Decodes the Bestdori chart at `path`; the file stem (without a trailing `.bestdori`) is the title.
    ///
    /// # Errors
    /// Returns an error if the file can not be read, is too large or not a Bestdori chart or has no playable note.
    pub fn decode_file(path: &Path) -> RoxResult<RoxChart> {
        Self::decode_with_title(&std::fs::read(path)?, &super::title_of(path))
    }
}

fn decode(data: &[u8], title: &str) -> RoxResult<(RoxChart, BandoriReport)> {
    let mut report = BandoriReport::default();
    let score = parse(data, title, &mut report)?;
    let chart = build(score, &mut report, "bestdori")?;
    chart.validate()?;
    Ok((chart, report))
}

fn invalid(reason: &str) -> RoxError {
    RoxError::InvalidFormat(format!("bestdori: {reason}"))
}

fn parse(data: &[u8], title: &str, report: &mut BandoriReport) -> RoxResult<Score> {
    if data.len() > MAX_FILE_SIZE {
        return Err(invalid(&format!(
            "file too large: {} bytes (max {MAX_FILE_SIZE})",
            data.len()
        )));
    }
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let root: Value = serde_json::from_slice(data).map_err(|e| invalid(&e.to_string()))?;
    let elements = root
        .as_array()
        .ok_or_else(|| invalid("the root is not an array"))?;
    if elements.len() > MAX_OBJECTS {
        return Err(invalid(&format!("more than {MAX_OBJECTS} elements")));
    }
    let mut bpm = Vec::new();
    let mut notes = Vec::new();
    for element in elements {
        let Some(object) = element.as_object() else {
            report.invalid_objects += 1;
            continue;
        };
        match object.get("type").and_then(Value::as_str) {
            Some("BPM") => tempo(object, &mut bpm, report),
            Some("Single") => single(object, false, &mut notes, report),
            Some("Directional") => single(object, true, &mut notes, report),
            Some("Long" | "Slide") => slide(object, &mut notes, report),
            _ => report.unknown_types += 1,
        }
    }
    Ok(Score {
        metadata: Metadata {
            title: title.into(),
            ..Metadata::default()
        },
        start_beat: 0.0,
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

/// A `BPM`: the tempo from its beat on; the sign is dropped (`ayachan` takes the absolute value).
fn tempo(object: &Map<String, Value>, bpm: &mut Vec<BpmChange>, report: &mut BandoriReport) {
    let (Some(beat), Some(value)) = (number(object, "beat"), number(object, "bpm")) else {
        report.invalid_bpm += 1;
        return;
    };
    if value < 0.0 {
        report.negative_bpms += 1;
    }
    if valid_bpm(value.abs()) {
        bpm.push(BpmChange {
            beat,
            bpm: value.abs(),
        });
    } else {
        report.invalid_bpm += 1;
    }
}

/// The flags of a note that is kept: counted, then dropped (the model has no place for them).
fn count_flags(object: &Map<String, Value>, report: &mut BandoriReport) {
    report.skill_flags += usize::from(flag(object, "skill"));
    report.charge_flags += usize::from(flag(object, "charge"));
}

/// A `Single` (a tap, with `flick` a flick) or a `Directional` flick: a tap on its lane.
fn single(
    object: &Map<String, Value>,
    directional: bool,
    notes: &mut Vec<ScoreNote>,
    report: &mut BandoriReport,
) {
    let (Some(beat), Some(lane)) = (number(object, "beat"), number(object, "lane")) else {
        report.invalid_notes += 1;
        return;
    };
    let Some(column) = column_of(lane, FIRST_LANE, report) else {
        return;
    };
    if directional {
        report.directional_flicks += 1;
    } else {
        report.flicks += usize::from(flag(object, "flick"));
    }
    count_flags(object, report);
    notes.push(ScoreNote {
        beat,
        end_beat: None,
        column,
    });
}

/// A `Long` or a `Slide`: ONE hold from its first connection to its last, on the first connection's lane.
fn slide(object: &Map<String, Value>, notes: &mut Vec<ScoreNote>, report: &mut BandoriReport) {
    let Some(connections) = object
        .get("connections")
        .and_then(Value::as_array)
        .filter(|connections| !connections.is_empty())
    else {
        report.invalid_slides += 1;
        return;
    };
    let mut joints = Vec::with_capacity(connections.len());
    for connection in connections {
        let joint = connection
            .as_object()
            .and_then(|c| Some((c, number(c, "beat")?, number(c, "lane")?)));
        let Some(joint) = joint else {
            report.invalid_slides += 1;
            return;
        };
        joints.push(joint);
    }
    let (head, head_beat, head_lane) = joints[0];
    let (last, last_beat, _) = joints[joints.len() - 1];
    if joints.len() > 1 && last_beat < head_beat {
        report.invalid_slides += 1;
        return;
    }
    let Some(column) = column_of(head_lane, FIRST_LANE, report) else {
        return;
    };
    if joints.len() == 1 {
        report.single_connection_slides += 1;
        report.flicks += usize::from(flag(head, "flick"));
        count_flags(head, report);
        notes.push(ScoreNote {
            beat: head_beat,
            end_beat: None,
            column,
        });
        return;
    }
    report.flicks += usize::from(flag(last, "flick"));
    for (joint, _, _) in &joints {
        count_flags(joint, report);
    }
    report.slide_joints_dropped += joints.len() - 2;
    report.slides_with_lane_changes += usize::from(
        joints
            .iter()
            .any(|&(_, _, lane)| (lane - head_lane).abs() > SAME_LANE),
    );
    notes.push(ScoreNote {
        beat: head_beat,
        end_beat: Some(last_beat),
        column,
    });
}
