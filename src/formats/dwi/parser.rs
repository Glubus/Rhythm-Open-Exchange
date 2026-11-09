#![warn(clippy::pedantic)]
//! `.dwi` parser: a Dance With Intensity step file into the types of the `.sm` reader, so that
//! timing (BPM changes, freezes, the `#GAP`) and the columns go through the one pipeline of
//! [`crate::formats::sm`].
//!
//! Ported from StepMania's `NotesLoaderDWI.cpp` (the note grammar, `DWIcharToNote`, the hold
//! pairing, the tags and their units); see `mod.rs` for the decisions.
// Adapted from StepMania (MIT), src/NotesLoaderDWI.cpp, (c) 2001-2004 Chris Danford, Glenn Maynard;
// their permission notice is reproduced in VENDORED.md.
use std::collections::BTreeMap;

use rox::error::{RoxError, RoxResult};

use crate::formats::sm::legacy::{
    beat_to_row, decode_text, hhmmss_to_seconds, row_to_beat, split_non_empty, string_to_float,
    string_to_int,
};
use crate::formats::sm::msd::{Tag, tokenize_raw};
use crate::formats::sm::ssc_parser::{SongTiming, bpm_points};
use crate::formats::sm::types::{SmChart, SmFile, SmMetadata, SmNote, SmNoteType};

const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// A parsed file and what the parser repaired or ignored while reading it.
#[derive(Debug, Clone, Default)]
pub struct Parsed {
    /// The charts that can be played (`file.skipped` names the others).
    pub file: SmFile,
    /// One line per thing that was dropped or repaired in a chart or a tag.
    pub warnings: Vec<String>,
}

/// The two languages this parser reads: DWI, and MSD, the DDR'99 simulator script DWI grew from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Dwi,
    /// `.msd`: the step grammar of DWI without holds. `#GAP` is in the simulator's own dots, not milliseconds: it is not applied
    /// (warned). The text is Shift-JIS when it is not UTF-8. The chart tags give the level first in the spec (`#SINGLE:4:BASIC:...`) and
    /// the mode first in real files (`#SINGLE:BASIC:4:...`, as DWI): a first parameter that is a number and a second that is not is
    /// read as the spec's order.
    Msd,
}

/// Parses a `.dwi` file.
///
/// Charts that can not be played (`COUPLE` steps, no step data, timing that can not be honoured)
/// are left out and described in [`SmFile::skipped`].
///
/// # Errors
/// Returns an error if the data exceeds 100 MB.
pub fn parse_detailed(data: &[u8]) -> RoxResult<Parsed> {
    parse_dialect(data, Dialect::Dwi)
}

/// Parses a `.dwi` or a `.msd` file (see [`parse_detailed`]).
///
/// # Errors
/// Returns an error if the data exceeds 100 MB.
pub fn parse_dialect(data: &[u8], dialect: Dialect) -> RoxResult<Parsed> {
    if data.len() > MAX_FILE_SIZE {
        return Err(RoxError::InvalidFormat(format!(
            "File too large: {} bytes (max 100MB)",
            data.len()
        )));
    }
    let text = match (dialect, std::str::from_utf8(data)) {
        (Dialect::Msd, Err(_)) => encoding_rs::SHIFT_JIS.decode(data).0,
        _ => decode_text(data, false),
    };
    let mut tags = tokenize_raw(&text);
    let mut warnings = Vec::new();
    let mut header = Header::read(&tags, &mut warnings);
    if dialect == Dialect::Msd {
        if header.gap_ms != 0 {
            warnings.push(format!("#GAP {} is in the simulator's dots, not milliseconds: not applied", header.gap_ms));
        }
        header.gap_ms = 0;
        tags.iter_mut().filter(|tag| is_chart_tag(&tag.name)).for_each(level_first_to_mode_first);
    }
    let song_timing = SongTiming::new(header.bpms(), header.stops(), Vec::new());
    let mut file = SmFile {
        metadata: header.metadata,
        // `#GAP` is the time of beat 0 in ms, positive = later: the opposite sign of `#OFFSET`.
        offset_us: -i64::from(header.gap_ms) * 1000,
        bpms: song_timing.bpm_points(),
        ..SmFile::default()
    };
    let chart_tags = tags.iter().filter(|tag| is_chart_tag(&tag.name));
    for (index, tag) in chart_tags.enumerate() {
        let label = format!("chart {}", index + 1);
        match chart(tag, &song_timing, &mut warnings) {
            Ok(chart) => file.charts.push(chart),
            Err(reason) => file.skipped.push(format!("{label}: {reason}")),
        }
    }
    Ok(Parsed { file, warnings })
}

