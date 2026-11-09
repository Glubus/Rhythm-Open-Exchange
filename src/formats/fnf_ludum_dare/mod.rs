#![warn(clippy::pedantic)]
//! Friday Night Funkin' as made for Ludum Dare 47 (the game jam build, tag `1.0.0` of FunkinCrew/Funkin): `<song>.json` names the song,
//! its BPM and its number of sections; each section is a bitmap, `<song>_section<N>.png`, whose pixels are the notes. Decode only.
//!
//! Format notes: `vsrg-format-docs/vsrg/friday-night-funkin/ludum-dare-png`, from the game's own code at that tag (`ChartParser.hx`,
//! `PlayState.generateSong`, `Note.hx`; read for facts), checked on the corpus songs.
//!
//! - **Which file**: a `.json` holding exactly a string `song`, a number `bpm` and a number `sections` ([`looks_like_ludum_dare`]); the
//!   legacy charts that came after hold an object under `song`.
//! - **Bitmaps**: 8 pixels wide. Each row gives one value per marked (non-white) pixel, or a 0 when none is: columns 0-3 a note of
//!   lane 1-4, columns 4-7 a hold piece of lane 1-4. The values follow one another: the n-th value of a section is its step n (a row
//!   with two marks pushes the next rows one step later, as the game does).
//! - **Time**: one BPM, a step is a quarter of a beat. A section of `n` values lasts `n / 4` beats rounded up to 4 or to 8 (longer as is);
//!   the opponent plays it, then the player plays the same notes that many beats later; the next section starts after both. Only the
//!   player's notes are in the chart.
//! - **Lanes**: the game draws lane 1 third, 2 fourth, 3 second and 4 first: the columns, left to right, are lanes 4, 3, 1, 2.
//! - **Holds**: a hold piece extends the last note to its time when that note is on the same lane; another piece is dropped and counted.
//! - **Files around it** ([`decode_file`]): the bitmaps beside the `.json`; the music, `assets/music/<song>_Inst.mp3` of the game's tree
//!   (`../../music/` from the chart's folder) when it is there. The vocals (`<song>_Voices.mp3`) are not mixed in.

mod png;

use std::path::Path;

use rox::error::{RoxError, RoxResult};
use rox::model::{Note, RoxChart, TimingPoint, drop_stacked};
use serde::Deserialize;

/// Files above this size are not a song's `.json` (it holds three fields).
const MAX_META_SIZE: u64 = 64 * 1024;
/// Most sections read.
const MAX_SECTIONS: u64 = 10_000;
const WIDTH: usize = 8;
/// The column, left to right, of the game's lanes 1 to 4.
const COLUMN_OF_LANE: [u8; 4] = [2, 3, 1, 0];

/// What the decoder dropped. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LudumDareReport {
    /// Hold pieces that do not follow a note of their lane: dropped.
    pub orphan_hold_pieces: usize,
    /// Notes on another note of their column (same time, or inside a hold): dropped.
    pub overlapping_notes_dropped: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Meta {
    song: String,
    bpm: f64,
    sections: u64,
}

fn invalid(reason: impl std::fmt::Display) -> RoxError {
    RoxError::InvalidFormat(format!("fnf ludum dare: {reason}"))
}

fn meta(data: &[u8]) -> Option<Meta> {
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    serde_json::from_slice::<Meta>(data).ok().filter(|meta| meta.bpm.is_finite() && meta.bpm > 0.0 && (1..=MAX_SECTIONS).contains(&meta.sections))
}

/// Whether a `.json`'s bytes are a Ludum Dare song file: `song`, `bpm` and `sections` and nothing else.
#[must_use]
pub fn looks_like_ludum_dare(data: &[u8]) -> bool {
    meta(data).is_some()
}

/// Whether the file at `path` is a Ludum Dare song file with its first section bitmap beside it; the file is read.
#[must_use]
pub fn is_ludum_dare_chart_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        && std::fs::metadata(path).is_ok_and(|file| file.len() <= MAX_META_SIZE)
        && std::fs::read(path).ok().and_then(|data| meta(&data)).is_some_and(|meta| path.with_file_name(section_name(&meta.song, 1)).is_file())
}

fn section_name(song: &str, section: u64) -> String {
    format!("{}_section{section}.png", song.to_lowercase())
}

/// Decoder of a Ludum Dare song from its `.json` and the bytes of its section bitmaps, in order.
pub struct LudumDareDecoder;

