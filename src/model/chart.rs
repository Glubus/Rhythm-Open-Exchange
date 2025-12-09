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

    /// Validate the chart (check column bounds, etc.)
    ///
    /// # Errors
    ///
    /// Returns an error if any note has a column index >= `key_count`.
    pub fn validate(&self) -> Result<(), crate::RoxError> {
        for note in &self.notes {
            if note.column >= self.key_count {
                return Err(crate::RoxError::InvalidColumn {
                    column: note.column,
                    key_count: self.key_count,
                });
            }
        }
        Ok(())
    }
}
