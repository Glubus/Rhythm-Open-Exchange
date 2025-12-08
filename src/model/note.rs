//! Note types for VSRG.

use bincode::{Decode, Encode};

/// Type of note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
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
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct Note {
    /// Position in microseconds.
    pub time_us: i64,
    /// Column index (0-indexed).
    pub column: u8,
    /// Type of note (tap, hold, burst, mine).
    pub note_type: NoteType,
    /// Optional index into RoxChart.hitsounds for keysounded notes.
    pub hitsound_index: Option<u16>,
}

impl Note {
    /// Create a tap note.
    pub fn tap(time_us: i64, column: u8) -> Self {
        Self {
            time_us,
            column,
            note_type: NoteType::Tap,
            hitsound_index: None,
        }
    }

    /// Create a hold note.
    pub fn hold(time_us: i64, duration_us: i64, column: u8) -> Self {
        Self {
            time_us,
            column,
            note_type: NoteType::Hold { duration_us },
            hitsound_index: None,
        }
    }

    /// Create a burst/roll note.
    pub fn burst(time_us: i64, duration_us: i64, column: u8) -> Self {
        Self {
            time_us,
            column,
            note_type: NoteType::Burst { duration_us },
            hitsound_index: None,
        }
    }

    /// Create a mine note.
    pub fn mine(time_us: i64, column: u8) -> Self {
        Self {
            time_us,
            column,
            note_type: NoteType::Mine,
            hitsound_index: None,
        }
    }

    /// Check if this is a hold note.
    pub fn is_hold(&self) -> bool {
        matches!(self.note_type, NoteType::Hold { .. })
    }

    /// Check if this is a burst note.
    pub fn is_burst(&self) -> bool {
        matches!(self.note_type, NoteType::Burst { .. })
    }

    /// Check if this is a mine.
    pub fn is_mine(&self) -> bool {
        matches!(self.note_type, NoteType::Mine)
    }

    /// Get the duration for holds/bursts, or 0 for taps/mines.
    pub fn duration_us(&self) -> i64 {
        match self.note_type {
            NoteType::Tap | NoteType::Mine => 0,
            NoteType::Hold { duration_us } | NoteType::Burst { duration_us } => duration_us,
        }
    }

    /// Get end time (start time + duration).
    pub fn end_time_us(&self) -> i64 {
        self.time_us + self.duration_us()
    }
}