impl LudumDareDecoder {
    /// Decodes a song and says what was dropped.
    ///
    /// # Errors
    /// Returns an error if the `.json` is not a Ludum Dare song file, a bitmap is not one, or the player has no note.
    pub fn decode_with_report(song: &[u8], sections: &[Vec<u8>]) -> RoxResult<(RoxChart, LudumDareReport)> {
        let meta = meta(song).ok_or_else(|| invalid("not a song file (song, bpm, sections)"))?;
        let values = sections
            .iter()
            .enumerate()
            .map(|(index, bitmap)| section_values(bitmap).map_err(|reason| invalid(format!("section {}: {reason}", index + 1))))
            .collect::<RoxResult<Vec<_>>>()?;
        let mut report = LudumDareReport::default();
        let mut chart = RoxChart::new(4);
        chart.notes = player_notes(&values, meta.bpm, &mut report);
        if chart.notes.is_empty() {
            return Err(invalid("no note"));
        }
        report.overlapping_notes_dropped = drop_stacked(&mut chart.notes, 4, |note| note).total();
        #[allow(clippy::cast_possible_truncation)] // a BPM fits f32
        chart.timing_points.push(TimingPoint::bpm(0, meta.bpm as f32));
        chart.metadata.title = meta.song.as_str().into();
        chart.validate()?;
        Ok((chart, report))
    }
}

/// The values of one section, as `ChartParser.parse` pushes them: per row, each marked pixel's (a lane 1-4, or -1 to -4 for a hold
/// piece), or 0 when the row has none.
fn section_values(bitmap: &[u8]) -> Result<Vec<i8>, String> {
    let marks = png::marks(bitmap)?;
    if marks.width != WIDTH {
        return Err(format!("{} pixels wide, not {WIDTH}", marks.width));
    }
    let mut values = Vec::with_capacity(marks.rows.len());
    for row in &marks.rows {
        let before = values.len();
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // 8 columns
        values.extend(row.iter().enumerate().filter(|(_, marked)| **marked).map(|(column, _)| if column < 4 { column as i8 + 1 } else { 3 - column as i8 }));
        if values.len() == before {
            values.push(0);
        }
    }
    Ok(values)
}

/// The player's notes of every section (`PlayState.generateSong`, second pass), holds joined.
fn player_notes(sections: &[Vec<i8>], bpm: f64, report: &mut LudumDareReport) -> Vec<Note> {
    let beat_ms = 60_000.0 / bpm;
    let step_ms = beat_ms / 4.0;
    let mut notes: Vec<Note> = Vec::new();
    let mut eights = 0u64;
    for values in sections {
        let beats = match values.len() / 4 {
            0..=4 => 4,
            5..=8 => 8,
            longer => longer,
        };
        #[allow(clippy::cast_precision_loss)] // small counts
        let start_ms = beat_ms * 8.0 * eights as f64 + beat_ms * beats as f64;
        for (step, &value) in values.iter().enumerate() {
            if value == 0 {
                continue;
            }
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)] // small counts; bounded times
            let time_us = ((start_ms + step_ms * step as f64) * 1000.0).round() as i64;
            let column = COLUMN_OF_LANE[usize::from(value.unsigned_abs()) - 1];
            if value > 0 {
                notes.push(Note::tap(time_us, column));
            } else {
                match notes.last_mut().filter(|last| last.column == column && last.time_us < time_us) {
                    Some(last) => *last = Note::hold(last.time_us, time_us - last.time_us, column),
                    None => report.orphan_hold_pieces += 1,
                }
            }
        }
        // `Math.round(coolSection / 4)`: sections of 8 beats and more take two slots or more.
        eights += (beats as u64 + 2) / 4;
    }
    notes
}

/// Reads a song file with the section bitmaps beside it (see the module notes): `data` is the `.json`'s bytes, `path` its path.
///
/// # Errors
/// Returns an error if the `.json` is not a song file, a bitmap is missing or unreadable, or the player has no note.
pub fn decode_file(path: &Path, data: &[u8]) -> RoxResult<RoxChart> {
    let meta = meta(data).ok_or_else(|| invalid("not a song file (song, bpm, sections)"))?;
    let sections = (1..=meta.sections)
        .map(|section| {
            let bitmap = path.with_file_name(section_name(&meta.song, section));
            std::fs::read(&bitmap).map_err(|e| invalid(format!("{}: {e}", bitmap.display())))
        })
        .collect::<RoxResult<Vec<_>>>()?;
    let (mut chart, _) = LudumDareDecoder::decode_with_report(data, &sections)?;
    let music = format!("{}_Inst.mp3", meta.song);
    if path.parent().and_then(Path::parent).and_then(Path::parent).is_some_and(|assets| assets.join("music").join(&music).is_file()) {
        chart.metadata.audio_file = format!("../../music/{music}").into();
    }
    Ok(chart)
}

#[cfg(test)]
mod tests;
