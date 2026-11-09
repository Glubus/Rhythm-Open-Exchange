#![warn(clippy::pedantic)]
//! pydance step files (`.dance`): song metadata, then one step section per game mode and difficulty. Decode only.
//!
//! Format notes: `vsrg-format-docs/vsrg/pydance/dance`, from the spec (`dance-spec.txt` 1.2) and pydance's own reader and player
//! (`fileparsers.py` `DanceFile`, `steps.py` `Steps`, `games.py`; read for facts), checked on the corpus files.
//!
//! - **Lines**: stripped; blank lines and lines starting with `#` are skipped; `end` closes a section. The metadata comes first
//!   (`key value`); `DESCRIPTION`, `LYRICS` and `BACKGROUND` sections are skipped; any other section is steps: its game mode
//!   (`SINGLE`, `DOUBLE`, `6PANEL`…), then `<difficulty> <rating>`, then the step lines.
//! - **Variants**: one per step section, in file order. The columns are the characters of a button string (all the same length:
//!   4 in `SINGLE`, 8 in `DOUBLE`, 6 in `6PANEL`…), in the order the game draws them. A `…COUPLE` line holds one string per player:
//!   the first player's is read.
//! - **Time** (`Steps.__init__`): starts at `startat - gap / 1000` seconds. A note line `<length> <buttons>` plays its buttons, then
//!   lasts its length (`o` 4 beats, `h` 2, `q` 1, `e` 1/2, `w` 1/3, `s` 1/4, `f` 1/6, `t` 1/8, `u` 1/12, `x` 1/16, `n` 1/48); `D x`
//!   lasts `x` beats, `W x` waits `x` seconds, `B x` sets the BPM, `S x` stops `x` seconds (the first notes after it play when the stop
//!   starts, as pydance places them). `R` and `L` show graphics or lyrics.
//! - **Buttons**: a digit per column, read as bits: 1 press, 2 hold, 4 secret. A hold starts on a hold digit; the next non-zero digit
//!   of its column ends it (and is no note). Secret notes (hidden, not counted by the game unless the player asks) and unknown
//!   characters are dropped and counted.
//! - **Metadata**: `title` (+ `subtitle`), `artist`, `author`, `filename` (the audio), `background`, `banner`, `preview` (start).

use std::borrow::Cow;

use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, Note, RoxChart, TimingPoint, drop_stacked};

/// Decoder of pydance `.dance` files: several charts per file (see [`DanceDecoder::variants`]).
pub struct DanceDecoder;

/// Files above this size are refused (a song is a few tens of kilobytes).
const MAX_FILE_SIZE: usize = 16 * 1024 * 1024;
/// Most columns a button string may have.
const MAX_COLUMNS: usize = 64;
/// Times further than this many seconds from the start are not read.
const MAX_SECONDS: f64 = 1e7;

/// One step section of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub index: usize,
    /// The game mode as written (`SINGLE`, `DOUBLE`, `6PANEL`…).
    pub mode: String,
    /// The difficulty as written (`BASIC`, `TRICK`, `MANIAC`…).
    pub difficulty: String,
    /// The rating (feet).
    pub rating: i64,
}

/// What the decoder dropped. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DanceReport {
    /// Secret notes and unknown button characters: dropped.
    pub secret_or_unknown: usize,
    /// Holds never ended: dropped.
    pub open_holds: usize,
    /// Lines that are no step or command, or a button string of another length: skipped.
    pub skipped_lines: usize,
    /// Notes on another note of their column (same time, or inside a hold): dropped.
    pub overlapping_notes_dropped: usize,
}

struct Section<'a> {
    variant: Variant,
    /// Step lines in the `.dance` grammar (a pyDDR `.step` line is translated to it).
    lines: Vec<Cow<'a, str>>,
}

struct Song<'a> {
    meta: Vec<(&'a str, &'a str)>,
    sections: Vec<Section<'a>>,
}

impl Song<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.meta.iter().find(|(k, _)| *k == key).map(|(_, value)| *value).filter(|value| !value.is_empty())
    }

    fn number(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|value| value.split_whitespace().next()).and_then(|value| value.parse().ok()).filter(|value: &f64| value.is_finite())
    }
}

fn invalid(reason: impl std::fmt::Display) -> RoxError {
    RoxError::InvalidFormat(format!("dance: {reason}"))
}

