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
#[derive(
    Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize,
)]
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

    /// Drops every mine and every roll (`Burst`, head and tail together) and keeps all other
    /// notes (taps, holds, with their times and columns) as they are, for games that play
    /// neither. Returns what was removed.
    pub fn remove_unsupported_notes(&mut self) -> RemovedNotes {
        let mut removed = RemovedNotes::default();
        self.notes.retain(|note| {
            if note.is_mine() {
                removed.mines += 1;
            } else if note.is_burst() {
                removed.rolls += 1;
            } else {
                return true;
            }
            false
        });
        removed
    }
}

/// What [`RoxChart::remove_unsupported_notes`] took out of a chart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RemovedNotes {
    pub mines: usize,
    pub rolls: usize,
}

impl RemovedNotes {
    /// Whether nothing was removed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mines == 0 && self.rolls == 0
    }
}

/// "2 mines removed, 1 roll removed": singular or plural, a part with a zero count left out.
impl core::fmt::Display for RemovedNotes {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut first = true;
        for (count, noun) in [(self.mines, "mine"), (self.rolls, "roll")] {
            if count == 0 {
                continue;
            }
            if !first {
                f.write_str(", ")?;
            }
            first = false;
            write!(
                f,
                "{count} {noun}{} removed",
                if count == 1 { "" } else { "s" }
            )?;
        }
        Ok(())
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

    #[rstest]
    fn test_remove_unsupported_notes_keeps_taps_and_holds(empty_4k: RoxChart) {
        let mut chart = empty_4k;
        chart.notes = vec![
            Note::tap(0, 0),
            Note::mine(0, 1),
            Note::hold(500_000, 250_000, 2),
            Note::mine(600_000, 2),
            Note::burst(1_000_000, 200_000, 3),
            Note::tap(1_500_000, 3),
        ];
        let kept = vec![
            chart.notes[0].clone(),
            chart.notes[2].clone(),
            chart.notes[5].clone(),
        ];
        let removed = chart.remove_unsupported_notes();
        assert_eq!(removed, RemovedNotes { mines: 2, rolls: 1 });
        assert_eq!(removed.to_string(), "2 mines removed, 1 roll removed");
        assert_eq!(chart.notes, kept);
        assert!(chart.remove_unsupported_notes().is_empty());
    }

    #[rstest]
    #[case(0, 0, "")]
    #[case(1, 0, "1 mine removed")]
    #[case(0, 3, "3 rolls removed")]
    #[case(1, 1, "1 mine removed, 1 roll removed")]
    #[case(5, 2, "5 mines removed, 2 rolls removed")]
    fn test_removed_notes_notice(#[case] mines: usize, #[case] rolls: usize, #[case] text: &str) {
        assert_eq!(RemovedNotes { mines, rolls }.to_string(), text);
    }
}
