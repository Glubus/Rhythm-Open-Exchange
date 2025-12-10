//! Type definitions for Quaver .qua format.

use serde::{Deserialize, Serialize};

/// Game mode (Keys4 or Keys7).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum QuaMode {
    #[default]
    Keys4,
    Keys7,
}

impl QuaMode {
    /// Get the key count for this mode.
    #[must_use]
    pub const fn key_count(&self) -> u8 {
        match self {
            Self::Keys4 => 4,
            Self::Keys7 => 7,
        }
    }
}

/// Time signature for timing points.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum TimeSignature {
    #[default]
    Quadruple,
    Triple,
}

impl TimeSignature {
    /// Get the beats per measure.
    #[must_use]
    pub const fn beats(&self) -> u8 {
        match self {
            Self::Quadruple => 4,
            Self::Triple => 3,
        }
    }
}

/// Parsed Quaver beatmap.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct QuaChart {
    #[serde(rename = "AudioFile")]
    pub audio_file: String,
    #[serde(rename = "SongPreviewTime")]
    pub preview_time: i32,
    #[serde(rename = "BackgroundFile")]
    pub background_file: Option<String>,
    #[serde(rename = "BannerFile")]
    pub banner_file: Option<String>,
    #[serde(rename = "MapId")]
    pub map_id: i32,
    #[serde(rename = "MapSetId")]
    pub map_set_id: i32,
    #[serde(rename = "Mode")]
    pub mode: QuaMode,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Artist")]
    pub artist: String,
    #[serde(rename = "Source")]
    pub source: Option<String>,
    #[serde(rename = "Tags")]
    pub tags: Option<String>,
    #[serde(rename = "Creator")]
    pub creator: String,
    #[serde(rename = "DifficultyName")]
    pub difficulty_name: String,
    #[serde(rename = "Description")]
    pub description: Option<String>,
    #[serde(rename = "BPMDoesNotAffectScrollVelocity")]
    pub bpm_does_not_affect_sv: bool,
    #[serde(rename = "InitialScrollVelocity", default = "default_sv")]
    pub initial_scroll_velocity: f32,
    #[serde(rename = "EditorLayers")]
    pub editor_layers: Vec<serde_yaml::Value>,
    #[serde(rename = "CustomAudioSamples")]
    pub custom_audio_samples: Vec<serde_yaml::Value>,
    #[serde(rename = "SoundEffects")]
    pub sound_effects: Vec<serde_yaml::Value>,
    #[serde(rename = "TimingPoints")]
    pub timing_points: Vec<QuaTimingPoint>,
    #[serde(rename = "SliderVelocities")]
    pub slider_velocities: Vec<QuaSliderVelocity>,
    #[serde(rename = "HitObjects")]
    pub hit_objects: Vec<QuaHitObject>,
}

fn default_sv() -> f32 {
    1.0
}

/// Timing point (BPM change).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct QuaTimingPoint {
    #[serde(rename = "StartTime")]
    pub start_time: f64,
    #[serde(rename = "Bpm")]
    pub bpm: f32,
    #[serde(rename = "Signature", skip_serializing_if = "Option::is_none")]
    pub signature: Option<TimeSignature>,
}

/// Scroll velocity change.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct QuaSliderVelocity {
    #[serde(rename = "StartTime")]
    pub start_time: f64,
    #[serde(rename = "Multiplier")]
    pub multiplier: f64,
}

/// Hit object (note or hold).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct QuaHitObject {
    #[serde(rename = "StartTime")]
    pub start_time: f64,
    #[serde(rename = "Lane")]
    pub lane: u8,
    #[serde(rename = "EndTime", skip_serializing_if = "Option::is_none")]
    pub end_time: Option<f64>,
    #[serde(rename = "KeySounds")]
    pub key_sounds: Vec<serde_yaml::Value>,
}
