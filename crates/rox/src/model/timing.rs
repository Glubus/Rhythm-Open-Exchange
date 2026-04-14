use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

/// A timing point — either a BPM change or a scroll velocity change.
#[derive(Debug, Clone, PartialEq)]
#[derive(Archive, Serialize, Deserialize)]
#[derive(SerdeSerialize, SerdeDeserialize)]
#[serde(tag = "type", content = "data")]
pub enum TimingPoint {
    Bpm { time_us: i64, bpm: f32, signature: u8 },
    Sv  { time_us: i64, scroll_speed: f32 },
}

impl TimingPoint {
    /// Create a BPM timing point with 4/4 time signature.
    #[must_use]
    pub fn bpm(time_us: i64, bpm: f32) -> Self {
        Self::Bpm { time_us, bpm, signature: 4 }
    }

    /// Create a scroll velocity change point.
    #[must_use]
    pub fn sv(time_us: i64, scroll_speed: f32) -> Self {
        Self::Sv { time_us, scroll_speed }
    }

    #[must_use]
    pub fn time_us(&self) -> i64 {
        match self {
            Self::Bpm { time_us, .. } | Self::Sv { time_us, .. } => *time_us,
        }
    }

    #[must_use]
    pub fn is_bpm(&self) -> bool { matches!(self, Self::Bpm { .. }) }

    #[must_use]
    pub fn is_sv(&self) -> bool { matches!(self, Self::Sv { .. }) }

    #[must_use]
    pub fn bpm_value(&self) -> Option<f32> {
        match self { Self::Bpm { bpm, .. } => Some(*bpm), Self::Sv { .. } => None }
    }

    #[must_use]
    pub fn scroll_speed(&self) -> Option<f32> {
        match self { Self::Sv { scroll_speed, .. } => Some(*scroll_speed), Self::Bpm { .. } => None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(TimingPoint::bpm(0, 180.0), 0, true, Some(180.0), None)]
    #[case(TimingPoint::sv(1_000_000, 1.5), 1_000_000, false, None, Some(1.5))]
    #[case(TimingPoint::bpm(500_000, 120.0), 500_000, true, Some(120.0), None)]
    fn test_timing_point_accessors(
        #[case] tp: TimingPoint,
        #[case] expected_time: i64,
        #[case] is_bpm: bool,
        #[case] bpm_val: Option<f32>,
        #[case] sv_val: Option<f32>,
    ) {
        assert_eq!(tp.time_us(), expected_time);
        assert_eq!(tp.is_bpm(), is_bpm);
        assert_eq!(tp.is_sv(), !is_bpm);
        assert_eq!(tp.bpm_value(), bpm_val);
        assert_eq!(tp.scroll_speed(), sv_val);
    }

    #[test]
    fn test_bpm_default_signature_is_4() {
        let TimingPoint::Bpm { signature, .. } = TimingPoint::bpm(0, 120.0) else {
            panic!("expected Bpm variant");
        };
        assert_eq!(signature, 4);
    }
}
