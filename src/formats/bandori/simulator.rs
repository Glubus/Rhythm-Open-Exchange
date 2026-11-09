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
    const NONE: Self = Self { flick: false, directional: false, skill: false, charge: false };
    const FLICK: Self = Self { flick: true, ..Self::NONE };
    const SKILL: Self = Self { skill: true, ..Self::NONE };

    fn count(self, report: &mut BandoriReport) {
        report.flicks += usize::from(self.flick);
        report.directional_flicks += usize::from(self.directional);
        report.skill_flags += usize::from(self.skill);
        report.charge_flags += usize::from(self.charge);
    }
}

/// The four type numbers of each chain's end: plain, flick, skill (`Bang info.txt`; 73/74/76 for the numbered slides).
const ENDS: [(Family, [u32; 3]); 4] = [
    (Family::A, [5, 12, 34]),
    (Family::B, [8, 13, 36]),
    (Family::Long, [25, 26, 32]),
    (Family::Numbered, [73, 74, 76]),
];

/// The meaning of a note type of the notes (`Bang info.txt`, versions 1.30 and 2.00) and of the numbered slides of `BanGConverter`
/// (71 to 77); `None` for a type that is none of them.
fn kind_of(kind: u32) -> Option<Kind> {
    let tap = |flags| Some(Kind::Tap(flags));
    match kind {
        0 => Some(Kind::MusicStart),
        20 => Some(Kind::Bpm),
        1 => tap(Flags::NONE),
        2 => tap(Flags::FLICK),
        10 => tap(Flags { charge: true, ..Flags::NONE }),
        11 => tap(Flags::SKILL),
        51..=57 | 61..=67 => tap(Flags { directional: true, ..Flags::NONE }),
        3 | 33 => Some(Kind::Start(Family::A, kind == 33)),
        6 | 35 => Some(Kind::Start(Family::B, kind == 35)),
        21 | 31 => Some(Kind::Start(Family::Long, kind == 31)),
        71 | 75 => Some(Kind::Start(Family::Numbered, kind == 75)),
        4 | 41 => Some(Kind::Among(Family::A)),
        7 | 42 => Some(Kind::Among(Family::B)),
        72 | 77 => Some(Kind::Among(Family::Numbered)),
        _ => end_of(kind),
    }
}

fn end_of(kind: u32) -> Option<Kind> {
    let (family, [_, flick, skill]) = ENDS.into_iter().find(|(_, types)| types.contains(&kind))?;
    Some(Kind::End(family, Flags { flick: kind == flick, skill: kind == skill, ..Flags::NONE }))
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
    let Some(kind) = fields.get(1).and_then(|field| field.trim().parse::<u32>().ok()) else {
        report.invalid_notes += 1;
        return None;
    };
    let Some(parsed) = kind_of(kind) else {
        report.unknown_types += 1;
        return None;
    };
    let Some((beat, value, number)) = line_fields(&fields, matches!(kind, 71..=77)) else {
        report.invalid_notes += 1;
        return None;
    };
    Some(Line { beat, kind: parsed, value, number })
}

/// `(beat, value, number)` of `beat/type/value`, or of `beat/type/number/value` for a numbered slide.
fn line_fields(fields: &[&str], numbered: bool) -> Option<(f64, f64, u32)> {
    match (numbered, fields) {
        (true, [beat, _, number, value]) => Some((float(beat)?, float(value)?, number.trim().parse().ok()?)),
        (false, [beat, _, value]) => Some((float(beat)?, float(value)?, 0)),
        _ => None,
    }
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
        return Err(invalid(&format!("file too large: {} bytes (max {MAX_FILE_SIZE})", data.len())));
    }
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let text = String::from_utf8_lossy(data);
    report.invalid_text = matches!(text, std::borrow::Cow::Owned(_));
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let song_bpm = read_header(&mut lines, report)?;
    let body = read_body(lines, song_bpm, report)?;
    Ok(Score {
        metadata: Metadata { title: title.into(), ..Metadata::default() },
        start_beat: body.start_beat.unwrap_or(0.0),
        bpm: body.bpm,
        notes: notes_of(&body.notes, report),
    })
}

/// The two header lines: the note offset (kept in the report) and the song's BPM, returned.
fn read_header<'a>(lines: &mut impl Iterator<Item = &'a str>, report: &mut BandoriReport) -> RoxResult<f64> {
    let offset = lines.next().and_then(float).ok_or_else(|| invalid("the first line is not a number (the note offset)"))?;
    report.note_offset = Some(offset);
    lines.next().and_then(float).filter(|&bpm| valid_bpm(bpm)).ok_or_else(|| invalid("the second line is not a valid BPM"))
}