/// An MSD chart tag in the spec's order (`level:mode:steps`) put in DWI's (`mode:level:steps`).
fn level_first_to_mode_first(tag: &mut Tag) {
    let is_number = |param: &String| !param.trim().is_empty() && param.trim().bytes().all(|b| b.is_ascii_digit());
    if let [first, second, ..] = tag.params.as_mut_slice()
        && is_number(first)
        && !is_number(second)
    {
        std::mem::swap(first, second);
    }
}

/// Parses a `.dwi` file into the `.sm` types (see [`parse_detailed`] for what was repaired).
///
/// # Errors
/// Returns an error if the data exceeds 100 MB.
pub fn parse(data: &[u8]) -> RoxResult<SmFile> {
    parse_detailed(data).map(|parsed| parsed.file)
}

fn is_chart_tag(name: &str) -> bool {
    matches!(name, "SINGLE" | "DOUBLE" | "COUPLE" | "SOLO")
}

/// The song-wide tags: metadata, `#GAP` and the BPM and freeze lists (rows of 1/48 beat).
struct Header {
    metadata: SmMetadata,
    gap_ms: i32,
    /// `(row, bpm)`, one per row (the last given wins, as `AddSegment` replaces).
    bpm_rows: Vec<(i64, f32)>,
    /// `(row, seconds)`.
    stop_rows: Vec<(i64, f32)>,
}

impl Header {
    fn read(tags: &[Tag], warnings: &mut Vec<String>) -> Self {
        let mut header = Self {
            metadata: SmMetadata::default(),
            gap_ms: 0,
            bpm_rows: Vec::new(),
            stop_rows: Vec::new(),
        };
        for tag in tags {
            // Like the loader, the tags are read in order and a later one replaces an earlier one.
            match tag.name.as_str() {
                "FILE" => header.metadata.music = tag.text(),
                "TITLE" => header.metadata.title = tag.text(),
                "ARTIST" => header.metadata.artist = tag.text(),
                "GAP" => header.gap_ms = string_to_int(tag.first()),
                "BPM" => {
                    let bpm = string_to_float(tag.first());
                    if bpm > 0.0 {
                        header.set_bpm(0, bpm);
                    } else {
                        warnings.push(format!("#BPM {bpm} is not positive: ignored"));
                    }
                }
                "SAMPLESTART" => {
                    header.metadata.sample_start = f64::from(broken_timestamp(&tag.params));
                }
                "SAMPLELENGTH" => {
                    let mut length = broken_timestamp(&tag.params);
                    // Some versions stored seconds in thousands.
                    if length > 0.0 && length < 1.0 {
                        length *= 1000.0;
                    }
                    header.metadata.sample_length = f64::from(length);
                }
                "FREEZE" => header.read_freezes(tag.first(), warnings),
                "CHANGEBPM" | "BPMCHANGE" => header.read_bpm_changes(tag.first(), warnings),
                _ => {}
            }
        }
        header
    }

    fn set_bpm(&mut self, row: i64, bpm: f32) {
        match self.bpm_rows.iter_mut().find(|(at, _)| *at == row) {
            Some(entry) => entry.1 = bpm,
            None => self.bpm_rows.push((row, bpm)),
        }
    }

    /// `#FREEZE:BBB=sss,...`: at DWI beat BBB (4 per beat) the arrows stop for sss ms.
    fn read_freezes(&mut self, value: &str, warnings: &mut Vec<String>) {
        for expression in split_non_empty(value, ',') {
            let Some((position, millis)) = pair(expression) else {
                warnings.push(format!("invalid #FREEZE `{expression}`: ignored"));
                continue;
            };
            let row = beat_to_row(string_to_float(position) / 4.0);
            self.stop_rows.push((row, string_to_float(millis) / 1000.0));
        }
    }

