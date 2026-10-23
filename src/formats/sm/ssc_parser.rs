#![warn(clippy::pedantic)]
//! `.ssc` (StepMania 5 / Etterna) parser: the same `#TAG:value;` syntax as `.sm`, but every
//! chart is a `#NOTEDATA:;` block of its own tags (`#STEPSTYPE`, `#DESCRIPTION`, `#DIFFICULTY`,
//! `#METER`, `#CREDIT`, `#NOTES`/`#NOTES2` holding only the measures) that may carry its own
//! `#BPMS`, `#STOPS`, `#DELAYS`, `#WARPS`, `#FAKES` and `#OFFSET`. Each of those, when the chart
//! lacks it, is the file's.
//!
//! Supported: `dance-single` (4 columns), `dance-solo` (6), `dance-double` (8), `pump-single` (5),
//! `pump-halfdouble` (6) and `kb7-single` (7); other step types are skipped with a message in
//! [`SmFile::skipped`]. Notes: `1` tap, `2` hold head, `3` hold/roll tail, `4` roll head, `M`
//! mine, `L` lift (a tap), `F` fake and `K` keysound (not played). Stops, delays and BPM changes
//! move the notes in time; warped notes and notes inside `#FAKES` (and the holds starting there)
//! are dropped. A note sits on a row of 1/48 beat (a 192nd of a measure), like in the game.
//! Not read: `#SPEEDS`, `#SCROLLS` (visual speed), `#TIMESIGNATURES` (display only: a measure
//! is always 4 beats), `#LABELS`, `#COMBOS`, `#TICKCOUNTS`, `#ATTACKS`. A chart with a zero or
//! negative BPM or a negative stop, delay or warp (the old `.sm` warp encoding) is refused with
//! a message in [`SmFile::skipped`], never played with the entry ignored.
#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use rox::error::{RoxError, RoxResult};

use super::msd::{Tag, tokenize};
use super::timing_data::TimingData;
use super::types::timing::ROWS_PER_BEAT;
use super::types::{SmChart, SmFile, SmMetadata, SmNote, SmNoteType};

const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Column count of a step type this crate can play, `None` for the others.
#[must_use]
pub fn columns_of(stepstype: &str) -> Option<u8> {
    match stepstype.trim().to_ascii_lowercase().as_str() {
        "dance-single" => Some(4),
        "pump-single" => Some(5),
        "dance-solo" | "pump-halfdouble" => Some(6),
        "kb7-single" => Some(7),
        "dance-double" => Some(8),
        _ => None,
    }
}

/// Parses an `.ssc` file (UTF-8, with or without a BOM; undecodable bytes become `�` in the text
/// fields and never touch the notes).
///
/// # Errors
/// Returns an error if the data exceeds 100 MB or has no `#NOTEDATA` chart. Charts that can not
/// be played are left out and described in [`SmFile::skipped`].
pub fn parse(data: &[u8]) -> RoxResult<SmFile> {
    if data.len() > MAX_FILE_SIZE {
        return Err(RoxError::InvalidFormat(format!(
            "File too large: {} bytes (max 100MB)",
            data.len()
        )));
    }
    let text = String::from_utf8_lossy(data);
    let tags = tokenize(text.trim_start_matches('\u{feff}'));
    let first_chart = tags
        .iter()
        .position(|t| t.name == "NOTEDATA")
        .ok_or_else(|| {
            RoxError::InvalidFormat(
                "No #NOTEDATA chart found (an .ssc file holds its charts in #NOTEDATA blocks)"
                    .to_string(),
            )
        })?;
    let song = &tags[..first_chart];
    let mut file = SmFile {
        metadata: metadata(song),
        ..SmFile::default()
    };
    file.offset_us = offset_us(song).unwrap_or(0);
    let song_timing = SongTiming::of(song);
    file.bpms = song_timing.bpm_points();
    let mut blocks: Vec<&[Tag]> = Vec::new();
    let mut start = first_chart;
    for (index, tag) in tags.iter().enumerate().skip(first_chart + 1) {
        if tag.name == "NOTEDATA" {
            blocks.push(&tags[start + 1..index]);
            start = index;
        }
    }
    blocks.push(&tags[start + 1..]);
    for (index, block) in blocks.into_iter().enumerate() {
        match chart(block, &song_timing) {
            Ok(chart) => file.charts.push(chart),
            Err(reason) => file.skipped.push(format!("chart {}: {reason}", index + 1)),
        }
    }
    Ok(file)
}

/// Last tag of that name (the loader reads tags in order, so the last one wins).
fn find<'a>(tags: &'a [Tag], name: &str) -> Option<&'a Tag> {
    tags.iter().rev().find(|t| t.name == name)
}

fn text(tags: &[Tag], name: &str) -> String {
    find(tags, name).map(Tag::text).unwrap_or_default()
}

fn number(tags: &[Tag], name: &str) -> Option<f64> {
    find(tags, name)?.first().parse().ok()
}

