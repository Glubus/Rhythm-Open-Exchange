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

impl Decoder for JroxDecoder {
    fn decode(data: &[u8]) -> RoxResult<RoxChart> {
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
}