    /// `#CHANGEBPM:BBB=nnn,...`: at DWI beat BBB (4 per beat) the BPM becomes nnn.
    fn read_bpm_changes(&mut self, value: &str, warnings: &mut Vec<String>) {
        for expression in split_non_empty(value, ',') {
            let Some((position, bpm)) = pair(expression) else {
                warnings.push(format!("invalid #CHANGEBPM `{expression}`: ignored"));
                continue;
            };
            let row = beat_to_row(string_to_float(position) / 4.0);
            let bpm = string_to_float(bpm);
            if bpm > 0.0 {
                self.set_bpm(row, bpm);
            } else {
                warnings.push(format!(
                    "#CHANGEBPM at beat {} has BPM {bpm}: ignored",
                    row_to_beat(row)
                ));
            }
        }
    }

    fn bpms(&self) -> Vec<(f64, f64)> {
        self.bpm_rows
            .iter()
            .map(|&(row, bpm)| (row_to_beat(row), f64::from(bpm)))
            .collect()
    }

    fn stops(&self) -> Vec<(f64, f64)> {
        self.stop_rows
            .iter()
            .map(|&(row, seconds)| (row_to_beat(row), f64::from(seconds)))
            .collect()
    }
}

/// `a=b` with exactly two non-empty halves, as the loader's `split(.., "=")` gives.
fn pair(expression: &str) -> Option<(&str, &str)> {
    let mut parts = split_non_empty(expression, '=');
    let (first, second) = (parts.next()?, parts.next()?);
    parts.next().is_none().then_some((first, second))
}

/// `ParseBrokenDWITimestamp`: milliseconds (`5230`), seconds (`5.23`), or `MM:SS.ss` /
/// `HH:MM:SS.ss` (the colons split the tag into parameters).
fn broken_timestamp(params: &[String]) -> f32 {
    let Some(first) = params.first().filter(|first| !first.is_empty()) else {
        return 0.0;
    };
    match params.get(1).filter(|second| !second.is_empty()) {
        None if first.contains('.') => string_to_float(first),
        None => string_to_float(first) / 1000.0,
        Some(second) => match params.get(2).filter(|third| !third.is_empty()) {
            None => hhmmss_to_seconds(&format!("{first}:{second}")),
            Some(third) => hhmmss_to_seconds(&format!("{first}:{second}:{third}")),
        },
    }
}

/// The panels of a pad.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    Left,
    UpLeft,
    Down,
    Up,
    UpRight,
    Right,
}

/// The panels a step character presses (`DWIcharToNote`); `None` for a character that is not one.
fn panels_of(c: char) -> Option<[Option<Panel>; 2]> {
    use Panel::{Down, Left, Right, Up, UpLeft, UpRight};
    Some(match c {
        '0' | '5' => [None, None],
        '1' => [Some(Down), Some(Left)],
        '2' => [Some(Down), None],
        '3' => [Some(Down), Some(Right)],
        '4' => [Some(Left), None],
        '6' => [Some(Right), None],
        '7' => [Some(Up), Some(Left)],
        '8' => [Some(Up), None],
        '9' => [Some(Up), Some(Right)],
        'A' => [Some(Up), Some(Down)],
        'B' => [Some(Left), Some(Right)],
        'C' => [Some(UpLeft), None],
        'D' => [Some(UpRight), None],
        'E' => [Some(Left), Some(UpLeft)],
        'F' => [Some(UpLeft), Some(Down)],
        'G' => [Some(UpLeft), Some(Up)],
        'H' => [Some(UpLeft), Some(Right)],
        'I' => [Some(Left), Some(UpRight)],
        'J' => [Some(Down), Some(UpRight)],
        'K' => [Some(Up), Some(UpRight)],
        'L' => [Some(UpRight), Some(Right)],
        'M' => [Some(UpLeft), Some(UpRight)],
        _ => return None,
    })
}

/// The step types of a `.dwi` this crate plays.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Single,
    Double,
    Solo,
}

impl Style {
    fn columns(self) -> u8 {
        match self {
            Self::Single => 4,
            Self::Solo => 6,
            Self::Double => 8,
        }
    }

