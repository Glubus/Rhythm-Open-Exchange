#![warn(clippy::pedantic)]
//! BanG! Simulator `.txt` reader: text to a [`Score`] (beats, columns), then to a ROX chart through `score::build`. See `mod.rs` for the
//! decisions.

use std::collections::HashMap;
use std::path::Path;

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, RoxChart};

use super::score::{
    BandoriReport, BpmChange, MAX_OBJECTS, Score, ScoreNote, build, column_of, valid_bpm,
};

/// Decoder of the BanG! Simulator's `.txt` score: one chart per file.
pub struct SimulatorDecoder;
crate::impl_format!(SimulatorDecoder, ["txt"]);

/// Files above this size are refused (a real score is a few hundred lines).
const MAX_FILE_SIZE: usize = 16 * 1024 * 1024;
/// The number of the leftmost lane of a simulator score (`8/1/3`: "the 3rd lane", 1 to 7).
const FIRST_LANE: i64 = 1;
/// A lane this close to the head's lane is the same lane, for the count of slides that change lane.
const SAME_LANE: f64 = 1e-4;

impl Decoder for SimulatorDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode(data, "").map(|(chart, _)| chart)
    }
}

impl SimulatorDecoder {
    /// Decodes a simulator score; it has no title, `title` becomes the chart's.
    ///
    /// # Errors
    /// Returns an error if the file is too large or not a simulator score or has no playable note.
    pub fn decode_with_title(data: &[u8], title: &str) -> RoxResult<RoxChart> {
        decode(data, title).map(|(chart, _)| chart)
    }

    /// Decodes a simulator score and says what was dropped or repaired.
    ///
    /// # Errors
    /// Returns an error if the file is too large or not a simulator score or has no playable note.
    pub fn decode_with_report(data: &[u8]) -> RoxResult<(RoxChart, BandoriReport)> {
        decode(data, "")
    }

    /// Decodes the simulator score at `path`; the file stem is the title.
    ///
    /// # Errors
    /// Returns an error if the file can not be read, is too large or not a simulator score or has no playable note.
    pub fn decode_file(path: &Path) -> RoxResult<RoxChart> {
        Self::decode_with_title(&std::fs::read(path)?, &super::title_of(path))
    }
}

fn decode(data: &[u8], title: &str) -> RoxResult<(RoxChart, BandoriReport)> {
    let mut report = BandoriReport::default();
    let score = parse(data, title, &mut report)?;
    let chart = build(score, &mut report, "txt")?;
    chart.validate()?;
    Ok((chart, report))
}

fn invalid(reason: &str) -> RoxError {
    RoxError::InvalidFormat(format!("txt: {reason}"))
}

/// The interleaved chains a joint can belong to: the notes' slide chains A and B, the long notes (one per lane) and the numbered
/// slides of types 71 to 77 (`BanGConverter`'s third and later overlapping slides; the number is the second field).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    A,
    B,
    Long,
    Numbered,
}

/// What a note type means.
#[derive(Clone, Copy)]
enum Kind {
    MusicStart,
    Bpm,
    Tap(Flags),
    Start(Family, bool),
    Among(Family),
    End(Family, Flags),
}

/// The flags a note can carry; every one is counted and dropped, none changes the note.
#[allow(clippy::struct_excessive_bools)] // four independent flags of one note
#[derive(Clone, Copy, Default)]
struct Flags {
    flick: bool,
    directional: bool,
    skill: bool,
    charge: bool,
}

impl Flags {
    fn count(self, report: &mut BandoriReport) {
        report.flicks += usize::from(self.flick);
        report.directional_flicks += usize::from(self.directional);
        report.skill_flags += usize::from(self.skill);
        report.charge_flags += usize::from(self.charge);
    }
}

