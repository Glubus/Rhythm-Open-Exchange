//! Type definitions for Malody .mc format.
//!
//! Malody Key mode charts are JSON files with:
//! - `meta`: song info, key count, creator
//! - `time`: BPM timing points with beat positions
//! - `note`: playable notes (tap/hold) and sound-meta entries
//! - `effect`: optional scroll velocity changes

use serde::Deserialize;

/// Beat position as [measure, numerator, divisor].
pub(crate) type Beat = [i32; 3];

/// Convert a beat triple to a floating-point beat count.
pub(crate) fn beat_to_f64(b: &Beat) -> f64 {
    b[0] as f64 + b[1] as f64 / b[2] as f64
}

/// Convert beats at a given BPM to milliseconds.
pub(crate) fn beats_to_ms(beats: f64, bpm: f64) -> f64 {
    1000.0 * (60.0 / bpm) * beats
}

/// Parsed Malody .mc chart (Key mode only).
#[derive(Debug, Deserialize)]
pub struct McChart {
    pub meta: McMeta,
    pub time: Vec<McTimingPoint>,
    pub note: Vec<McNote>,
    #[serde(default)]
    pub effect: Vec<McEffect>,
}

#[derive(Debug, Deserialize)]
pub struct McMeta {
    pub mode: u8,
    pub song: McSong,
    pub mode_ext: McModeExt,
    #[serde(default)]
    pub background: String,
    #[serde(default)]
    pub creator: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub preview: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct McSong {
    pub title: String,
    #[serde(default)]
    pub titleorg: Option<String>,
    pub artist: String,
    #[serde(default)]
    pub artistorg: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct McModeExt {
    pub column: u8,
}

/// BPM timing point.
#[derive(Debug, Deserialize)]
pub struct McTimingPoint {
    pub beat: Beat,
    pub bpm: f64,
    #[serde(default = "default_sign")]
    pub sign: u8,
}

fn default_sign() -> u8 {
    4
}

/// A note entry. Two kinds:
/// - `note_type == 0` (default): playable note with beat/column/optional endbeat
/// - `note_type != 0`: sound metadata (audio file, offset, volume)
#[derive(Debug, Deserialize)]
pub struct McNote {
    #[serde(default)]
    pub beat: Option<Beat>,
    pub column: Option<u8>,
    pub endbeat: Option<Beat>,
    #[serde(default, rename = "type")]
    pub note_type: u8,
    pub sound: Option<String>,
    #[serde(default)]
    pub offset: Option<i64>,
    pub vol: Option<f32>,
}

/// Scroll velocity change.
#[derive(Debug, Deserialize)]
pub struct McEffect {
    pub beat: Beat,
    pub scroll: f64,
}