fn parse(data: &[u8]) -> RoxResult<Song<'_>> {
    if data.len() > MAX_FILE_SIZE {
        return Err(invalid(format!("file too large: {} bytes", data.len())));
    }
    let text = std::str::from_utf8(data).map_err(|_| invalid("not UTF-8"))?;
    let mut song = Song { meta: Vec::new(), sections: Vec::new() };
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#'));
    // The metadata, up to its `end`.
    for line in lines.by_ref() {
        if line == "end" {
            break;
        }
        let (key, value) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        song.meta.push((key, value.trim()));
    }
    while let Some(token) = lines.next() {
        if matches!(token, "DESCRIPTION" | "LYRICS" | "BACKGROUND") {
            lines.by_ref().find(|line| *line == "end");
            continue;
        }
        let Some(header) = lines.next().filter(|line| *line != "end") else { continue };
        let (difficulty, rating) = header.split_once(char::is_whitespace).unwrap_or((header, ""));
        let variant = Variant { index: song.sections.len(), mode: token.to_owned(), difficulty: difficulty.to_owned(), rating: rating.trim().parse().unwrap_or(0) };
        let steps = lines.by_ref().take_while(|line| *line != "end").map(Cow::Borrowed).collect();
        song.sections.push(Section { variant, lines: steps });
    }
    if song.get("bpm").is_none() {
        return Err(invalid("no bpm"));
    }
    Ok(song)
}

/// Beats of a note-length letter (`DanceFile.BEATS`, in quarters of a beat there).
fn beats_of(letter: &str) -> Option<f64> {
    Some(match letter {
        "o" => 4.0,
        "h" => 2.0,
        "q" => 1.0,
        "e" => 0.5,
        "w" => 1.0 / 3.0,
        "s" => 0.25,
        "f" => 1.0 / 6.0,
        "t" => 0.125,
        "u" => 1.0 / 12.0,
        "x" => 0.0625,
        "n" => 1.0 / 48.0,
        _ => return None,
    })
}

impl DanceDecoder {
    /// The step sections of the file, in order.
    ///
    /// # Errors
    /// Returns an error if the file is not UTF-8 text, too large, or has no `bpm`.
    pub fn variants(data: &[u8]) -> RoxResult<Vec<Variant>> {
        Ok(parse(data)?.sections.into_iter().map(|section| section.variant).collect())
    }

    /// Decodes step section `index` (the numbering of [`Self::variants`]).
    ///
    /// # Errors
    /// Returns an error if the file does not parse, has no such section, or the section has no note.
    pub fn decode_variant(data: &[u8], index: usize) -> RoxResult<RoxChart> {
        Self::decode_with_report(data, index).map(|(chart, _)| chart)
    }

    /// [`Self::decode_variant`] and what was dropped.
    ///
    /// # Errors
    /// Same as [`Self::decode_variant`].
    pub fn decode_with_report(data: &[u8], index: usize) -> RoxResult<(RoxChart, DanceReport)> {
        let song = parse(data)?;
        let section = song.sections.get(index).ok_or_else(|| invalid(format!("no step section {index}")))?;
        decode_section(&song, section)
    }
}

impl rox::codec::Decoder for DanceDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        DanceDecoder::decode_variant(data, 0)
    }
}

crate::impl_format!(DanceDecoder, ["dance"]);

/// The time and BPM walk of one section.
struct Walk {
    seconds: f64,
    bpm: f64,
    /// When a stop started and no note has played since: the next notes play then.
    stop_started: Option<f64>,
}

