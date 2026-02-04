//! Timing points for BPM and scroll velocity changes.

use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

/// A point that defines timing or scroll velocity changes.
#[derive(
    Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize,
)]
pub struct TimingPoint {
    /// Position in microseconds.
    pub time_us: i64,
    /// Beats per minute (only meaningful if not inherited).
    pub bpm: f32,
    /// Time signature numerator (e.g., 4 for 4/4 time).
    pub signature: u8,
    /// If true, this is a scroll velocity change, not a BPM change.
    pub is_inherited: bool,
    /// Scroll velocity multiplier (1.0 = normal speed).
    pub scroll_speed: f32,
}

impl TimingPoint {
    /// Create a new BPM timing point.
    #[must_use]
    pub fn bpm(time_us: i64, bpm: f32) -> Self {
        Self {
            time_us,
            bpm,
            signature: 4,
            is_inherited: false,
            scroll_speed: 1.0,
        }
    }

    /// Create a scroll velocity change point.
    #[must_use]
    pub fn sv(time_us: i64, scroll_speed: f32) -> Self {
        Self {
            time_us,
            bpm: 0.0,
            signature: 4,
            is_inherited: true,
            scroll_speed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timing_point_bpm() {
        let tp = TimingPoint::bpm(0, 180.0);

        assert_eq!(tp.time_us, 0);
        assert_eq!(tp.bpm, 180.0);
        assert_eq!(tp.signature, 4);
        assert!(!tp.is_inherited);
        assert_eq!(tp.scroll_speed, 1.0);
    }

    #[test]
    fn test_timing_point_sv() {
        let tp = TimingPoint::sv(1_000_000, 1.5);

        assert_eq!(tp.time_us, 1_000_000);
        assert_eq!(tp.bpm, 0.0);
        assert_eq!(tp.signature, 4);
        assert!(tp.is_inherited);
        assert_eq!(tp.scroll_speed, 1.5);
    }
}
