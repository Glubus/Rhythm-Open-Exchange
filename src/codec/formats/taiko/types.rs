//! Type definitions for osu!taiko format conversion.

use crate::codec::formats::osu::types::{OsuDifficulty, OsuGeneral, OsuMetadata, OsuTimingPoint};

use bitflags::bitflags;

bitflags! {
    /// Hitsound flags that determine Taiko note type.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TaikoHitsound: u32 {
        /// Normal sound - Don (center)
        const NORMAL = 1 << 0;
        /// Whistle sound - Kat (rim)
        const WHISTLE = 1 << 1;
        /// Finish modifier - Big note (2 columns)
        const FINISH = 1 << 2;
        /// Clap sound - Kat (rim)
        const CLAP = 1 << 3;
    }
}

impl TaikoHitsound {
    /// Check if this is a Kat (rim hit).
    #[must_use]
    pub fn is_kat(self) -> bool {
        self.contains(Self::WHISTLE) || self.contains(Self::CLAP)
    }

    /// Check if this is a Don (center hit).
    #[must_use]
    pub fn is_don(self) -> bool {
        self.contains(Self::NORMAL) || self.is_empty()
    }

    /// Check if this is a big note (Finish).
    #[must_use]
    pub fn is_big(self) -> bool {
        self.contains(Self::FINISH)
    }
}

/// A parsed Taiko hit object.
#[derive(Debug, Clone)]
pub struct TaikoHitObject {
    /// Time in milliseconds.
    pub time_ms: f64,
    /// Hitsound flags.
    pub hitsound: TaikoHitsound,
    /// Object type flags (for detecting spinners/sliders).
    pub object_type: u32,
}

impl TaikoHitObject {
    /// Check if this is a spinner (ignore for conversion).
    #[must_use]
    pub fn is_spinner(&self) -> bool {
        (self.object_type & 8) != 0
    }

    /// Check if this is a slider/drumroll (convert as single hit).
    #[must_use]
    pub fn is_slider(&self) -> bool {
        (self.object_type & 2) != 0
    }
}

/// Parsed Taiko beatmap data.
#[derive(Debug, Default)]
pub struct TaikoBeatmap {
    /// Format version.
    pub format_version: u8,
    /// General section.
    pub general: OsuGeneral,
    /// Metadata section.
    pub metadata: OsuMetadata,
    /// Difficulty section.
    pub difficulty: OsuDifficulty,
    /// Background file.
    pub background: Option<String>,
    /// Timing points.
    pub timing_points: Vec<OsuTimingPoint>,
    /// Taiko hit objects.
    pub hit_objects: Vec<TaikoHitObject>,
}

/// Column layout options for Taiko->`4K` conversion.
/// The layout defines which columns are Don (D) and which are Kat (K).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnLayout {
    /// D K K D - Dons on outer columns (0, 3), Kats on inner (1, 2)
    #[default]
    Dkkd,
    /// D K D K - Alternating, starting with Don
    Dkdk,
    /// K D D K - Kats on outer columns (0, 3), Dons on inner (1, 2)
    Kddk,
}

impl ColumnLayout {
    /// Get the columns for Don notes (sorted from smallest to largest).
    #[must_use]
    pub const fn don_columns(self) -> [u8; 2] {
        match self {
            Self::Dkkd => [0, 3], // outer
            Self::Dkdk => [0, 2], // even columns
            Self::Kddk => [1, 2], // inner
        }
    }

    /// Get the columns for Kat notes (sorted from smallest to largest).
    #[must_use]
    pub const fn kat_columns(self) -> [u8; 2] {
        match self {
            Self::Dkkd => [1, 2], // inner
            Self::Dkdk => [1, 3], // odd columns
            Self::Kddk => [0, 3], // outer
        }
    }
}

/// Alternation state for column assignment.
#[derive(Debug)]
pub struct AlternationState {
    /// Current layout.
    pub layout: ColumnLayout,
    /// Next Don column index (0 or 1, indexes into `don_columns`).
    pub don_index: usize,
    /// Next Kat column index (0 or 1, indexes into `kat_columns`).
    pub kat_index: usize,
}

impl Default for AlternationState {
    fn default() -> Self {
        Self::new(ColumnLayout::default())
    }
}

impl AlternationState {
    /// Create a new alternation state with the given layout.
    #[must_use]
    pub const fn new(layout: ColumnLayout) -> Self {
        Self {
            layout,
            don_index: 0,
            kat_index: 0,
        }
    }

    /// Get the next Don column(s) based on whether it's a big note.
    /// Always starts from the smallest column in the layout.
    pub fn next_don_columns(&mut self, is_big: bool) -> Vec<u8> {
        let columns = self.layout.don_columns();

        if is_big {
            // Big note: both columns at once
            vec![columns[0], columns[1]]
        } else {
            // Single note: alternate, starting from smallest (index 0)
            let col = columns[self.don_index];
            self.don_index = (self.don_index + 1) % 2;
            vec![col]
        }
    }

    /// Get the next Kat column(s) based on whether it's a big note.
    /// Always starts from the smallest column in the layout.
    pub fn next_kat_columns(&mut self, is_big: bool) -> Vec<u8> {
        let columns = self.layout.kat_columns();

        if is_big {
            // Big note: both columns at once
            vec![columns[0], columns[1]]
        } else {
            // Single note: alternate, starting from smallest (index 0)
            let col = columns[self.kat_index];
            self.kat_index = (self.kat_index + 1) % 2;
            vec![col]
        }
    }
}
