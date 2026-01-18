//! Note types for VSRG.

use rkyv::{Archive, Deserialize, Serialize};

/// Type of note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum NoteType {
    /// Single tap note.
    Tap,
    /// Long note (hold) - must be held for the duration.
    Hold { duration_us: i64 },
    /// Burst/roll note - rapid tapping during the duration.
    Burst { duration_us: i64 },
    /// Mine - avoid hitting this note.
    Mine,
}

/// A single note in the chart.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct Note {
    /// Position in microseconds.
    pub time_us: i64,
    /// Column index (0-indexed).
    pub column: u8,
    /// Type of note (tap, hold, burst, mine).
    pub note_type: NoteType,
    /// Optional index into `RoxChart.hitsounds` for keysounded notes.
    pub hitsound_index: Option<u16>,
}

impl Note {
    /// Create a tap note.
    #[must_use]
    pub fn tap(time_us: i64, column: u8) -> Self {
        Self {
            time_us,
            column,
            note_type: NoteType::Tap,
            hitsound_index: None,
        }
    }

    /// Create a hold note.
    #[must_use]
    pub fn hold(time_us: i64, duration_us: i64, column: u8) -> Self {
        Self {
            time_us,
            column,
            note_type: NoteType::Hold { duration_us },
            hitsound_index: None,
        }
    }

    /// Create a burst/roll note.
    #[must_use]
    pub fn burst(time_us: i64, duration_us: i64, column: u8) -> Self {
        Self {
            time_us,
            column,
            note_type: NoteType::Burst { duration_us },
            hitsound_index: None,
        }
    }

    /// Create a mine note.
    #[must_use]
    pub fn mine(time_us: i64, column: u8) -> Self {
        Self {
            time_us,
            column,
            note_type: NoteType::Mine,
            hitsound_index: None,
        }
    }

    /// Check if this is a hold note.
    #[must_use]
    pub fn is_hold(&self) -> bool {
        matches!(self.note_type, NoteType::Hold { .. })
    }

    /// Check if this is a burst note.
    #[must_use]
    pub fn is_burst(&self) -> bool {
        matches!(self.note_type, NoteType::Burst { .. })
    }

    /// Check if this is a mine.
    #[must_use]
    pub fn is_mine(&self) -> bool {
        matches!(self.note_type, NoteType::Mine)
    }

    /// Get the duration for holds/bursts, or 0 for taps/mines.
    #[must_use]
    pub fn duration_us(&self) -> i64 {
        match self.note_type {
            NoteType::Tap | NoteType::Mine => 0,
            NoteType::Hold { duration_us } | NoteType::Burst { duration_us } => duration_us,
        }
    }

    /// Get end time (start time + duration).
    #[must_use]
    pub fn end_time_us(&self) -> i64 {
        self.time_us + self.duration_us()
    }
}
