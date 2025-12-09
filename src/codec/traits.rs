//! Encoder and Decoder traits for format conversion.

use std::path::Path;

use crate::error::RoxResult;
use crate::model::RoxChart;

/// Trait for decoding from external formats to ROX.
pub trait Decoder {
    /// Decode a chart from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the data is invalid or cannot be parsed.
    fn decode(data: &[u8]) -> RoxResult<RoxChart>;

    /// Decode a chart from a file path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or contains invalid data.
    fn decode_from_path(path: impl AsRef<Path>) -> RoxResult<RoxChart> {
        let data = std::fs::read(path)?;
        Self::decode(&data)
    }
}

/// Trait for encoding from ROX to external formats.
pub trait Encoder {
    /// Encode a chart to raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the chart is invalid or encoding fails.
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>>;

    /// Encode a chart to a file path.
    ///
    /// # Errors
    ///
    /// Returns an error if encoding fails or the file cannot be written.
    fn encode_to_path(chart: &RoxChart, path: impl AsRef<Path>) -> RoxResult<()> {
        let data = Self::encode(chart)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Encode a chart to a String (for text-based formats like .osu).
    ///
    /// # Errors
    ///
    /// Returns an error if encoding fails or the output is not valid UTF-8.
    fn encode_to_string(chart: &RoxChart) -> RoxResult<String> {
        let data = Self::encode(chart)?;
        String::from_utf8(data)
            .map_err(|e| crate::error::RoxError::InvalidFormat(format!("Invalid UTF-8: {e}")))
    }
}
