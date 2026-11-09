#![warn(clippy::pedantic)]
//! fluXis maps (`.fsc`): one difficulty per file, a JSON `MapInfo`. Decode only.
//!
//! Format notes: `vsrg-format-docs/vsrg/fluxis/fsc`, from the game's C# classes (`MapInfo`, `HitObject`, `TimingPoint`, `ScrollVelocity`;
//! fluXis, MIT, read for facts), checked on the corpus maps.
//!
//! - **Columns**: as many as the highest `lane` (1-based), as the game counts its key mode; lane `n` is column `n - 1`. A dual map
//!   (`dual: 2`, two playfields) keeps all its lanes side by side.
//! - **Notes**: `type 0` with a `holdtime` above 0 is a hold of that many milliseconds, otherwise a tap; `type 2` (landmine) a mine;
//!   `type 1` (tick: judged while its key is held, as the notes on a slide's path) a tap, counted in [`FscReport::ticks`]. Hidden notes are
//!   kept (only their drawing differs). Hitsounds, scroll groups and visual lanes are not read.
//! - **Time**: milliseconds from the start of the audio. `TimingPoints` give the BPM and the beats per measure, `ScrollVelocities` the
//!   scroll speed (per-group velocities apply to every lane here).
//! - **Metadata**: `metadata.title`, `artist`, `mapper`, `difficulty`, `previewtime`, `tags`; `AudioFile`, `BackgroundFile` (else
//!   `CoverFile`). The effects file (`.ffx`) and the storyboard are not read.

use std::collections::HashMap;

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, Note, RoxChart, TimingPoint, drop_stacked};
use serde::Deserialize;

/// Decoder of fluXis `.fsc` maps.
pub struct FscDecoder;
crate::impl_format!(FscDecoder, ["fsc"]);

/// Files above this size are refused (a real map is well under a megabyte).
const MAX_FILE_SIZE: usize = 64 * 1024 * 1024;
/// Most columns read (a lane above is dropped and counted).
const MAX_LANES: u8 = 64;
/// Times further than this many milliseconds from the start of the audio are not read.
const MAX_MS: f64 = 1e10;

/// What the decoder dropped or changed. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FscReport {
    /// Tick notes read as taps (the game judges them while their key is held).
    pub ticks: usize,
    /// Notes of a lane below 1 or above 64, of an unknown type, or with a time that is not a number in range: dropped.
    pub invalid_notes: usize,
    /// Timing points with a BPM or a signature that is not above 0: ignored.
    pub invalid_timing_points: usize,
    /// Notes on another note of their column (same time, or inside a hold): dropped.
    pub overlapping_notes_dropped: usize,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct MapInfo {
    #[serde(rename = "AudioFile")]
    audio_file: String,
    #[serde(rename = "BackgroundFile")]
    background_file: String,
    #[serde(rename = "CoverFile")]
    cover_file: String,
    metadata: FscMetadata,
    #[serde(rename = "HitObjects")]
    hit_objects: Vec<HitObject>,
    #[serde(rename = "TimingPoints")]
    timing_points: Vec<FscTiming>,
    #[serde(rename = "ScrollVelocities")]
    scroll_velocities: Vec<ScrollVelocity>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FscMetadata {
    title: String,
    artist: String,
    mapper: String,
    difficulty: String,
    tags: String,
    previewtime: i64,
}

#[derive(Deserialize)]
struct HitObject {
    time: f64,
    lane: i64,
    #[serde(default)]
    holdtime: f64,
    #[serde(default, rename = "type")]
    kind: i64,
}

#[derive(Deserialize)]
struct FscTiming {
    time: f64,
    #[serde(default = "default_bpm")]
    bpm: f64,
    #[serde(default = "default_signature")]
    signature: i64,
}

#[derive(Deserialize)]
struct ScrollVelocity {
    time: f64,
    #[serde(default = "default_multiplier")]
    multiplier: f64,
}

fn default_bpm() -> f64 {
    120.0
}
fn default_signature() -> i64 {
    4
}
fn default_multiplier() -> f64 {
    1.0
}

impl Decoder for FscDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode(data).map(|(chart, _)| chart)
    }
}

impl FscDecoder {
    /// Decodes a map and says what was dropped.
    ///
    /// # Errors
    /// Returns an error if the file is too large, not a fluXis map, or has no playable note.
    pub fn decode_with_report(data: &[u8]) -> RoxResult<(RoxChart, FscReport)> {
        decode(data)
    }
}

fn invalid(reason: impl std::fmt::Display) -> RoxError {
    RoxError::InvalidFormat(format!("fsc: {reason}"))
}

/// Milliseconds to microseconds; `None` for a time that is not a number in range.
fn us(ms: f64) -> Option<i64> {
    #[allow(clippy::cast_possible_truncation)] // bounded just before
    (ms.is_finite() && ms.abs() <= MAX_MS).then(|| (ms * 1000.0).round() as i64)
}

