#![warn(clippy::pedantic)]
use rkyv::rancor::Error as RkyvError;
use rox::model::{NoteType, RoxChart};
use xxhash_rust::xxh3::xxh3_128;

use crate::analysis::bpm::bpm_mode;

/// Compute xxh3-128 hash of the full chart (rkyv-serialized).
///
/// # Panics
///
/// Panics if rkyv serialization fails (should never happen for valid `RoxChart` data).
#[must_use]
pub fn hash(chart: &RoxChart) -> String {
    let bytes = rkyv::to_bytes::<RkyvError>(chart).expect("rkyv serialization must not fail");
    format!("{:032x}", xxh3_128(&bytes))
}

/// Compute xxh3-128 hash of notes only.
///
/// # Panics
///
/// Panics if rkyv serialization fails (should never happen for valid `RoxChart` data).
#[must_use]
pub fn notes_hash(chart: &RoxChart) -> String {
    let bytes =
        rkyv::to_bytes::<RkyvError>(&chart.notes).expect("rkyv serialization must not fail");
    format!("{:032x}", xxh3_128(&bytes))
}

/// Compute xxh3-128 hash of timing points only.
///
/// # Panics
///
/// Panics if rkyv serialization fails (should never happen for valid `RoxChart` data).
#[must_use]
pub fn timings_hash(chart: &RoxChart) -> String {
    let bytes = rkyv::to_bytes::<RkyvError>(&chart.timing_points)
        .expect("rkyv serialization must not fail");
    format!("{:032x}", xxh3_128(&bytes))
}

/// First 16 hex chars of the full hash (64-bit prefix).
#[must_use]
pub fn short_hash(chart: &RoxChart) -> String {
    hash(chart)[..16].to_string()
}

/// Hash notes after scaling timings so dominant BPM = `target_bpm`.
/// Stable across rate variants of the same chart for the same `target_bpm`.
#[must_use]
pub fn normalized_notes_hash(chart: &RoxChart, target_bpm: f64) -> String {
    let mode = bpm_mode(chart);
    if mode <= 0.0 || target_bpm <= 0.0 {
        return notes_hash(chart);
    }
    let mult = target_bpm / mode;
    let mut scaled = chart.clone();
    for note in &mut scaled.notes {
        note.time_us = (note.time_us as f64 * mult) as i64;
        match &mut note.note_type {
            NoteType::Hold { duration_us } | NoteType::Burst { duration_us } => {
                *duration_us = (*duration_us as f64 * mult) as i64;
            }
            NoteType::Tap | NoteType::Mine => {}
        }
    }
    notes_hash(&scaled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::model::{Note, TimingPoint};
    use rstest::{fixture, rstest};

    #[fixture]
    fn chart_with_notes() -> RoxChart {
        let mut chart = RoxChart::new(4);
        chart.notes.push(Note::tap(1_000_000, 0));
        chart.notes.push(Note::tap(2_000_000, 1));
        chart.notes.push(Note::hold(3_000_000, 500_000, 2));
        chart
    }

    #[rstest]
    fn test_hash_returns_32_hex_chars(chart_with_notes: RoxChart) {
        let h = hash(&chart_with_notes);
        assert_eq!(h.len(), 32);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[rstest]
    fn test_hash_is_deterministic(chart_with_notes: RoxChart) {
        assert_eq!(hash(&chart_with_notes), hash(&chart_with_notes));
    }

    #[rstest]
    fn test_hash_changes_with_content(chart_with_notes: RoxChart) {
        let mut other = chart_with_notes.clone();
        other.notes.push(Note::tap(9_000_000, 3));
        assert_ne!(hash(&chart_with_notes), hash(&other));
    }

    #[rstest]
    fn test_notes_hash_ignores_timing_points(chart_with_notes: RoxChart) {
        let mut with_tp = chart_with_notes.clone();
        with_tp.timing_points.push(TimingPoint::bpm(0, 180.0));
        // notes_hash must be identical — timing change should not affect it
        assert_eq!(notes_hash(&chart_with_notes), notes_hash(&with_tp));
    }

    #[rstest]
    fn test_timings_hash_ignores_notes(chart_with_notes: RoxChart) {
        let mut with_extra_note = chart_with_notes.clone();
        with_extra_note.notes.push(Note::tap(9_000_000, 3));
        assert_eq!(
            timings_hash(&chart_with_notes),
            timings_hash(&with_extra_note)
        );
    }

    #[rstest]
    fn test_short_hash_is_16_chars(chart_with_notes: RoxChart) {
        let s = short_hash(&chart_with_notes);
        assert_eq!(s.len(), 16);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[rstest]
    fn test_short_hash_is_prefix_of_hash(chart_with_notes: RoxChart) {
        let h = hash(&chart_with_notes);
        let s = short_hash(&chart_with_notes);
        assert!(h.starts_with(&s));
    }
}
