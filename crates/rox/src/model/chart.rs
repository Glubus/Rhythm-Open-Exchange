#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

use super::{Hitsound, Metadata, Note, TimingPoint};

/// Current ROX binary format version.
pub const ROX_VERSION: u8 = 3;

/// Magic bytes identifying ROX files: "ROX\0"
pub const ROX_MAGIC: [u8; 4] = [0x52, 0x4F, 0x58, 0x00];

/// A complete VSRG chart in ROX format.
#[derive(Debug, Clone, PartialEq)]
#[derive(Archive, Serialize, Deserialize)]
#[derive(SerdeSerialize, SerdeDeserialize)]
pub struct RoxChart {
    pub version: u8,
    pub key_count: u8,
    pub metadata: Metadata,
    pub timing_points: Vec<TimingPoint>,
    pub notes: Vec<Note>,
    pub hitsounds: Vec<Hitsound>,
}

impl RoxChart {
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

    #[must_use]
    pub fn duration_us(&self) -> i64 {
        self.notes.iter().map(Note::end_time_us).max().unwrap_or(0)
    }

    #[must_use]
    pub fn note_count(&self) -> usize {
        self.notes.len()
    }

    /// Validates the chart. Called automatically by `Encoder::encode` and `Decoder::decode`.
    ///
    /// # Errors
    ///
    /// Returns the first validation error found.
    pub fn validate(&self) -> crate::error::RoxResult<()> {
        crate::codec::validate::validate(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Note;
    use rstest::{fixture, rstest};

    #[fixture]
    fn empty_4k() -> RoxChart {
        RoxChart::new(4)
    }

    #[rstest]
    fn test_new_sets_version_and_key_count(empty_4k: RoxChart) {
        assert_eq!(empty_4k.version, ROX_VERSION);
        assert_eq!(empty_4k.key_count, 4);
        assert!(empty_4k.notes.is_empty());
        assert!(empty_4k.timing_points.is_empty());
        assert!(empty_4k.hitsounds.is_empty());
    }

    #[rstest]
    fn test_duration_empty_chart_is_zero(empty_4k: RoxChart) {
        assert_eq!(empty_4k.duration_us(), 0);
    }

    #[rstest]
    #[case(vec![Note::tap(1_000_000, 0), Note::hold(2_000_000, 500_000, 1)], 2_500_000)]
    #[case(vec![Note::tap(500_000, 0)], 500_000)]
    fn test_duration_us(#[case] notes: Vec<Note>, #[case] expected: i64) {
        let mut chart = RoxChart::new(4);
        chart.notes = notes;
        assert_eq!(chart.duration_us(), expected);
    }

    #[rstest]
    fn test_note_count(empty_4k: RoxChart) {
        let mut chart = empty_4k;
        assert_eq!(chart.note_count(), 0);
        chart.notes.push(Note::tap(0, 0));
        assert_eq!(chart.note_count(), 1);
    }
}