    /// The column of a panel of pad `pad` (the order of the `.sm` step types: left, down, up,
    /// right; `dance-solo` left, up-left, down, up, up-right, right); `None` when the style has
    /// no such panel.
    fn column(self, panel: Panel, pad: u8) -> Option<u8> {
        let column = match self {
            Self::Single | Self::Double => match panel {
                Panel::Left => 0,
                Panel::Down => 1,
                Panel::Up => 2,
                Panel::Right => 3,
                Panel::UpLeft | Panel::UpRight => return None,
            },
            Self::Solo => match panel {
                Panel::Left => 0,
                Panel::UpLeft => 1,
                Panel::Down => 2,
                Panel::Up => 3,
                Panel::UpRight => 4,
                Panel::Right => 5,
            },
        };
        Some(column + 4 * pad)
    }
}

/// What reading the step strings of a chart dropped.
#[derive(Default)]
struct StepStats {
    /// Characters that are no step character (they still take their time slot).
    invalid_characters: usize,
    /// Panels the step type does not have (`C`..`M` outside `SOLO`, the second pad's extras).
    missing_panels: usize,
    /// Holds with no later arrow in their column to release them: removed.
    unclosed_holds: usize,
}

/// Whether `<` at `chars[start - 1]` opens a 1/192 marker (a `0` before the next `>`), not a jump.
fn is_192(chars: &[char], start: usize) -> bool {
    for &c in &chars[start..] {
        match c {
            '>' => return false,
            '0' => return true,
            _ => {}
        }
    }
    false
}

/// Beats per character: 1/8 of a measure of 4 beats.
const EIGHTH: f64 = 4.0 / 8.0;

/// The notes of one pad: `(column, row) -> is a hold head`. A later write on a cell replaces the
/// earlier one, as `SetTapNote` does.
fn read_pad(steps: &str, style: Style, pad: u8, cells: &mut BTreeMap<(u8, i64), bool>, stats: &mut StepStats) {
    let chars: Vec<char> = steps.chars().filter(|c| !matches!(c, '\n' | '\r' | '\t' | ' ')).collect();
    let mut reader = PadReader { chars: &chars, i: 0, style, pad, cells, stats };
    let (mut beat, mut step) = (0.0_f64, EIGHTH);
    while let Some(c) = reader.next() {
        match c {
            '(' => step = 4.0 / 16.0,
            '[' => step = 4.0 / 24.0,
            '{' => step = 4.0 / 64.0,
            '`' => step = 4.0 / 192.0,
            ')' | ']' | '}' | '\'' | '>' => step = EIGHTH,
            // A hold marker with no arrow before it.
            '!' => reader.stats.invalid_characters += 1,
            // Old files used `<..>` as the 1/192 marker.
            '<' if is_192(&chars, reader.i) => step = 4.0 / 192.0,
            _ => {
                #[allow(clippy::cast_possible_truncation)] // a beat is far below f32 range limits
                let row = beat_to_row(beat as f32);
                if c == '<' {
                    reader.jump(row);
                } else {
                    reader.arrow(c, row);
                }
                beat += step;
            }
        }
    }
}

/// The characters of one pad's steps being read, and where their notes go.
struct PadReader<'a> {
    chars: &'a [char],
    i: usize,
    style: Style,
    pad: u8,
    cells: &'a mut BTreeMap<(u8, i64), bool>,
    stats: &'a mut StepStats,
}

impl PadReader<'_> {
    fn next(&mut self) -> Option<char> {
        let c = self.chars.get(self.i).copied()?;
        self.i += 1;
        Some(c)
    }

    /// `<...>`: every arrow up to `>` sounds in the same slot.
    fn jump(&mut self, row: i64) {
        while let Some(c) = self.next() {
            if c == '>' {
                break;
            }
            self.arrow(c, row);
        }
    }

    /// An arrow character, and the hold after it: `show!hold` starts a hold on the panels of the character after `!`.
    fn arrow(&mut self, c: char, row: i64) {
        self.place(c, false, row);
        if self.chars.get(self.i) == Some(&'!') {
            self.i += 1;
            let hold = self.next().unwrap_or('\0');
            self.place(hold, true, row);
        }
    }

    fn place(&mut self, c: char, head: bool, row: i64) {
        let Some(panels) = panels_of(c) else {
            self.stats.invalid_characters += 1;
            return;
        };
        for panel in panels.into_iter().flatten() {
            if let Some(column) = self.style.column(panel, self.pad) {
                self.cells.insert((column, row), head);
            } else {
                self.stats.missing_panels += 1;
            }
        }
    }
}

