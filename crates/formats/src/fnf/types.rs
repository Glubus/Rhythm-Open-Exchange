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
    pub bpm: f32,
    #[serde(default = "default_speed")]
    pub speed: f32,
    #[serde(default = "default_player1")]
    pub player1: String,
    #[serde(default = "default_player2")]
    pub player2: String,
    #[serde(default)]
    pub needs_voices: bool,
    #[serde(default)]
    pub notes: Vec<FnfSection>,
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
    #[serde(default = "default_length")]
    pub length_in_steps: i32,
    #[serde(default)]
    pub must_hit_section: bool,
    #[serde(default)]
    pub change_bpm: bool,
    #[serde(default)]
    pub bpm: f32,
}

fn default_length() -> i32 {
    16
}

/// A note: `[time_ms, lane, duration_ms]`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FnfNote(pub Vec<f64>);

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