/// The meaning of a note type of the notes (`Bang info.txt`, versions 1.30 and 2.00) and of the numbered slides of `BanGConverter`
/// (71 to 77); `None` for a type that is none of them.
fn kind_of(kind: u32) -> Option<Kind> {
    let flags = Flags::default();
    Some(match kind {
        0 => Kind::MusicStart,
        20 => Kind::Bpm,
        1 => Kind::Tap(flags),
        2 => Kind::Tap(Flags {
            flick: true,
            ..flags
        }),
        10 => Kind::Tap(Flags {
            charge: true,
            ..flags
        }),
        11 => Kind::Tap(Flags {
            skill: true,
            ..flags
        }),
        51..=57 | 61..=67 => Kind::Tap(Flags {
            directional: true,
            ..flags
        }),
        3 | 33 => Kind::Start(Family::A, kind == 33),
        6 | 35 => Kind::Start(Family::B, kind == 35),
        21 | 31 => Kind::Start(Family::Long, kind == 31),
        71 | 75 => Kind::Start(Family::Numbered, kind == 75),
        4 | 41 => Kind::Among(Family::A),
        7 | 42 => Kind::Among(Family::B),
        72 | 77 => Kind::Among(Family::Numbered),
        5 | 12 | 34 => Kind::End(
            Family::A,
            Flags {
                flick: kind == 12,
                skill: kind == 34,
                ..flags
            },
        ),
        8 | 13 | 36 => Kind::End(
            Family::B,
            Flags {
                flick: kind == 13,
                skill: kind == 36,
                ..flags
            },
        ),
        25 | 26 | 32 => Kind::End(
            Family::Long,
            Flags {
                flick: kind == 26,
                skill: kind == 32,
                ..flags
            },
        ),
        73 | 74 | 76 => Kind::End(
            Family::Numbered,
            Flags {
                flick: kind == 74,
                skill: kind == 76,
                ..flags
            },
        ),
        _ => return None,
    })
}

/// A parsed `beat/type/lane` line (`beat/type/number/lane` for the numbered slides).
struct Line {
    beat: f64,
    kind: Kind,
    /// The third field: a lane, or the BPM of a type 20.
    value: f64,
    /// The numbered slide's number.
    number: u32,
}