fn decode_section(song: &Song, section: &Section) -> RoxResult<(RoxChart, DanceReport)> {
    let bpm = song.number("bpm").filter(|bpm| *bpm > 0.0).ok_or_else(|| invalid("bpm is not a positive number"))?;
    let start = song.number("startat").unwrap_or(0.0) - song.number("gap").unwrap_or(0.0) / 1000.0;
    let mut report = DanceReport::default();
    let mut walk = Walk { seconds: start, bpm, stop_started: None };
    let mut width = None;
    let mut notes = Vec::new();
    let mut holding: Vec<Option<i64>> = Vec::new();
    let mut tempo = vec![(start, bpm)];
    for line in &section.lines {
        let mut words = line.split_whitespace();
        let command = words.next().unwrap_or("");
        let value = || words.clone().next().and_then(|word| word.parse::<f64>().ok()).filter(|value| value.is_finite());
        if let Some(beats) = beats_of(command) {
            let Some(buttons) = words.next().filter(|buttons| buttons.len() <= MAX_COLUMNS && width.is_none_or(|width| buttons.len() == width)) else {
                report.skipped_lines += 1;
                continue;
            };
            if width.is_none() {
                width = Some(buttons.len());
                holding = vec![None; buttons.len()];
            }
            if buttons.bytes().any(|byte| byte != b'0') {
                let at = walk.stop_started.take().unwrap_or(walk.seconds);
                #[allow(clippy::cast_possible_truncation)] // bounded below
                let time = (at * 1e6).round() as i64;
                if at.abs() > MAX_SECONDS {
                    report.skipped_lines += 1;
                } else {
                    play(buttons, time, &mut holding, &mut notes, &mut report);
                }
            } else {
                walk.stop_started = None;
            }
            walk.seconds += beats * 60.0 / walk.bpm;
            continue;
        }
        match (command, value()) {
            ("D", Some(beats)) => {
                walk.stop_started = None;
                walk.seconds += beats * 60.0 / walk.bpm;
            }
            ("W", Some(seconds)) => {
                walk.stop_started = None;
                walk.seconds += seconds;
            }
            ("B", Some(next)) if next > 0.0 => {
                walk.stop_started = None;
                walk.bpm = next;
                tempo.push((walk.seconds, next));
            }
            ("S", Some(seconds)) => {
                walk.stop_started = Some(walk.seconds);
                walk.seconds += seconds;
            }
            ("R" | "L", _) => {}
            _ => report.skipped_lines += 1,
        }
    }
    report.open_holds = holding.iter().flatten().count();
    let width = width.ok_or_else(|| invalid("no step line"))?;
    let key_count = u8::try_from(width).map_err(|_| invalid("too many columns"))?;
    if notes.is_empty() {
        return Err(invalid("no note"));
    }
    notes.sort_by_key(|note: &Note| (note.time_us, note.column));
    let mut chart = RoxChart::new(key_count);
    chart.notes = notes;
    report.overlapping_notes_dropped = drop_stacked(&mut chart.notes, key_count, |note| note).total();
    let first = chart.notes[0].time_us;
    for (index, (seconds, bpm)) in tempo.into_iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)] // a BPM fits f32; times bounded by the notes'
        let (time_us, bpm) = ((seconds * 1e6).round() as i64, bpm as f32);
        // The first BPM holds from the start of the chart, even before `startat - gap`.
        chart.timing_points.push(TimingPoint::bpm(if index == 0 { time_us.min(first) } else { time_us }, bpm));
    }
    chart.metadata = metadata(song, &section.variant);
    chart.validate()?;
    Ok((chart, report))
}

/// The notes of one button string at `time`.
fn play(buttons: &str, time: i64, holding: &mut [Option<i64>], notes: &mut Vec<Note>, report: &mut DanceReport) {
    for (column, byte) in buttons.bytes().enumerate() {
        let Some(value) = (byte as char).to_digit(10).filter(|value| *value != 0) else {
            if byte != b'0' {
                report.secret_or_unknown += 1;
            }
            continue;
        };
        #[allow(clippy::cast_possible_truncation)] // at most 64 columns
        let lane = column as u8;
        if let Some(start) = holding[column] {
            // Any press of a held column ends its hold and is no note of its own.
            notes.push(Note::hold(start, (time - start).max(1), lane));
            holding[column] = None;
        } else if value & 4 != 0 {
            report.secret_or_unknown += 1;
        } else if value & 2 != 0 {
            holding[column] = Some(time);
        } else if value & 1 != 0 {
            notes.push(Note::tap(time, lane));
        } else {
            report.secret_or_unknown += 1;
        }
    }
}

fn metadata(song: &Song, variant: &Variant) -> Metadata {
    let title = match (song.get("title"), song.get("subtitle")) {
        (Some(title), Some(subtitle)) => format!("{title} {subtitle}"),
        (title, _) => title.unwrap_or_default().to_owned(),
    };
    #[allow(clippy::cast_possible_truncation)] // a preview time in range
    let preview_time_us = song.number("preview").map_or(0, |seconds| (seconds * 1e6).round() as i64);
    Metadata {
        title: title.into(),
        artist: song.get("artist").unwrap_or_default().into(),
        creator: song.get("author").unwrap_or_default().into(),
        difficulty_name: format!("{} {}", variant.mode, variant.difficulty).into(),
        audio_file: song.get("filename").unwrap_or_default().into(),
        background_file: song.get("background").or_else(|| song.get("banner")).map(Into::into),
        preview_time_us,
        ..Metadata::default()
    }
}