/// `(difficulty name as StepMania files it, ...)`: the loader's `DwiCompatibleStringToDifficulty`.
fn difficulty_of(name: &str) -> Option<&'static str> {
    Some(match name.to_ascii_lowercase().as_str() {
        "beginner" => "Beginner",
        "easy" | "basic" | "light" => "Easy",
        "medium" | "another" | "trick" | "standard" | "difficult" => "Medium",
        "hard" | "ssr" | "maniac" | "heavy" => "Hard",
        "smaniac" | "challenge" | "expert" | "oni" => "Challenge",
        "edit" => "Edit",
        _ => return None,
    })
}

fn chart(tag: &Tag, song: &SongTiming, warnings: &mut Vec<String>) -> Result<SmChart, String> {
    let (stepstype, style) = match tag.name.as_str() {
        "SINGLE" => ("dance-single", Style::Single),
        "DOUBLE" => ("dance-double", Style::Double),
        "SOLO" => ("dance-solo", Style::Solo),
        "COUPLE" => {
            return Err("unsupported step type `dance-couple` (two players' steps)".to_string());
        }
        other => return Err(format!("unknown step type `{other}`")),
    };
    let [difficulty, feet, step1, rest @ ..] = tag.params.as_slice() else {
        return Err(format!(
            "{stepstype}: malformed tag (expected difficulty, feet and steps)"
        ));
    };
    let step2 = rest.first().map_or("", String::as_str);
    if step1.chars().count() < 2 && step2.chars().count() < 2 {
        return Err(format!("{stepstype}: no step data"));
    }
    let (bpms, timing) = song
        .timing_for(&[])
        .map_err(|reason| format!("{stepstype}: {reason}"))?;
    let mut cells = BTreeMap::new();
    let mut stats = StepStats::default();
    read_pad(step1, style, 0, &mut cells, &mut stats);
    if style == Style::Double && !step2.is_empty() {
        read_pad(step2, style, 1, &mut cells, &mut stats);
    }
    let notes = pair_holds(style.columns(), &cells, &timing, &mut stats);
    let meter = if feet.is_empty() {
        1
    } else {
        u32::try_from(string_to_int(feet)).unwrap_or(0)
    };
    let label = format!("{stepstype} {difficulty}");
    if stats.invalid_characters > 0 {
        warnings.push(format!(
            "{label}: {} invalid step characters (their time slot kept)",
            stats.invalid_characters
        ));
    }
    if stats.missing_panels > 0 {
        warnings.push(format!(
            "{label}: {} panels the step type does not have: ignored",
            stats.missing_panels
        ));
    }
    if stats.unclosed_holds > 0 {
        warnings.push(format!(
            "{label}: {} holds never closed: removed",
            stats.unclosed_holds
        ));
    }
    Ok(SmChart {
        stepstype: stepstype.to_string(),
        difficulty: difficulty_of(difficulty).map_or_else(|| difficulty.clone(), str::to_string),
        meter,
        column_count: style.columns(),
        notes,
        description: difficulty.clone(),
        credit: None,
        bpms: Some(bpm_points(&bpms, &timing)),
        offset_us: None,
    })
}

/// Turns the cells into timed notes. A hold head is released by the next arrow of its column,
/// whatever it is: that arrow is consumed (the loader's "fill in iDuration").
fn pair_holds(
    columns: u8,
    cells: &BTreeMap<(u8, i64), bool>,
    timing: &crate::formats::sm::timing_data::TimingData,
    stats: &mut StepStats,
) -> Vec<SmNote> {
    let note = |row: i64, column: u8, note_type| SmNote {
        time_us: timing.note_us(row_to_beat(row)),
        column,
        note_type,
    };
    let mut notes = Vec::new();
    for column in 0..columns {
        let rows: Vec<(i64, bool)> = cells
            .range((column, i64::MIN)..=(column, i64::MAX))
            .map(|(&(_, row), &head)| (row, head))
            .collect();
        let mut index = 0;
        while index < rows.len() {
            let (row, head) = rows[index];
            if !head {
                notes.push(note(row, column, SmNoteType::Tap));
                index += 1;
            } else if let Some(&(tail, _)) = rows.get(index + 1) {
                notes.push(note(row, column, SmNoteType::HoldHead));
                notes.push(note(tail, column, SmNoteType::Tail));
                index += 2;
            } else {
                stats.unclosed_holds += 1;
                index += 1;
            }
        }
    }
    notes
}
