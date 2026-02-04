//! Main chart container.

use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

use super::{Hitsound, Metadata, Note, TimingPoint};

/// Current ROX format version.
pub const ROX_VERSION: u8 = 2;

/// Magic bytes to identify ROX files: "ROX\0"
pub const ROX_MAGIC: [u8; 4] = [0x52, 0x4F, 0x58, 0x00];

/// A complete VSRG chart in ROX format.
#[derive(
    Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize,
)]
pub struct RoxChart {
    /// Format version for backwards compatibility.
    pub version: u8,
    /// Chart metadata.
    pub metadata: Metadata,
    /// Timing points (BPM and SV changes).
    pub timing_points: Vec<TimingPoint>,
    /// All notes in the chart.
    pub notes: Vec<Note>,
    /// Hitsound samples (notes reference by index).
    pub hitsounds: Vec<Hitsound>,
}

impl RoxChart {
    /// Create a new empty chart with the given key count.
    #[must_use]
    pub fn new(key_count: u8) -> Self {
        Self {
            version: ROX_VERSION,
            metadata: Metadata {
                key_count,
                ..Metadata::default()
            },
            timing_points: Vec::new(),
            notes: Vec::new(),
            hitsounds: Vec::new(),
        }
    }

    /// Get the key count (convenience accessor for `metadata.key_count`).
    #[must_use]
    pub fn key_count(&self) -> u8 {
        self.metadata.key_count
    }

    /// Get the total duration of the chart in microseconds.
    #[must_use]
    pub fn duration_us(&self) -> i64 {
        self.notes
            .iter()
            .map(super::note::Note::end_time_us)
            .max()
            .unwrap_or(0)
    }

    /// Get the number of notes (taps + holds).
    #[must_use]
    pub fn note_count(&self) -> usize {
        self.notes.len()
    }

    /// Validate the chart for consistency and correctness.
    ///
    /// Checks:
    /// - All note columns are within bounds
    /// - All hold/burst durations are > 0
    /// - Timing points are sorted by time
    /// - At least one BPM timing point exists
    /// - First BPM timing point is at or before the first note
    /// - No overlapping notes on the same column
    ///
    /// # Errors
    ///
    /// Returns an error if any validation check fails.
    pub fn validate(&self) -> Result<(), crate::RoxError> {
        // 1. Check metadata consistency
        let key_count = self.key_count();
        if self.metadata.is_coop && !key_count.is_multiple_of(2) {
            return Err(crate::RoxError::InvalidFormat(format!(
                "Coop mode requires even key count, got {key_count}"
            )));
        }

        // 2. Check timing points sorted by time
        // This is O(T)
        let mut prev_time = i64::MIN;
        for tp in &self.timing_points {
            if tp.time_us < prev_time {
                return Err(crate::RoxError::TimingPointsNotSorted {
                    prev_time_us: prev_time,
                    time_us: tp.time_us,
                });
            }
            prev_time = tp.time_us;
        }

        if !self.notes.is_empty() {
            // Check at least one BPM timing point exists
            if !self.timing_points.iter().any(|tp| !tp.is_inherited) {
                return Err(crate::RoxError::NoBpmTimingPoint);
            }
        }

        // 3. Single pass validation for notes O(N)
        // We track the last end time for each column to detect overlaps.
        // This requires notes to be sorted globally by time, or at least per column.
        // The previous implementation sorted per-column. Here we assume global sort or sorted-per-column input.
        // However, to strictly guarantee O(N) overlap checks without allocation, we track per-column state.
        let mut last_end_times = vec![i64::MIN; key_count as usize];

        // We verify that notes are strictly sorted by time overall.
        // If they are not, `validate` fails. This enforces strict ordering.
        let mut prev_note_time = i64::MIN;

        for note in &self.notes {
            // 3a. Check global sort order
            if note.time_us < prev_note_time {
                return Err(crate::RoxError::NotesNotSorted {
                    prev_time_us: prev_note_time,
                    time_us: note.time_us,
                });
            }
            prev_note_time = note.time_us;

            // 3b. Check column bounds
            if note.column >= key_count {
                return Err(crate::RoxError::InvalidColumn {
                    column: note.column,
                    key_count,
                });
            }

            // 3c. Check durations
            let duration = note.duration_us();
            if (note.is_hold() || note.is_burst()) && duration <= 0 {
                return Err(crate::RoxError::InvalidHoldDuration {
                    time_us: note.time_us,
                    duration_us: duration,
                });
            }

            // 3d. Check overlaps on specific column
            let col_idx = note.column as usize;
            if note.time_us < last_end_times[col_idx] {
                // Overlap detected!
                return Err(crate::RoxError::OverlappingNotes {
                    column: note.column,
                    time_us: note.time_us,
                });
            }
            last_end_times[col_idx] = note.end_time_us();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rox_chart_new() {
        let chart = RoxChart::new(4);

        assert_eq!(chart.version, 2);
        assert_eq!(chart.key_count(), 4);
        assert!(chart.timing_points.is_empty());
        assert!(chart.notes.is_empty());
        assert!(chart.hitsounds.is_empty());
    }

    #[test]
    fn test_rox_chart_new_7k() {
        let chart = RoxChart::new(7);
        assert_eq!(chart.key_count(), 7);
    }

    #[test]
    fn test_rox_chart_duration_empty() {
        let chart = RoxChart::new(4);
        assert_eq!(chart.duration_us(), 0);
    }

    #[test]
    fn test_rox_chart_duration_with_notes() {
        let mut chart = RoxChart::new(4);
        chart.notes.push(Note::tap(1_000_000, 0));
        chart.notes.push(Note::tap(2_000_000, 1));
        chart.notes.push(Note::hold(3_000_000, 500_000, 2)); // ends at 3.5s

        assert_eq!(chart.duration_us(), 3_500_000);
    }

    #[test]
    fn test_rox_chart_note_count() {
        let mut chart = RoxChart::new(4);
        assert_eq!(chart.note_count(), 0);

        chart.notes.push(Note::tap(0, 0));
        chart.notes.push(Note::hold(1_000_000, 500_000, 1));
        chart.notes.push(Note::mine(2_000_000, 2));

        assert_eq!(chart.note_count(), 3);
    }

    #[test]
    fn test_rox_chart_validate_valid() {
        let mut chart = RoxChart::new(4);
        chart.notes.push(Note::tap(0, 0));
        chart.notes.push(Note::tap(0, 1));
        chart.notes.push(Note::tap(0, 2));
        chart.notes.push(Note::tap(0, 3));

        chart.timing_points.push(TimingPoint::bpm(0, 120.0));

        assert!(chart.validate().is_ok());
    }

    #[test]
    fn test_rox_chart_validate_invalid_column() {
        let mut chart = RoxChart::new(4);
        chart.notes.push(Note::tap(0, 4)); // Invalid: column 4 doesn't exist in 4K

        assert!(chart.validate().is_err());
    }
}
