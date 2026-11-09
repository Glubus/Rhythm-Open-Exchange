#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec};

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

    /// Sorts the notes and drops the ones a game can not play on top of another of their column (see [`drop_stacked`]). Returns
    /// how many were dropped. Decoders of formats whose files carry such stacks (FNF charts, Quaver maps) call it so that the chart
    /// passes [`Self::validate`] instead of being refused.
    pub fn drop_overlapping_notes(&mut self) -> usize {
        drop_stacked(&mut self.notes, self.key_count, |note| note).total()
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

/// What [`drop_stacked`] dropped.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stacked {
    /// Taps at the same time as the tap kept before them on their column.
    pub taps: usize,
    /// The other notes: starting inside a long note of their column, or at the same time as a long note or a mine there.
    pub overlapping: usize,
}

impl Stacked {
    #[must_use]
    pub fn total(self) -> usize {
        self.taps + self.overlapping
    }
}

/// Sorts `items` by the time of their note (at one time long notes first, then taps, then mines, then by column) and drops the notes a
/// game can not play on top of another of their column: a note at the same time as the one kept before it, or starting inside one of
/// its long notes (a note may start exactly where a long note ends). `note` gives the note of an item, so decoders can keep what goes
/// with it (a keysound) until the stacks are gone. Columns at or past `key_count` are kept as they are.
pub fn drop_stacked<T>(items: &mut Vec<T>, key_count: u8, note: impl Fn(&T) -> &Note) -> Stacked {
    items.sort_by_key(|item| {
        let note = note(item);
        (note.time_us, u8::from(!note.is_hold()) + u8::from(note.is_mine()), note.column)
    });
    // Per column: the start and the end of the note kept last, and whether it is a tap.
    let mut last = vec![(i64::MIN, i64::MIN, false); usize::from(key_count)];
    let mut dropped = Stacked::default();
    items.retain(|item| {
        let note = note(item);
        let Some((start, end, tap)) = last.get_mut(usize::from(note.column)) else { return true };
        let is_tap = !note.is_hold() && !note.is_mine() && !note.is_burst();
        if note.time_us == *start && *tap && is_tap {
            dropped.taps += 1;
            return false;
        }
        if note.time_us == *start || note.time_us < *end {
            dropped.overlapping += 1;
            return false;
        }
        (*start, *end, *tap) = (note.time_us, note.end_time_us(), is_tap);
        true
    });
    dropped
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