fn decode(data: &[u8]) -> RoxResult<(RoxChart, FscReport)> {
    if data.len() > MAX_FILE_SIZE {
        return Err(invalid(format!("file too large: {} bytes", data.len())));
    }
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let map: MapInfo = serde_json::from_slice(data).map_err(invalid)?;
    let mut report = FscReport::default();
    let notes: Vec<Note> = map.hit_objects.iter().filter_map(|object| note_of(object, &mut report)).collect();
    let key_count = notes.iter().map(|note| note.column + 1).max().ok_or_else(|| invalid("no playable note"))?;
    let mut chart = RoxChart::new(key_count);
    chart.notes = notes;
    report.overlapping_notes_dropped = drop_stacked(&mut chart.notes, key_count, |note| note).total();
    chart.timing_points = timing_points(&map, &mut report);
    if let Some(first) = chart.notes.first().map(|note| note.time_us)
        && chart.timing_points.first().is_none_or(|point| point.time_us() > first)
    {
        // The model wants a BPM no later than the first note: the first one goes on backwards (the game's default is 120).
        let bpm = chart.timing_points.iter().find_map(TimingPoint::bpm_value);
        chart.timing_points.insert(0, TimingPoint::bpm(first.min(0), bpm.unwrap_or(120.0)));
    }
    chart.metadata = metadata(&map);
    chart.validate()?;
    Ok((chart, report))
}

fn note_of(object: &HitObject, report: &mut FscReport) -> Option<Note> {
    let column = u8::try_from(object.lane - 1).ok().filter(|&column| column < MAX_LANES);
    let (Some(column), Some(start)) = (column, us(object.time)) else {
        report.invalid_notes += 1;
        return None;
    };
    match object.kind {
        0 => match us(object.holdtime).filter(|&length| length > 0) {
            Some(length) => Some(Note::hold(start, length, column)),
            None => Some(Note::tap(start, column)),
        },
        1 => {
            report.ticks += 1;
            Some(Note::tap(start, column))
        }
        2 => Some(Note::mine(start, column)),
        _ => {
            report.invalid_notes += 1;
            None
        }
    }
}

/// BPM points (one per change of BPM or signature) and scroll velocities, by time.
fn timing_points(map: &MapInfo, report: &mut FscReport) -> Vec<TimingPoint> {
    let mut points = Vec::new();
    let mut last: Option<(u32, u8)> = None;
    let mut bpms: Vec<&FscTiming> = map.timing_points.iter().collect();
    bpms.sort_by(|a, b| a.time.total_cmp(&b.time));
    for point in bpms {
        let signature = u8::try_from(point.signature).ok().filter(|&s| s > 0);
        let (Some(time_us), Some(signature), true) = (us(point.time), signature, point.bpm.is_finite() && point.bpm > 0.0) else {
            report.invalid_timing_points += 1;
            continue;
        };
        #[allow(clippy::cast_possible_truncation)] // a BPM fits f32
        let bpm = point.bpm as f32;
        if last != Some((bpm.to_bits(), signature)) {
            points.push(TimingPoint::Bpm { time_us, bpm, signature });
            last = Some((bpm.to_bits(), signature));
        }
    }
    // One velocity per time: the last written wins.
    let mut velocities: HashMap<i64, f64> = HashMap::new();
    for velocity in &map.scroll_velocities {
        if let Some(time_us) = us(velocity.time).filter(|_| velocity.multiplier.is_finite()) {
            velocities.insert(time_us, velocity.multiplier);
        }
    }
    #[allow(clippy::cast_possible_truncation)] // a multiplier fits f32
    points.extend(velocities.into_iter().map(|(time_us, multiplier)| TimingPoint::sv(time_us, multiplier as f32)));
    points.sort_by_key(|point| (point.time_us(), u8::from(matches!(point, TimingPoint::Sv { .. }))));
    points
}

fn metadata(map: &MapInfo) -> Metadata {
    let meta = &map.metadata;
    let background = if map.background_file.is_empty() { &map.cover_file } else { &map.background_file };
    Metadata {
        title: meta.title.as_str().into(),
        artist: meta.artist.as_str().into(),
        creator: meta.mapper.as_str().into(),
        difficulty_name: meta.difficulty.as_str().into(),
        audio_file: map.audio_file.as_str().into(),
        background_file: Some(background.as_str()).filter(|name| !name.is_empty()).map(Into::into),
        preview_time_us: meta.previewtime.saturating_mul(1000),
        tags: meta.tags.split([' ', ',']).filter(|tag| !tag.is_empty()).map(Into::into).collect(),
        ..Metadata::default()
    }
}

#[cfg(test)]
mod tests;
