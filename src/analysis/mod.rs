pub mod bpm;
pub mod hash;
pub mod nps;

pub use bpm::{bpm_max, bpm_min, bpm_mode};
pub use hash::{hash, notes_hash, timings_hash};
pub use nps::{density, highest_drain_time, highest_nps, lowest_nps, nps};

use crate::model::RoxChart;

/// Extension trait to add analysis methods directly to `RoxChart`.
pub trait RoxAnalysis {
    fn bpm_min(&self) -> f64;
    fn bpm_max(&self) -> f64;
    fn bpm_mode(&self) -> f64;

    fn nps(&self) -> f64;
    fn density(&self, segments: usize) -> Vec<f64>;
    fn highest_nps(&self, window_size_s: f64) -> f64;
    fn lowest_nps(&self, window_size_s: f64) -> f64;
    fn highest_drain_time(&self) -> f64;

    fn hash(&self) -> String;
    fn notes_hash(&self) -> String;
    fn timings_hash(&self) -> String;
    fn short_hash(&self) -> String;
}

impl RoxAnalysis for RoxChart {
    fn bpm_min(&self) -> f64 {
        bpm::bpm_min(self)
    }
    fn bpm_max(&self) -> f64 {
        bpm::bpm_max(self)
    }
    fn bpm_mode(&self) -> f64 {
        bpm::bpm_mode(self)
    }

    fn nps(&self) -> f64 {
        nps::nps(self)
    }
    fn density(&self, segments: usize) -> Vec<f64> {
        nps::density(self, segments)
    }
    fn highest_nps(&self, window_size_s: f64) -> f64 {
        nps::highest_nps(self, window_size_s)
    }
    fn lowest_nps(&self, window_size_s: f64) -> f64 {
        nps::lowest_nps(self, window_size_s)
    }
    fn highest_drain_time(&self) -> f64 {
        nps::highest_drain_time(self)
    }

