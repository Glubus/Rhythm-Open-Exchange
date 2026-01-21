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
        let s = std::str::from_utf8(data)
            .map_err(|e| RoxError::InvalidFormat(format!("Invalid UTF-8: {}", e)))?;
        serde_yaml::from_str(s).map_err(|e| RoxError::InvalidFormat(e.to_string()))
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
    use super::*;
    use crate::codec::{Decoder, Encoder};
    use crate::model::RoxChart;

    #[test]
    fn test_yrox_roundtrip() {
        let mut chart = RoxChart::new(4);
        chart.metadata.title = "Yrox Test".to_string();

        let encoded = YroxEncoder::encode(&chart).unwrap();
        let decoded = YroxDecoder::decode(&encoded).unwrap();

        assert_eq!(chart.key_count(), decoded.key_count());
        assert_eq!(chart.metadata.title, decoded.metadata.title);
    }
}
