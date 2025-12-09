//! Hitsound definitions for keysounded charts.

use bincode::{Decode, Encode};

/// A hitsound sample definition.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct Hitsound {
    /// Relative path to the audio sample.
    pub file: String,
    /// Volume (0-100, optional override).
    pub volume: Option<u8>,
}

impl Hitsound {
    /// Create a new hitsound with default volume.
    #[must_use]
    pub fn new(file: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            volume: None,
        }
    }

    /// Create a hitsound with custom volume.
    #[must_use]
    pub fn with_volume(file: impl Into<String>, volume: u8) -> Self {
        Self {
            file: file.into(),
            volume: Some(volume.min(100)),
        }
    }
}