/// Decoder of pyDDR `.step` files (pydance's format before `.dance`): several charts per file, read like the `.dance` they became.
///
/// No grammar of the format is published; this reader is the corpus files' (`vsrg-format-docs/vsrg/legacy-dance-formats/step`),
/// matched line for line against the `.dance` pydance converted them to (`6jan`): `version`, `file` (the audio), `song` (title),
/// `group` (artist), `bpm`, `offset` (the `.dance` `gap`, in ms) and `mix` head the file; then sections of a game mode line
/// (`SINGLE`), a `<difficulty> <rating>` line, steps and `end`. A step line is `whole`, `halfn`, `qurtr` or `eight` (4, 2, 1, 1/2 beat)
/// and one two-digit code per panel (`08` a note, `00` none; any other code is not known: dropped and counted); `delay x` lasts `x`
/// beats, `ready` shows "Ready", `lyric` and `trans` show text. Holds were never seen in a `.step`.
pub struct PyddrStepDecoder;

/// Whether the start of a file is a pyDDR `.step` (text whose first line is one of its header keys), not DDR's binary STEP data.
#[must_use]
pub fn looks_like_pyddr_step(head: &[u8]) -> bool {
    let first = head.split(|&byte| byte == b'\n').map(<[u8]>::trim_ascii).find(|line| !line.is_empty());
    first.is_some_and(|line| ["version ", "file ", "song ", "group ", "bpm ", "offset "].iter().any(|key| line.starts_with(key.as_bytes())))
}

impl PyddrStepDecoder {
    /// The step sections of the file, in order.
    ///
    /// # Errors
    /// Returns an error if the file is not UTF-8 text, too large, or has no `bpm`.
    pub fn variants(data: &[u8]) -> RoxResult<Vec<Variant>> {
        Ok(parse_step(data)?.sections.into_iter().map(|section| section.variant).collect())
    }

    /// Decodes step section `index` (the numbering of [`Self::variants`]) and says what was dropped.
    ///
    /// # Errors
    /// Returns an error if the file does not parse, has no such section, or the section has no note.
    pub fn decode_with_report(data: &[u8], index: usize) -> RoxResult<(RoxChart, DanceReport)> {
        let song = parse_step(data)?;
        let section = song.sections.get(index).ok_or_else(|| invalid(format!("no step section {index}")))?;
        decode_section(&song, section)
    }

    /// [`Self::decode_with_report`] without the report.
    ///
    /// # Errors
    /// Same as [`Self::decode_with_report`].
    pub fn decode_variant(data: &[u8], index: usize) -> RoxResult<RoxChart> {
        Self::decode_with_report(data, index).map(|(chart, _)| chart)
    }
}

fn parse_step(data: &[u8]) -> RoxResult<Song<'_>> {
    if data.len() > MAX_FILE_SIZE {
        return Err(invalid(format!("file too large: {} bytes", data.len())));
    }
    let text = std::str::from_utf8(data).map_err(|_| invalid("not UTF-8"))?;
    let mut song = Song { meta: Vec::new(), sections: Vec::new() };
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty()).peekable();
    // The header, up to the first game mode (one upper-case word).
    let is_mode = |line: &str| !line.contains(' ') && line.bytes().all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit());
    while let Some(line) = lines.next_if(|line| !is_mode(line)) {
        let (key, value) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let key = match key {
            "file" => "filename",
            "song" => "title",
            "group" => "artist",
            "offset" => "gap",
            other => other,
        };
        song.meta.push((key, value.trim()));
    }
    while let Some(mode) = lines.next() {
        let Some(header) = lines.next().filter(|line| *line != "end") else { continue };
        let (difficulty, rating) = header.split_once(char::is_whitespace).unwrap_or((header, ""));
        let variant = Variant { index: song.sections.len(), mode: mode.to_owned(), difficulty: difficulty.to_owned(), rating: rating.trim().parse().unwrap_or(0) };
        let steps = lines.by_ref().take_while(|line| *line != "end").map(dance_line).collect();
        song.sections.push(Section { variant, lines: steps });
    }
    if song.get("bpm").is_none() {
        return Err(invalid("no bpm"));
    }
    Ok(song)
}

/// A `.step` line in the `.dance` grammar.
fn dance_line(line: &str) -> Cow<'_, str> {
    let mut words = line.split_whitespace();
    let letter = match words.next() {
        Some("whole") => "o",
        Some("halfn") => "h",
        Some("qurtr") => "q",
        Some("eight") => "e",
        Some("delay") => return Cow::Owned(format!("D {}", words.next().unwrap_or(""))),
        Some("ready") => return Cow::Borrowed("R"),
        Some("lyric" | "trans") => return Cow::Borrowed("L"),
        _ => return Cow::Borrowed(line),
    };
    let buttons: String = words.map(|code| match code {
        "00" => '0',
        "08" => '1',
        _ => 'x',
    }).collect();
    Cow::Owned(format!("{letter} {buttons}"))
}

#[cfg(test)]
mod tests;