    fn hash(&self) -> String {
        hash::hash(self)
    }
    fn notes_hash(&self) -> String {
        hash::notes_hash(self)
    }
    fn timings_hash(&self) -> String {
        hash::timings_hash(self)
    }
    fn short_hash(&self) -> String {
        let h = self.hash();
        if h.len() >= 16 {
            h[..16].to_string()
        } else {
            h
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Note, RoxChart, TimingPoint};

    #[test]
    fn test_bpm_stats() {
        let mut chart = RoxChart::new(4);
        chart.timing_points.push(TimingPoint::bpm(0, 100.0));
        chart
            .timing_points
            .push(TimingPoint::bpm(10_000_000, 200.0)); // At 10s
        chart
            .timing_points
            .push(TimingPoint::bpm(20_000_000, 100.0)); // At 20s

        // Add a note at 30s to define duration
        chart.notes.push(Note::tap(30_000_000, 0)); // Duration 30s

        assert_eq!(chart.bpm_min(), 100.0);
        assert_eq!(chart.bpm_max(), 200.0);

        // 0-10s: 100bpm (10s)
        // 10-20s: 200bpm (10s)
        // 20-30s: 100bpm (10s)
        // Total 100bpm: 20s. Total 200bpm: 10s. Mode should be 100.
        assert_eq!(chart.bpm_mode(), 100.0);
    }

    #[test]
    fn test_nps() {
        let mut chart = RoxChart::new(4);
        chart.notes.push(Note::tap(0, 0));
        chart.notes.push(Note::tap(1_000_000, 0));
        chart.notes.push(Note::tap(2_000_000, 0));
        // Duration 2s.
        assert_eq!(chart.nps(), 1.5);
    }

    #[test]
    fn test_density() {
        let mut chart = RoxChart::new(4);
        // Duration 10s
        chart.notes.push(Note::tap(9_999_999, 0)); // Force duration ~10s

        // Add 10 notes in first 5 seconds (0-5s)
        for i in 0..10 {
            chart.notes.push(Note::tap(i * 500_000, 0));
        }

        let dens = chart.density(2);
        // 2 segments. Each 5s.
        // Segment 1: 10 notes / 5s = 2.0 NPS
        // Segment 2: 1 note (the last one) / 5s = 0.2 NPS

        assert_eq!(dens.len(), 2);
        assert!((dens[0] - 2.0).abs() < 0.001);
        assert!((dens[1] - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_highest_nps() {
        let mut chart = RoxChart::new(4);
        // Cluster of 10 notes within 1 second
        for i in 0..10 {
            chart.notes.push(Note::tap(10_000_000 + i * 50_000, 0)); // 10s to 10.5s
        }
        // Sparse notes elsewhere
        chart.notes.push(Note::tap(0, 0));
        chart.notes.push(Note::tap(20_000_000, 0));

        let peak = chart.highest_nps(1.0);
        // 10 notes in 1s window.
        assert_eq!(peak, 10.0);
    }

    #[test]
    fn test_lowest_nps() {
        let mut chart = RoxChart::new(4);
        // 2 notes at start
        chart.notes.push(Note::tap(0, 0));
        chart.notes.push(Note::tap(1_000_000, 0));

        // Big gap
        chart.notes.push(Note::tap(10_000_000, 0));

        // Window 2s. Gap 9s.
        assert_eq!(chart.lowest_nps(2.0), 0.0);
    }

    #[test]
    fn test_highest_drain_time() {
        let mut chart = RoxChart::new(4);

        // Create a section with 10 NPS for 5 seconds
        // 5 seconds * 10 NPS = 50 notes
        for i in 0..50 {
            // From 1.0s to 6.0s
            chart.notes.push(Note::tap(1_000_000 + i * 100_000, 0));
        }

        // A gap of 4s

        // Another section with 10 NPS for 10 seconds
        for i in 0..100 {
            // From 10.0s to 20.0s
            chart.notes.push(Note::tap(10_000_000 + i * 100_000, 0));
        }

        let drain = chart.highest_drain_time();
        // Sliding window of 1s means we lose about ~0.5-1s at edges where density isn't full yet.
        // 9.2s was observed.
        assert!(drain >= 9.0 && drain <= 10.5, "Drain time was {}", drain);
    }

    #[test]
    fn test_hash_correctness() {
        let mut chart = RoxChart::new(4);
        chart.notes.push(Note::tap(1_000_000, 0));
        chart.notes.push(Note::tap(2_000_000, 1));
        chart.notes.push(Note::hold(3_000_000, 500_000, 2)); // ends at 3.5s

        // Known hash values for this specific chart configuration
        assert_eq!(
            chart.hash(),
            "81066bfbd1e257fd6a4838168b6f7eff7f58657dee4c09aeeb6fd3d76d39753f"
        );
        assert_eq!(
            chart.notes_hash(),
            "d94e91bdf1d5a6813cf9b8beeb4983171abbe64780f8e141b236f6d924d6204f"
        );
        assert_eq!(chart.short_hash(), "81066bfbd1e257fd");
    }

    #[test]
    fn test_hash_extension() {
        let mut chart = RoxChart::new(4);
        chart.notes.push(Note::tap(0, 0));

        // Test direct calling via trait
        let h1 = chart.hash();
        assert!(!h1.is_empty());

        let nh1 = chart.notes_hash();
        assert!(!nh1.is_empty());
    }

    #[test]
    fn test_hash_determinism() {
        let mut chart = RoxChart::new(4);
        chart.metadata.title = "Test".into();
        chart.notes.push(Note::tap(0, 0));

        let hash1 = chart.hash();

        // Same chart should produce same hash
        let mut chart2 = RoxChart::new(4);
        chart2.metadata.title = "Test".into();
        chart2.notes.push(Note::tap(0, 0));

        let hash2 = chart2.hash();
        assert_eq!(hash1, hash2);

        // Different chart should produce different hash
        chart2.notes.push(Note::tap(1_000_000, 1));
        let hash3 = chart2.hash();
        assert_ne!(hash1, hash3);
    }
}
