use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

/// Type of a note in the chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Archive, Serialize, Deserialize)]
#[derive(SerdeSerialize, SerdeDeserialize)]
#[serde(tag = "type", content = "data")]
pub enum NoteType {
    Tap,
    Hold { duration_us: i64 },
    Burst { duration_us: i64 },
    Mine,
}

/// A single note in a chart.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Archive, Serialize, Deserialize)]
#[derive(SerdeSerialize, SerdeDeserialize)]
pub struct Note {
    /// Start time in microseconds.
    pub time_us: i64,
    /// Note type (tap, hold, burst, mine).
    pub note_type: NoteType,
    /// Optional index into `RoxChart.hitsounds`.
    pub hitsound_index: Option<u16>,
    /// Column index (0-indexed).
    pub column: u8,
}

impl Note {
    #[must_use]
    pub fn tap(time_us: i64, column: u8) -> Self {
        Self { time_us, column, note_type: NoteType::Tap, hitsound_index: None }
    }

    #[must_use]
    pub fn hold(time_us: i64, duration_us: i64, column: u8) -> Self {
        Self { time_us, column, note_type: NoteType::Hold { duration_us }, hitsound_index: None }
    }

    #[must_use]
    pub fn burst(time_us: i64, duration_us: i64, column: u8) -> Self {
        Self { time_us, column, note_type: NoteType::Burst { duration_us }, hitsound_index: None }
    }

    #[must_use]
    pub fn mine(time_us: i64, column: u8) -> Self {
        Self { time_us, column, note_type: NoteType::Mine, hitsound_index: None }
    }

    #[must_use]
    pub fn is_hold(&self) -> bool {
        matches!(self.note_type, NoteType::Hold { .. })
    }

    #[must_use]
    pub fn is_burst(&self) -> bool {
        matches!(self.note_type, NoteType::Burst { .. })
    }

    #[must_use]
    pub fn is_mine(&self) -> bool {
        matches!(self.note_type, NoteType::Mine)
    }

    #[must_use]
    pub fn duration_us(&self) -> i64 {
        match self.note_type {
            NoteType::Tap | NoteType::Mine => 0,
            NoteType::Hold { duration_us } | NoteType::Burst { duration_us } => duration_us,
        }
    }

    #[must_use]
    pub fn end_time_us(&self) -> i64 {
        self.time_us + self.duration_us()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(Note::tap(1_000_000, 2), 0, false, false, false)]
    #[case(Note::hold(0, 500_000, 1), 500_000, true, false, false)]
    #[case(Note::burst(0, 300_000, 3), 300_000, false, true, false)]
    #[case(Note::mine(0, 0), 0, false, false, true)]
    fn test_note_predicates_and_duration(
        #[case] note: Note,
        #[case] expected_duration: i64,
        #[case] is_hold: bool,
        #[case] is_burst: bool,
        #[case] is_mine: bool,
    ) {
        assert_eq!(note.duration_us(), expected_duration);
        assert_eq!(note.is_hold(), is_hold);
        assert_eq!(note.is_burst(), is_burst);
        assert_eq!(note.is_mine(), is_mine);
    }

    #[rstest]
    #[case(Note::tap(1_000_000, 0), 1_000_000)]
    #[case(Note::hold(1_000_000, 500_000, 0), 1_500_000)]
    #[case(Note::burst(2_000_000, 300_000, 0), 2_300_000)]
    #[case(Note::mine(3_000_000, 0), 3_000_000)]
    fn test_end_time_us(#[case] note: Note, #[case] expected: i64) {
        assert_eq!(note.end_time_us(), expected);
    }

    #[test]
    fn test_hitsound_index_defaults_to_none() {
        assert!(Note::tap(0, 0).hitsound_index.is_none());
        assert!(Note::hold(0, 1, 0).hitsound_index.is_none());
    }
}