pub(super) fn metadata(song: &[Tag]) -> SmMetadata {
    SmMetadata {
        title: text(song, "TITLE"),
        artist: text(song, "ARTIST"),
        credit: text(song, "CREDIT"),
        music: text(song, "MUSIC"),
        banner: text(song, "BANNER"),
        background: text(song, "BACKGROUND"),
        sample_start: number(song, "SAMPLESTART").unwrap_or(0.0),
        sample_length: number(song, "SAMPLELENGTH").unwrap_or(0.0),
    }
}

pub(super) fn offset_us(tags: &[Tag]) -> Option<i64> {
    number(tags, "OFFSET")
        .filter(|offset| offset.is_finite())
        .map(|offset| {
            #[allow(clippy::cast_possible_truncation)]
            // seconds→µs i64: safe for any realistic offset
            let us = (offset * 1_000_000.0).round() as i64;
            us
        })
}

/// `beat=value,beat=value` pairs, `None` when the tag is absent.
fn pairs(tags: &[Tag], name: &str) -> Option<Vec<(f64, f64)>> {
    let tag = find(tags, name)?;
    let mut result = Vec::new();
    for pair in tag.text().split(',') {
        if let Some((beat, value)) = pair.split_once('=')
            && let (Ok(beat), Ok(value)) = (beat.trim().parse::<f64>(), value.trim().parse::<f64>())
        {
            result.push((beat, value));
        }
    }
    Some(result)
}

/// The timing tags of the song header: what a chart without its own falls back to.
#[derive(Default)]
pub(super) struct SongTiming {
    bpms: Option<Vec<(f64, f64)>>,
    stops: Option<Vec<(f64, f64)>>,
    delays: Option<Vec<(f64, f64)>>,
    warps: Option<Vec<(f64, f64)>>,
    fakes: Option<Vec<(f64, f64)>>,
}

impl SongTiming {
    pub(super) fn of(tags: &[Tag]) -> Self {
        Self {
            bpms: pairs(tags, "BPMS"),
            stops: pairs(tags, "STOPS"),
            delays: pairs(tags, "DELAYS"),
            warps: pairs(tags, "WARPS"),
            fakes: pairs(tags, "FAKES"),
        }
    }

    /// The song's timeline with the chart's tags laid over it. A chart with BPMs of its own has
    /// split timing: its stops, delays, warps and fakes are its own too, even empty.
    ///
    /// # Errors
    /// A timing value the timeline can not honour (no BPM, a zero or negative BPM, a negative
    /// stop, delay or warp: the old `.sm` warp encoding) refuses the chart: ignoring it would
    /// shift every note after it without a word.
    pub(super) fn timing_for(
        &self,
        chart: &[Tag],
    ) -> Result<(Vec<(f64, f64)>, TimingData), String> {
        let split = pairs(chart, "BPMS").is_some_and(|own| !own.is_empty());
        let pick = |name: &str, song: &Option<Vec<(f64, f64)>>| match pairs(chart, name) {
            Some(own) if split || !own.is_empty() => own,
            _ => song.clone().unwrap_or_default(),
        };
        let bpms = pick("BPMS", &self.bpms);
        if bpms.is_empty() {
            return Err("no valid #BPMS".to_string());
        }
        let stops = pick("STOPS", &self.stops);
        let delays = pick("DELAYS", &self.delays);
        let warps = pick("WARPS", &self.warps);
        let fakes = pick("FAKES", &self.fakes);
        if let Some(&(beat, bpm)) = bpms
            .iter()
            .find(|&&(beat, bpm)| !beat.is_finite() || !bpm.is_finite() || bpm <= 0.0)
        {
            return Err(format!(
                "unsupported #BPMS entry {beat}={bpm} (zero, negative or not finite: old warp encoding)"
            ));
        }
        for (name, entries) in [
            ("STOPS", &stops),
            ("DELAYS", &delays),
            ("WARPS", &warps),
            ("FAKES", &fakes),
        ] {
            if let Some(&(beat, value)) = entries
                .iter()
                .find(|&&(beat, value)| !beat.is_finite() || !value.is_finite() || value < 0.0)
            {
                return Err(format!(
                    "unsupported #{name} entry {beat}={value} (negative or not finite: old warp encoding)"
                ));
            }
        }
        let timing = TimingData::new(&bpms, &stops, &delays, &warps, &fakes);
        Ok((bpms, timing))
    }

    /// BPM changes of the song's header as timing points (empty without `#BPMS`).
    pub(super) fn bpm_points(&self) -> Vec<(i64, f32)> {
        let Some(bpms) = &self.bpms else {
            return Vec::new();
        };
        let timing = TimingData::new(
            bpms,
            self.stops.as_deref().unwrap_or(&[]),
            self.delays.as_deref().unwrap_or(&[]),
            self.warps.as_deref().unwrap_or(&[]),
            &[],
        );
        bpm_points(bpms, &timing)
    }
}

