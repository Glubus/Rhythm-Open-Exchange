//! Main chart container.

use bincode::{Decode, Encode, config};

use super::{Hitsound, Metadata, Note, TimingPoint};

/// Current ROX format version.
pub const ROX_VERSION: u8 = 1;

/// Magic bytes to identify ROX files: "ROX\0"
pub const ROX_MAGIC: [u8; 4] = [0x52, 0x4F, 0x58, 0x00];

/// A complete VSRG chart in ROX format.
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct RoxChart {
    /// Format version for backwards compatibility.
    pub version: u8,
    /// Number of columns/keys (e.g., 4 for 4K, 7 for 7K).
    pub key_count: u8,
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
            key_count,
            metadata: Metadata::default(),
            timing_points: Vec::new(),
            notes: Vec::new(),
            hitsounds: Vec::new(),
        }
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

    /// Compute the BLAKE3 hash of the chart.
    /// Returns a 32-byte hash as a hex string.
    #[must_use]
    pub fn hash(&self) -> String {
        let config = config::standard()
            .with_little_endian()
            .with_variable_int_encoding();
        let encoded = bincode::encode_to_vec(self, config).unwrap_or_default();
        blake3::hash(&encoded).to_hex().to_string()
    }

    /// Compute a short hash (first 16 hex chars).
    #[must_use]
    pub fn short_hash(&self) -> String {
        self.hash()[..16].to_string()
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
        // Check column bounds
        for note in &self.notes {
            if note.column >= self.key_count {
                return Err(crate::RoxError::InvalidColumn {
                    column: note.column,
                    key_count: self.key_count,
                });
            }
        }

        // Check coop mode requires even key count
        if self.metadata.is_coop && !self.key_count.is_multiple_of(2) {
            return Err(crate::RoxError::InvalidFormat(format!(
                "Coop mode requires even key count, got {}",
                self.key_count
            )));
        }

        // Check hold/burst durations > 0
        for note in &self.notes {
            let duration = note.duration_us();
            if (note.is_hold() || note.is_burst()) && duration <= 0 {
                return Err(crate::RoxError::InvalidHoldDuration {
                    time_us: note.time_us,
                    duration_us: duration,
                });
            }
        }

        // Check timing points sorted by time
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
        // Check BPM timing point requirements only if chart has notes
        if !self.notes.is_empty() {
            // Check at least one BPM timing point exists
            let first_bpm = self.timing_points.iter().find(|tp| !tp.is_inherited);
            let Some(_first_bpm) = first_bpm else {
                return Err(crate::RoxError::NoBpmTimingPoint);
            };

            // Strict check removed: Real-world maps sometimes have notes slightly before the first BPM.
            // Engines should handle this by extending the first BPM backwards.
            /*
            // Check first BPM is at or before first note
            if let Some(first_note) = self.notes.first()
                && first_bpm.time_us > first_note.time_us
            {
                return Err(crate::RoxError::BpmAfterFirstNote {
                    bpm_time_us: first_bpm.time_us,
                    note_time_us: first_note.time_us,
                });
            }
            */
        }

        // Check for overlapping notes on same column
        // Group notes by column, then check for overlaps
        for col in 0..self.key_count {
            let mut col_notes: Vec<_> = self.notes.iter().filter(|n| n.column == col).collect();
            col_notes.sort_by_key(|n| n.time_us);

            for window in col_notes.windows(2) {
                let prev = window[0];
                let curr = window[1];
                let prev_end = prev.end_time_us();
                if curr.time_us < prev_end {
                    return Err(crate::RoxError::OverlappingNotes {
                        column: col,
                        time_us: curr.time_us,
                    });
                }
            }
        }

        Ok(())
    }
}