/// The lines after the header, sorted out: the music start, the BPM changes, and the notes in beat order.
struct Body {
    start_beat: Option<f64>,
    bpm: Vec<BpmChange>,
    notes: Vec<Line>,
}

fn read_body<'a>(lines: impl Iterator<Item = &'a str>, song_bpm: f64, report: &mut BandoriReport) -> RoxResult<Body> {
    let mut body = Body { start_beat: None, bpm: vec![BpmChange { beat: 0.0, bpm: song_bpm }], notes: Vec::new() };
    for (count, line) in lines.enumerate() {
        if count >= MAX_OBJECTS {
            return Err(invalid(&format!("more than {MAX_OBJECTS} lines")));
        }
        let Some(read) = read_line(line, report) else { continue };
        match read.kind {
            Kind::MusicStart if body.start_beat.is_none() => body.start_beat = Some(read.beat),
            Kind::MusicStart => report.extra_music_starts += 1,
            Kind::Bpm if valid_bpm(read.value) => body.bpm.push(BpmChange { beat: read.beat, bpm: read.value }),
            Kind::Bpm => report.invalid_bpm += 1,
            _ => body.notes.push(read),
        }
    }
    // The notes are read in beat order; at one beat, in file order ("notes at the same time should be sequentially ordered").
    body.notes.sort_by(|a, b| a.beat.total_cmp(&b.beat));
    Ok(body)
}

/// Taps, and one hold for each slide and long note.
fn notes_of(lines: &[Line], report: &mut BandoriReport) -> Vec<ScoreNote> {
    let mut chains = Chains { notes: Vec::new(), open: HashMap::new(), report };
    for line in lines {
        match line.kind {
            Kind::Tap(flags) => chains.tap(line, flags),
            Kind::Start(family, skill) => chains.start(family, skill, line),
            Kind::Among(family) => chains.joint(family, line),
            Kind::End(family, flags) => chains.end(family, flags, line),
            Kind::MusicStart | Kind::Bpm => {}
        }
    }
    chains.report.unterminated_slides += chains.open.len();
    chains.notes
}

/// The notes read so far and the slides and long notes still open.
struct Chains<'r> {
    notes: Vec<ScoreNote>,
    open: HashMap<Key, Open>,
    report: &'r mut BandoriReport,
}

impl Chains<'_> {
    fn tap(&mut self, line: &Line, flags: Flags) {
        if let Some(column) = column_of(line.value, FIRST_LANE, self.report) {
            flags.count(self.report);
            self.notes.push(ScoreNote { beat: line.beat, end_beat: None, column });
        }
    }

    fn start(&mut self, family: Family, skill: bool, line: &Line) {
        let Some(key) = key_of(family, line, self.report) else { return };
        // The lane of a long note was counted by `key_of`; the other families count it here.
        let column = if family == Family::Long { u8::try_from(key.1).ok() } else { column_of(line.value, FIRST_LANE, self.report) };
        self.report.skill_flags += usize::from(skill);
        let started = Open { beat: line.beat, column, lane: line.value, joints: 0, lane_changed: false };
        if self.open.insert(key, started).is_some() {
            self.report.unterminated_slides += 1;
        }
    }

    fn joint(&mut self, family: Family, line: &Line) {
        match key_of(family, line, self.report).and_then(|key| self.open.get_mut(&key)) {
            Some(slide) => {
                slide.joints += 1;
                slide.lane_changed |= (line.value - slide.lane).abs() > SAME_LANE;
            }
            None => self.report.slide_points_without_start += 1,
        }
    }

    fn end(&mut self, family: Family, flags: Flags, line: &Line) {
        let Some(key) = key_of(family, line, self.report) else { return };
        let Some(slide) = self.open.remove(&key) else {
            self.report.slide_points_without_start += 1;
            return;
        };
        flags.count(self.report);
        let Some(column) = slide.column else { return };
        self.report.slide_joints_dropped += slide.joints;
        let moved = family != Family::Long && (line.value - slide.lane).abs() > SAME_LANE;
        self.report.slides_with_lane_changes += usize::from(slide.lane_changed || moved);
        self.notes.push(ScoreNote { beat: slide.beat, end_beat: Some(line.beat), column });
    }
}