/// A finite number.
fn float(field: &str) -> Option<f64> {
    field.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Reads a note line; `None` once counted (a type unknown to this decoder, or a line that is not `beat/type/lane`).
fn read_line(line: &str, report: &mut BandoriReport) -> Option<Line> {
    let fields: Vec<&str> = line.split('/').collect();
    let kind = fields
        .get(1)
        .and_then(|field| field.trim().parse::<u32>().ok());
    let Some(kind) = kind else {
        report.invalid_notes += 1;
        return None;
    };
    let Some(parsed) = kind_of(kind) else {
        report.unknown_types += 1;
        return None;
    };
    let numbered = matches!(kind, 71..=77);
    let read = if numbered && fields.len() == 4 {
        float(fields[0])
            .zip(float(fields[3]))
            .zip(fields[2].trim().parse::<u32>().ok())
            .map(|((beat, value), number)| (beat, value, number))
    } else if !numbered && fields.len() == 3 {
        float(fields[0])
            .zip(float(fields[2]))
            .map(|(beat, value)| (beat, value, 0))
    } else {
        None
    };
    let Some((beat, value, number)) = read else {
        report.invalid_notes += 1;
        return None;
    };
    Some(Line {
        beat,
        kind: parsed,
        value,
        number,
    })
}

/// A slide or long note being read: from its start to the joint just read.
struct Open {
    beat: f64,
    /// `None` when the start's lane was not one of the 7 (counted): the slide is read to its end and dropped.
    column: Option<u8>,
    lane: f64,
    joints: usize,
    lane_changed: bool,
}

/// Which open slide a joint belongs to: A, B, a lane's long note, or a numbered slide.
type Key = (u8, u32);

fn key_of(family: Family, line: &Line, report: &mut BandoriReport) -> Option<Key> {
    Some(match family {
        Family::A => (0, 0),
        Family::B => (1, 0),
        Family::Numbered => (2, line.number),
        Family::Long => (3, u32::from(column_of(line.value, FIRST_LANE, report)?)),
    })
}

fn parse(data: &[u8], title: &str, report: &mut BandoriReport) -> RoxResult<Score> {
    if data.len() > MAX_FILE_SIZE {
        return Err(invalid(&format!(
            "file too large: {} bytes (max {MAX_FILE_SIZE})",
            data.len()
        )));
    }
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let text = String::from_utf8_lossy(data);
    report.invalid_text = matches!(text, std::borrow::Cow::Owned(_));
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let offset = lines
        .next()
        .and_then(float)
        .ok_or_else(|| invalid("the first line is not a number (the note offset)"))?;
    report.note_offset = Some(offset);
    let song_bpm = lines
        .next()
        .and_then(float)
        .filter(|&bpm| valid_bpm(bpm))
        .ok_or_else(|| invalid("the second line is not a valid BPM"))?;

    let mut bpm = vec![BpmChange {
        beat: 0.0,
        bpm: song_bpm,
    }];
    let mut start_beat = None;
    let mut found = Vec::new();
    for (count, line) in lines.enumerate() {
        if count >= MAX_OBJECTS {
            return Err(invalid(&format!("more than {MAX_OBJECTS} lines")));
        }
        let Some(read) = read_line(line, report) else {
            continue;
        };
        match read.kind {
            Kind::MusicStart if start_beat.is_none() => start_beat = Some(read.beat),
            Kind::MusicStart => report.extra_music_starts += 1,
            Kind::Bpm if valid_bpm(read.value) => bpm.push(BpmChange {
                beat: read.beat,
                bpm: read.value,
            }),
            Kind::Bpm => report.invalid_bpm += 1,
            _ => found.push(read),
        }
    }
    // The notes are read in beat order; at one beat, in file order ("notes at the same time should be sequentially ordered").
    found.sort_by(|a, b| a.beat.total_cmp(&b.beat));
    let notes = notes_of(&found, report);
    Ok(Score {
        metadata: Metadata {
            title: title.into(),
            ..Metadata::default()
        },
        start_beat: start_beat.unwrap_or(0.0),
        bpm,
        notes,
    })
}

/// Taps, and one hold for each slide and long note.
fn notes_of(lines: &[Line], report: &mut BandoriReport) -> Vec<ScoreNote> {
    let mut notes = Vec::new();
    let mut open: HashMap<Key, Open> = HashMap::new();
    for line in lines {
        match line.kind {
            Kind::Tap(flags) => {
                if let Some(column) = column_of(line.value, FIRST_LANE, report) {
                    flags.count(report);
                    notes.push(ScoreNote {
                        beat: line.beat,
                        end_beat: None,
                        column,
                    });
                }
            }
            Kind::Start(family, skill) => {
                let Some(key) = key_of(family, line, report) else {
                    continue;
                };
                // The lane of a long note was counted by `key_of`; the other families count it here.
                let column = if family == Family::Long {
                    u8::try_from(key.1).ok()
                } else {
                    column_of(line.value, FIRST_LANE, report)
                };
                report.skill_flags += usize::from(skill);
                let started = Open {
                    beat: line.beat,
                    column,
                    lane: line.value,
                    joints: 0,
                    lane_changed: false,
                };
                if open.insert(key, started).is_some() {
                    report.unterminated_slides += 1;
                }
            }
            Kind::Among(family) => {
                match key_of(family, line, report).and_then(|key| open.get_mut(&key)) {
                    Some(slide) => {
                        slide.joints += 1;
                        slide.lane_changed |= (line.value - slide.lane).abs() > SAME_LANE;
                    }
                    None => report.slide_points_without_start += 1,
                }
            }
            Kind::End(family, flags) => {
                let Some(key) = key_of(family, line, report) else {
                    continue;
                };
                let Some(slide) = open.remove(&key) else {
                    report.slide_points_without_start += 1;
                    continue;
                };
                flags.count(report);
                let Some(column) = slide.column else {
                    continue;
                };
                report.slide_joints_dropped += slide.joints;
                report.slides_with_lane_changes += usize::from(
                    slide.lane_changed
                        || (family != Family::Long && (line.value - slide.lane).abs() > SAME_LANE),
                );
                notes.push(ScoreNote {
                    beat: slide.beat,
                    end_beat: Some(line.beat),
                    column,
                });
            }
            Kind::MusicStart | Kind::Bpm => {}
        }
    }
    report.unterminated_slides += open.len();
    notes
}
