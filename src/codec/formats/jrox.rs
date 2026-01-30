//! JSON-based ROX format (JROX).
//!
//! Useful for debugging and manual editing.

use crate::codec::{Decoder, Encoder, Format};
use crate::error::{RoxError, RoxResult};
use crate::model::RoxChart;

/// JROX (JSON ROX) decoder.
pub struct JroxDecoder;

/// JROX (JSON ROX) encoder.
pub struct JroxEncoder;

// Safety limit: 100MB to prevent memory exhaustion
const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

impl Decoder for JroxDecoder {
    fn decode(data: &[u8]) -> RoxResult<RoxChart> {
        if data.len() > MAX_FILE_SIZE {
            return Err(RoxError::InvalidFormat(format!(
                "File too large: {} bytes (max {}MB)",
                data.len(),
                MAX_FILE_SIZE / 1024 / 1024
            )));
        }
        serde_json::from_slice(data).map_err(|e| RoxError::InvalidFormat(e.to_string()))
    }
}

impl Encoder for JroxEncoder {
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        serde_json::to_vec_pretty(chart).map_err(|e| RoxError::Serialize(e.to_string()))
    }
}

impl Format for JroxDecoder {
    const EXTENSIONS: &'static [&'static str] = &["jrox"];
}

impl Format for JroxEncoder {
    const EXTENSIONS: &'static [&'static str] = &["jrox"];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{Decoder, Encoder};
    use crate::model::RoxChart;

    #[test]
    fn test_jrox_roundtrip() {
        let mut chart = RoxChart::new(4);
        chart.metadata.title = "Jrox Test".to_string();

        let encoded = JroxEncoder::encode(&chart).unwrap();
        let decoded = JroxDecoder::decode(&encoded).unwrap();

        assert_eq!(chart.key_count(), decoded.key_count());
        assert_eq!(chart.metadata.title, decoded.metadata.title);
    }

    #[test]
    fn test_file_too_large() {
        let big_data = vec![0; MAX_FILE_SIZE + 1];
        let result = JroxDecoder::decode(&big_data);
        assert!(
            matches!(result, Err(RoxError::InvalidFormat(msg)) if msg.contains("File too large"))
        );
    }
}