/// BPM changes as `(time_us, bpm)`: the first at time 0, one per time (the last wins).
fn bpm_points(bpms: &[(f64, f64)], timing: &TimingData) -> Vec<(i64, f32)> {
    let mut sorted: Vec<(f64, f64)> = bpms
        .iter()
        .copied()
        .filter(|&(b, v)| b.is_finite() && v.is_finite() && v > 0.0)
        .collect();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut points: Vec<(i64, f32)> = Vec::new();
    for (index, &(beat, bpm)) in sorted.iter().enumerate() {
        let time = if index == 0 {
            0
        } else {
            timing.arrival_us(beat)
        };
        #[allow(clippy::cast_possible_truncation)] // BPM fits f32
        let bpm = bpm as f32;
        match points.last_mut() {
            Some(last) if last.0 == time => last.1 = bpm,
            _ => points.push((time, bpm)),
        }
    }
    points
}

pub(super) fn chart(tags: &[Tag], song: &SongTiming) -> Result<SmChart, String> {
    let stepstype = text(tags, "STEPSTYPE").to_ascii_lowercase();
    let columns = columns_of(&stepstype).ok_or_else(|| {
        if stepstype.is_empty() {
            "no #STEPSTYPE".to_string()
        } else {
            format!("unsupported step type `{stepstype}`")
        }
    })?;
    let notes = tags
        .iter()
        .rev()
        .find(|t| (t.name == "NOTES" || t.name == "NOTES2") && !t.text().is_empty())
        .ok_or_else(|| format!("{stepstype}: no #NOTES data"))?;
    let (bpms, timing) = song
        .timing_for(tags)
        .map_err(|reason| format!("{stepstype}: {reason}"))?;
    let data = notes.text();
    let rows = measures(&data, columns).map_err(|reason| format!("{stepstype}: {reason}"))?;
    let meter = find(tags, "METER").map_or(1, |t| {
        t.first()
            .parse::<u32>()
            .ok()
            .or_else(|| {
                t.first().parse::<f64>().ok().map(|m| {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    // meters are small and non-negative
                    let meter = m.max(0.0) as u32;
                    meter
                })
            })
            .unwrap_or(1)
    });
    let credit = text(tags, "CREDIT");
    Ok(SmChart {
        stepstype,
        difficulty: text(tags, "DIFFICULTY"),
        meter,
        column_count: columns,
        notes: place(&rows, columns, &timing),
        description: text(tags, "DESCRIPTION"),
        credit: (!credit.is_empty()).then_some(credit),
        bpms: Some(bpm_points(&bpms, &timing)),
        offset_us: offset_us(tags),
    })
}

/// One row of a measure: its beat and its characters.
struct Row<'a> {
    beat: f64,
    cells: &'a str,
}

/// Splits note data into rows with their beats (a measure is 4 beats, its rows evenly spaced).
fn measures(data: &str, columns: u8) -> Result<Vec<Row<'_>>, String> {
    let mut rows = Vec::new();
    for (measure, block) in data.split(',').enumerate() {
        let lines: Vec<&str> = block
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        for (index, cells) in lines.iter().enumerate() {
            if cells.chars().count() != usize::from(columns) {
                return Err(format!(
                    "measure {}: row `{cells}` has {} columns, expected {columns}",
                    measure + 1,
                    cells.chars().count()
                ));
            }
            #[allow(clippy::cast_precision_loss)] // measure and row counts are far below 2^52
            let exact = measure as f64 * 4.0 + index as f64 * 4.0 / lines.len() as f64;
            // StepMania stores a note on a row of 1/48 beat (a 192nd of a measure): measures
            // with another line count are rounded to the nearest row, as the game does.
            let beat = (exact * ROWS_PER_BEAT).round() / ROWS_PER_BEAT;
            rows.push(Row { beat, cells });
        }
    }
    Ok(rows)
}

/// Turns rows into timed notes, dropping warped and fake ones (a hold lives or dies by its head).
fn place(rows: &[Row<'_>], columns: u8, timing: &TimingData) -> Vec<SmNote> {
    let mut notes = Vec::new();
    // Per column, whether the hold being read was kept.
    let mut open: Vec<Option<bool>> = vec![None; usize::from(columns)];
    for row in rows {
        let playable = !timing.is_warped(row.beat) && !timing.is_fake(row.beat);
        let time_us = timing.note_us(row.beat);
        for (column, cell) in row.cells.chars().enumerate() {
            let note_type = match cell {
                '1' => SmNoteType::Tap,
                'L' | 'l' => SmNoteType::Lift,
                'M' | 'm' => SmNoteType::Mine,
                '2' => SmNoteType::HoldHead,
                '4' => SmNoteType::RollHead,
                '3' => {
                    if open[column].take() == Some(true) {
                        notes.push(note(time_us, column, SmNoteType::Tail));
                    }
                    continue;
                }
                _ => continue,
            };
            if matches!(note_type, SmNoteType::HoldHead | SmNoteType::RollHead) {
                open[column] = Some(playable);
            }
            if playable {
                notes.push(note(time_us, column, note_type));
            }
        }
    }
    notes
}

fn note(time_us: i64, column: usize, note_type: SmNoteType) -> SmNote {
    #[allow(clippy::cast_possible_truncation)] // a row has at most `columns` (≤ 8) cells
    SmNote {
        time_us,
        column: column as u8,
        note_type,
    }
}
