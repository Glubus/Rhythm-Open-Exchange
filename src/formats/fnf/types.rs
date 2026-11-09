#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use serde::{Deserialize, Serialize};

/// Which side to extract from an FNF chart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FnfSide {
    #[default]
    Player,
    Opponent,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FnfChart {
    pub song: FnfSong,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FnfSong {
    pub song: String,
    #[serde(deserialize_with = "lenient_f32")]
    pub bpm: f32,
    #[serde(default = "default_speed", deserialize_with = "lenient_speed")]
    pub speed: f32,
    #[serde(default = "default_player1")]
    pub player1: String,
    #[serde(default = "default_player2")]
    pub player2: String,
    #[serde(default, deserialize_with = "lenient_bool")]
    pub needs_voices: bool,
    #[serde(default)]
    pub notes: Vec<FnfSection>,
    /// Psych Engine's `format` field (`psych_v1` for current charts, absent in legacy ones). A `psych_v1` file gives
    /// the lanes 0-3 to the player and 4-7 to the opponent whatever `mustHitSection` says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

fn default_speed() -> f32 {
    1.0
}
fn default_player1() -> String {
    "bf".to_string()
}
fn default_player2() -> String {
    "dad".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FnfSection {
    #[serde(default)]
    pub section_notes: Vec<FnfNote>,
    #[serde(default = "default_length", deserialize_with = "lenient_length")]
    pub length_in_steps: i32,
    #[serde(default, deserialize_with = "lenient_bool")]
    pub must_hit_section: bool,
    /// Written `changeBPM` by the game and every engine since (a camel-case `changeBpm` is read too).
    #[serde(default, rename = "changeBPM", alias = "changeBpm", deserialize_with = "lenient_bool")]
    pub change_bpm: bool,
    #[serde(default, deserialize_with = "lenient_f32")]
    pub bpm: f32,
}

// Engines and editors write odd values in fields the chart does not need (`"bpm": false`, `"speed": null`, a string): the chart
// is still read, such a value counting as absent.
fn number(value: &serde_json::Value) -> Option<f64> {
    value.as_f64().filter(|value| value.is_finite())
}

#[allow(clippy::cast_possible_truncation)] // a BPM or a speed as f32
fn lenient_f32<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    Ok(number(&serde_json::Value::deserialize(deserializer)?).map_or(0.0, |value| value as f32))
}

#[allow(clippy::cast_possible_truncation)]
fn lenient_speed<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    Ok(number(&serde_json::Value::deserialize(deserializer)?).map_or_else(default_speed, |value| value as f32))
}

#[allow(clippy::cast_possible_truncation)]
fn lenient_length<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<i32, D::Error> {
    Ok(number(&serde_json::Value::deserialize(deserializer)?).map_or_else(default_length, |value| value as i32))
}

fn lenient_bool<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Ok(serde_json::Value::deserialize(deserializer)?.as_bool().unwrap_or(false))
}

fn default_length() -> i32 {
    16
}

/// A note: `[time_ms, lane, duration_ms, …]`. Engines add a note type after the duration (`"Hurt Note"`, `"No Animation"`) and put
/// events in the same list (`[time, -1, "Hey!", …]`): an element that is not a number reads as NaN, and such an entry is no note
/// ([`Self::is_note`]).
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct FnfNote(pub Vec<f64>);

impl<'de> Deserialize<'de> for FnfNote {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let values = Vec::<serde_json::Value>::deserialize(deserializer)?;
        Ok(Self(values.iter().take(3).map(|value| number(value).unwrap_or(f64::NAN)).collect()))
    }
}

impl FnfNote {
    #[must_use]
    pub fn time_ms(&self) -> f64 {
        self.0.first().copied().unwrap_or(0.0)
    }
    #[must_use]
    pub fn lane(&self) -> u8 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // value is non-negative in valid input; lane fits in u8
        {
            self.0.get(1).copied().unwrap_or(0.0) as u8
        }
    }
    #[must_use]
    pub fn duration_ms(&self) -> f64 {
        self.0.get(2).copied().unwrap_or(0.0)
    }
    /// A playable note: a finite time and a lane from 0 (an event's lane is -1, a note type in place of a number is no lane).
    #[must_use]
    pub fn is_note(&self) -> bool {
        self.time_ms().is_finite() && self.0.get(1).is_some_and(|lane| lane.is_finite() && (0.0..=255.0).contains(lane))
    }
    #[must_use]
    pub fn is_hold(&self) -> bool {
        self.duration_ms() > 0.0
    }

    #[must_use]
    pub fn tap(time_ms: f64, lane: u8) -> Self {
        Self(vec![time_ms, f64::from(lane), 0.0])
    }
    #[must_use]
    pub fn hold(time_ms: f64, lane: u8, duration_ms: f64) -> Self {
        Self(vec![time_ms, f64::from(lane), duration_ms])
    }
}
