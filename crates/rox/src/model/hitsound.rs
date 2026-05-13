use compact_str::CompactString;
use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

/// A hitsound sample definition.
#[derive(
    Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize,
)]
pub struct Hitsound {
    /// Relative path to the audio sample.
    pub file: CompactString,
    /// Volume override (0–100). `None` means use default.
    pub volume: Option<u8>,
}

impl Hitsound {
    #[must_use]
    pub fn new(file: impl Into<CompactString>) -> Self {
        Self {
            file: file.into(),
            volume: None,
        }
    }

    #[must_use]
    pub fn with_volume(file: impl Into<CompactString>, volume: u8) -> Self {
        Self {
            file: file.into(),
            volume: Some(volume.min(100)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(150, 100)]
    #[case(100, 100)]
    #[case(50, 50)]
    #[case(0, 0)]
    fn test_volume_clamped_to_100(#[case] input: u8, #[case] expected: u8) {
        assert_eq!(Hitsound::with_volume("f.wav", input).volume, Some(expected));
    }

    #[test]
    fn test_new_has_no_volume() {
        assert!(Hitsound::new("kick.wav").volume.is_none());
    }
}
