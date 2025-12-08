//! Encoder and Decoder traits for format conversion.

use std::path::Path;

use crate::error::RoxResult;
use crate::model::RoxChart;

/// Trait for decoding from external formats to ROX.
pub trait Decoder {
    /// Decode a chart from raw bytes.
    fn decode(data: &[u8]) -> RoxResult<RoxChart>;

    /// Decode a chart from a file path.
    fn decode_from_path(path: impl AsRef<Path>) -> RoxResult<RoxChart> {
        let data = std::fs::read(path)?;
        Self::decode(&data)
    }
}

/// Trait for encoding from ROX to external formats.
pub trait Encoder {
    /// Encode a chart to raw bytes.
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>>;

    /// Encode a chart to a file path.
    fn encode_to_path(chart: &RoxChart, path: impl AsRef<Path>) -> RoxResult<()> {
        let data = Self::encode(chart)?;
        std::fs::write(path, data)?;
        Ok(())
    }
}
