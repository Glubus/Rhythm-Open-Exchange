#![warn(clippy::pedantic)]
//! The two JSON files as serde types, and the checks that do not depend on a difficulty: size, version, time format,
//! the tempo map. Every field the decoder ignores is left out of the structs (serde skips unknown keys).

use std::collections::BTreeMap;

use rox::error::{RoxError, RoxResult};
use rox::model::TimingPoint;
use serde::Deserialize;
use serde::de::IgnoredAny;

/// A JSON file larger than this is refused (real charts are a few hundred KiB).
pub(super) const MAX_JSON_BYTES: usize = 64 * 1024 * 1024;
/// A time or a length above this many milliseconds (about 11.6 days) is out of range; it keeps every µs value far from `i64` overflow.
const MAX_MS: f64 = 1.0e9;
/// A tempo above this is refused.
const MAX_BPM: f64 = 1.0e6;

pub(super) fn bad(message: impl AsRef<str>) -> RoxError {
    RoxError::InvalidFormat(format!("fnf v-slice: {}", message.as_ref()))
}

/// Milliseconds to microseconds; `None` when not finite or beyond ±`MAX_MS`.
pub(super) fn ms_to_us(ms: f64) -> Option<i64> {
    if ms.is_finite() && ms.abs() <= MAX_MS {
        #[allow(clippy::cast_possible_truncation)] // bounded by MAX_MS above
        Some((ms * 1000.0).round() as i64)
    } else {
        None
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct ChartFile {
    pub version: Option<String>,
    #[serde(default, rename = "scrollSpeed")]
    pub scroll_speed: BTreeMap<String, f64>,
    #[serde(default)]
    pub events: Vec<IgnoredAny>,
    #[serde(default)]
    pub notes: BTreeMap<String, Vec<NoteData>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct NoteData {
    pub t: f64,
    pub d: i64,
    #[serde(default)]
    pub l: f64,
    #[serde(default)]
    pub k: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MetadataFile {
    pub version: Option<String>,
    pub song_name: Option<String>,
    pub artist: Option<String>,
    pub charter: Option<String>,
    pub time_format: Option<String>,
    pub offsets: Option<Offsets>,
    pub play_data: Option<PlayData>,
    pub time_changes: Option<Vec<TimeChange>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Offsets {
    pub instrumental: Option<f64>,
    pub alt_instrumentals: Option<BTreeMap<String, f64>>,
    pub vocals: Option<BTreeMap<String, f64>>,
    pub alt_vocals: Option<BTreeMap<String, BTreeMap<String, f64>>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PlayData {
    pub characters: Option<Characters>,
    pub ratings: Option<BTreeMap<String, f64>>,
    pub preview_start: Option<f64>,
    pub preview_end: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Characters {
    pub player: Option<String>,
    pub opponent: Option<String>,
    pub instrumental: Option<String>,
    pub player_vocals: Option<Vec<String>>,
    pub opponent_vocals: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TimeChange {
    pub t: f64,
    pub bpm: Option<f64>,
    pub n: Option<i64>,
    pub d: Option<i64>,
}

pub(super) fn parse_json<'a, T: Deserialize<'a>>(bytes: &'a [u8], what: &str) -> RoxResult<T> {
    if bytes.len() > MAX_JSON_BYTES {
        return Err(bad(format!(
            "{what} is {} bytes, more than the {MAX_JSON_BYTES} allowed",
            bytes.len()
        )));
    }
    serde_json::from_slice(bytes).map_err(|e| bad(format!("{what} JSON: {e}")))
}

/// `major.minor` of a semver string (`2.2.4`, `2.0.0-rc.1`): the loader of the game takes `2.0.x` charts and `2.2.x` metadata.
fn check_version(version: Option<&str>, what: &str, major: u64, minor: u64) -> RoxResult<()> {
    let Some(text) = version else {
        return Err(bad(format!(
            "{what} has no `version` (expected {major}.{minor}.x)"
        )));
    };
    let core = text.trim().split(['-', '+']).next().unwrap_or("");
    let mut parts = core.split('.');
    let parsed = (
        parts.next().and_then(|p| p.parse::<u64>().ok()),
        parts.next().and_then(|p| p.parse::<u64>().ok()),
        parts.next().and_then(|p| p.parse::<u64>().ok()),
    );
    match parsed {
        (Some(ma), Some(mi), Some(_)) if parts.next().is_none() && ma == major && mi == minor => {
            Ok(())
        }
        _ => Err(bad(format!(
            "{what} version {text:?} is not supported (the game reads {major}.{minor}.x)"
        ))),
    }
}

/// What the tempo map needed repairing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TempoReport {
    pub unsorted_time_changes: bool,
    pub negative_time_changes_clamped: usize,
    pub same_time_changes_replaced: usize,
    pub repeated_time_changes: usize,
    pub non_quarter_denominators: usize,
}

pub(super) struct Loaded {
    pub chart: ChartFile,
    pub meta: MetadataFile,
    pub timing: Vec<TimingPoint>,
    pub tempo: TempoReport,
}

pub(super) fn load(chart: &[u8], metadata: &[u8]) -> RoxResult<Loaded> {
    let chart: ChartFile = parse_json(chart, "chart")?;
    check_version(chart.version.as_deref(), "chart", 2, 0)?;
    let meta: MetadataFile = parse_json(metadata, "metadata")?;
    check_version(meta.version.as_deref(), "metadata", 2, 2)?;
    if let Some(format) = meta.time_format.as_deref()
        && format != "ms"
    {
        return Err(bad(format!(
            "timeFormat {format:?} is not supported (only \"ms\")"
        )));
    }
    let (timing, tempo) = tempo_map(meta.time_changes.as_deref().unwrap_or(&[]))?;
    Ok(Loaded {
        chart,
        meta,
        timing,
        tempo,
    })
}

/// `timeChanges` to BPM points. Like the game (`Conductor.mapTimeChanges`) the entries are sorted by time and a time
/// before 0 becomes 0. The first entry must be at 0 and carry a `bpm`; a later `bpm` that is absent keeps the one before,
/// an absent numerator is 4. At one time the last entry wins; an entry that changes neither the BPM nor the numerator is dropped.
fn tempo_map(changes: &[TimeChange]) -> RoxResult<(Vec<TimingPoint>, TempoReport)> {
    let mut report = TempoReport::default();
    if changes.is_empty() {
        return Err(bad("timeChanges is empty (an entry at t = 0 is required)"));
    }
    let mut rows: Vec<(f64, &TimeChange)> = Vec::with_capacity(changes.len());
    for change in changes {
        if !change.t.is_finite() || change.t.abs() > MAX_MS {
            return Err(bad(format!(
                "time change at t = {} is out of range",
                change.t
            )));
        }
        let mut t = change.t;
        if t < 0.0 {
            t = 0.0;
            report.negative_time_changes_clamped += 1;
        }
        rows.push((t, change));
    }
    report.unsorted_time_changes = rows.windows(2).any(|pair| pair[1].0 < pair[0].0);
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    if rows[0].0 != 0.0 {
        return Err(bad(format!(
            "the first time change is at t = {} ms, not 0",
            rows[0].0
        )));
    }
    // One entry per time: the last one wins (the sort is stable).
    let mut kept: Vec<(f64, &TimeChange)> = Vec::with_capacity(rows.len());
    for row in rows {
        if kept
            .last()
            .is_some_and(|last| last.0.to_bits() == row.0.to_bits())
        {
            kept.pop();
            report.same_time_changes_replaced += 1;
        }
        kept.push(row);
    }

    let mut points: Vec<TimingPoint> = Vec::with_capacity(kept.len());
    let mut bpm = f64::NAN;
    let mut previous: Option<(f32, u8)> = None;
    for (position, &(t, change)) in kept.iter().enumerate() {
        match change.bpm {
            Some(value) if value.is_finite() && value > 0.0 && value <= MAX_BPM => bpm = value,
            Some(value) => return Err(bad(format!("time change at t = {t} has bpm {value}"))),
            None if position == 0 => return Err(bad("the first time change has no bpm")),
            None => {}
        }
        let signature = match change.n {
            None => 4,
            Some(n) => u8::try_from(n)
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| bad(format!("time change at t = {t} has numerator {n}")))?,
        };
        if change.d.is_some_and(|d| d != 4) {
            report.non_quarter_denominators += 1;
        }
        #[allow(clippy::cast_possible_truncation)] // bounded by MAX_BPM
        let as_f32 = bpm as f32;
        if previous == Some((as_f32, signature)) {
            report.repeated_time_changes += 1;
        } else {
            let time_us = ms_to_us(t).ok_or_else(|| bad("time change out of range"))?;
            points.push(TimingPoint::Bpm {
                time_us,
                bpm: as_f32,
                signature,
            });
            previous = Some((as_f32, signature));
        }
    }
    Ok((points, report))
}
