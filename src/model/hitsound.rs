//! Hitsound definitions for keysounded charts.

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
    /// Volume (0-100, optional override).
    pub volume: Option<u8>,
}

impl Hitsound {
    /// Create a new hitsound with default volume.
    #[must_use]
    pub fn new(file: impl Into<CompactString>) -> Self {
        Self {
            file: file.into(),
            volume: None,
        }
    }

    /// Create a hitsound with custom volume.
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

    #[test]
    fn test_hitsound_new() {
        let hs = Hitsound::new("kick.wav");

        assert_eq!(hs.file, "kick.wav");
        assert!(hs.volume.is_none());
    }

    #[test]
    fn test_hitsound_with_volume() {
        let hs = Hitsound::with_volume("snare.ogg", 75);

        assert_eq!(hs.file, "snare.ogg");
        assert_eq!(hs.volume, Some(75));
    }

    #[test]
    fn test_hitsound_volume_clamped_to_100() {
        let hs = Hitsound::with_volume("loud.wav", 150);

        assert_eq!(hs.volume, Some(100));
    }
}
