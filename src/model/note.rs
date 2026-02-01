//! Note types for VSRG.

use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

/// Type of note.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Archive,
    Serialize,
    Deserialize,
    SerdeSerialize,
    SerdeDeserialize,
)]
#[serde(tag = "type", content = "data")]
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
#[derive(
    Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize,
)]
pub struct Note {
    /// Position in microseconds.
    pub time_us: i64,
    /// Type of note (tap, hold, burst, mine).
    pub note_type: NoteType,
    /// Optional index into `RoxChart.hitsounds` for keysounded notes.
    pub hitsound_index: Option<u16>,
    /// Column index (0-indexed).
    pub column: u8,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_tap_constructor() {
        let note = Note::tap(1_000_000, 2);

        assert_eq!(note.time_us, 1_000_000);
        assert_eq!(note.column, 2);
        assert!(matches!(note.note_type, NoteType::Tap));
        assert!(note.hitsound_index.is_none());
    }

    #[test]
    fn test_note_hold_constructor() {
        let note = Note::hold(2_000_000, 500_000, 1);

        assert_eq!(note.time_us, 2_000_000);
        assert_eq!(note.column, 1);
        assert!(matches!(
            note.note_type,
            NoteType::Hold {
                duration_us: 500_000
            }
        ));
    }

    #[test]
    fn test_note_burst_constructor() {
        let note = Note::burst(3_000_000, 300_000, 3);

        assert_eq!(note.time_us, 3_000_000);
        assert_eq!(note.column, 3);
        assert!(matches!(
            note.note_type,
            NoteType::Burst {
                duration_us: 300_000
            }
        ));
    }

    #[test]
    fn test_note_mine_constructor() {
        let note = Note::mine(4_000_000, 0);

        assert_eq!(note.time_us, 4_000_000);
        assert_eq!(note.column, 0);
        assert!(matches!(note.note_type, NoteType::Mine));
    }

    #[test]
    fn test_note_is_hold() {
        assert!(!Note::tap(0, 0).is_hold());
        assert!(Note::hold(0, 100, 0).is_hold());
        assert!(!Note::burst(0, 100, 0).is_hold());
        assert!(!Note::mine(0, 0).is_hold());
    }

    #[test]
    fn test_note_is_burst() {
        assert!(!Note::tap(0, 0).is_burst());
        assert!(!Note::hold(0, 100, 0).is_burst());
        assert!(Note::burst(0, 100, 0).is_burst());
        assert!(!Note::mine(0, 0).is_burst());
    }

    #[test]
    fn test_note_is_mine() {
        assert!(!Note::tap(0, 0).is_mine());
        assert!(!Note::hold(0, 100, 0).is_mine());
        assert!(!Note::burst(0, 100, 0).is_mine());
        assert!(Note::mine(0, 0).is_mine());
    }

    #[test]
    fn test_note_duration_us() {
        assert_eq!(Note::tap(0, 0).duration_us(), 0);
        assert_eq!(Note::hold(0, 500_000, 0).duration_us(), 500_000);
        assert_eq!(Note::burst(0, 300_000, 0).duration_us(), 300_000);
        assert_eq!(Note::mine(0, 0).duration_us(), 0);
    }

    #[test]
    fn test_note_end_time_us() {
        assert_eq!(Note::tap(1_000_000, 0).end_time_us(), 1_000_000);
        assert_eq!(Note::hold(1_000_000, 500_000, 0).end_time_us(), 1_500_000);
        assert_eq!(Note::burst(2_000_000, 300_000, 0).end_time_us(), 2_300_000);
        assert_eq!(Note::mine(3_000_000, 0).end_time_us(), 3_000_000);
    }
}
