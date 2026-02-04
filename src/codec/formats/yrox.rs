//! YAML-based ROX format (YROX).
//!
//! Useful for human-readable configuration-like charts.

use crate::codec::{Decoder, Encoder, Format};
use crate::error::{RoxError, RoxResult};
use crate::model::RoxChart;

/// YROX (YAML ROX) decoder.
pub struct YroxDecoder;

/// YROX (YAML ROX) encoder.
pub struct YroxEncoder;

impl Decoder for YroxDecoder {
    fn decode(data: &[u8]) -> RoxResult<RoxChart> {
        // Safety limit: 100MB
        const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;
        if data.len() > MAX_FILE_SIZE {
            return Err(RoxError::InvalidFormat(format!(
                "File too large: {} bytes (max {}MB)",
                data.len(),
                MAX_FILE_SIZE / 1024 / 1024
            )));
        }

        let chart: RoxChart = serde_yaml::from_slice(data)
            .map_err(|e| RoxError::InvalidFormat(format!("YROX parse error: {e}")))?;

        Ok(chart)
    }
}

impl Encoder for YroxEncoder {
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        let s = serde_yaml::to_string(chart).map_err(|e| RoxError::Serialize(e.to_string()))?;
        Ok(s.into_bytes())
    }
}

impl Format for YroxDecoder {
    const EXTENSIONS: &'static [&'static str] = &["yrox"];
}

impl Format for YroxEncoder {
    const EXTENSIONS: &'static [&'static str] = &["yrox"];
}

#[cfg(test)]
mod tests {
    use compact_str::ToCompactString;

    use super::*;
    use crate::codec::{Decoder, Encoder};
    use crate::model::RoxChart;

    #[test]
    fn test_yrox_roundtrip() {
        let mut chart = RoxChart::new(4);
        chart.metadata.title = "Yrox Test".to_compact_string();

        let encoded = YroxEncoder::encode(&chart).unwrap();
        let decoded = YroxDecoder::decode(&encoded).unwrap();

        assert_eq!(chart.key_count(), decoded.key_count());
        assert_eq!(chart.metadata.title, decoded.metadata.title);
    }

    #[test]
    fn test_file_too_large() {
        // Safety limit: 100MB to prevent memory exhaustion
        let max_file_size: usize = 100 * 1024 * 1024;
        let big_data = vec![0; max_file_size + 1];
        let result = YroxDecoder::decode(&big_data);
        assert!(
            matches!(result, Err(RoxError::InvalidFormat(msg)) if msg.contains("File too large"))
        );
    }
}
