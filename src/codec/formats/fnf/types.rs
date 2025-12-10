//! Type definitions for Friday Night Funkin' .json chart format.

use serde::{Deserialize, Serialize};

/// Which side to extract from an FNF chart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FnfSide {
    /// Player notes only (4K) - lanes remapped to 0-3
    #[default]
    Player,
    /// Opponent notes only (4K) - lanes remapped to 0-3
    Opponent,
    /// Both sides (8K) - opponent 0-3, player 4-7
    Both,
}

/// Root FNF chart structure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FnfChart {
    pub song: FnfSong,
}

/// Song data container.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FnfSong {
    /// Song name.
    pub song: String,
    /// Base BPM.
    pub bpm: f32,
    /// Scroll speed multiplier.
    #[serde(default = "default_speed")]
    pub speed: f32,
    /// Player character ID.
    #[serde(default = "default_player1")]
    pub player1: String,
    /// Opponent character ID.
    #[serde(default = "default_player2")]
    pub player2: String,
    /// Whether the song has a vocal track.
    #[serde(default)]
    pub needs_voices: bool,
    /// Whether this is a valid score submission.
    #[serde(default = "default_true")]
    pub valid_score: bool,
    /// Sections containing notes.
    #[serde(default)]
    pub notes: Vec<FnfSection>,
    /// Number of sections (often unused).
    #[serde(default)]
    pub sections: i32,
    /// Section lengths (often unused).
    #[serde(default)]
    pub section_lengths: Vec<i32>,
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
fn default_true() -> bool {
    true
}

/// A section of the song containing notes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FnfSection {
    /// Notes in this section: `[time_ms, lane, duration_ms]`.
    #[serde(default)]
    pub section_notes: Vec<FnfNote>,
    /// Length of section in steps (16th notes).
    #[serde(default = "default_length")]
    pub length_in_steps: i32,
    /// If true, lanes 0-3 are player, 4-7 are opponent.
    /// If false, lanes 0-3 are opponent, 4-7 are player.
    #[serde(default)]
    pub must_hit_section: bool,
    /// Whether BPM changes in this section.
    #[serde(default)]
    pub change_bpm: bool,
    /// New BPM if `change_bpm` is true.
    #[serde(default)]
    pub bpm: f32,
    /// Section type (often 0).
    #[serde(default)]
    pub type_of_section: i32,
}

fn default_length() -> i32 {
    16
}

/// A single note: `[time_ms, lane, duration_ms]`.
/// Using a tuple struct for the array format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FnfNote(pub Vec<f64>);

impl FnfNote {
    /// Get time in milliseconds.
    #[must_use]
    pub fn time_ms(&self) -> f64 {
        self.0.first().copied().unwrap_or(0.0)
    }

    /// Get lane (0-7).
    #[must_use]
    pub fn lane(&self) -> u8 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let lane = self.0.get(1).copied().unwrap_or(0.0) as u8;
        lane
    }

    /// Get duration in milliseconds (0 = tap, >0 = hold).
    #[must_use]
    pub fn duration_ms(&self) -> f64 {
        self.0.get(2).copied().unwrap_or(0.0)
    }

    /// Check if this is a hold note.
    #[must_use]
    pub fn is_hold(&self) -> bool {
        self.duration_ms() > 0.0
    }

    /// Create a new tap note.
    #[must_use]
    pub fn tap(time_ms: f64, lane: u8) -> Self {
        Self(vec![time_ms, f64::from(lane), 0.0])
    }

    /// Create a new hold note.
    #[must_use]
    pub fn hold(time_ms: f64, lane: u8, duration_ms: f64) -> Self {
        Self(vec![time_ms, f64::from(lane), duration_ms])
    }
}
