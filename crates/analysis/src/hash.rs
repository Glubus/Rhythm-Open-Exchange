use rkyv::rancor::Error as RkyvError;
use rox::model::RoxChart;
use xxhash_rust::xxh3::xxh3_128;

/// Compute xxh3-128 hash of the full chart (rkyv-serialized).
pub fn hash(chart: &RoxChart) -> String {
    let bytes = rkyv::to_bytes::<RkyvError>(chart).unwrap_or_default();
    format!("{:032x}", xxh3_128(&bytes))
}

/// Compute xxh3-128 hash of notes only.
pub fn notes_hash(chart: &RoxChart) -> String {
    let bytes = rkyv::to_bytes::<RkyvError>(&chart.notes).unwrap_or_default();
    format!("{:032x}", xxh3_128(&bytes))
}

/// Compute xxh3-128 hash of timing points only.
pub fn timings_hash(chart: &RoxChart) -> String {
    let bytes = rkyv::to_bytes::<RkyvError>(&chart.timing_points).unwrap_or_default();
    format!("{:032x}", xxh3_128(&bytes))
}

/// First 16 hex chars of the full hash (64-bit prefix).
pub fn short_hash(chart: &RoxChart) -> String {
    hash(chart)[..16].to_string()
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
