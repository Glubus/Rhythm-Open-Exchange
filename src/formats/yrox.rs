#![warn(clippy::pedantic)]
use rox::codec::{Decoder, Encoder};
use rox::error::{RoxError, RoxResult};
use rox::model::RoxChart;

const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// YROX (YAML ROX) decoder.
pub struct YroxDecoder;
crate::impl_format!(YroxDecoder, ["yrox"]);

/// YROX (YAML ROX) encoder.
pub struct YroxEncoder;
crate::impl_format!(YroxEncoder, ["yrox"]);

impl Decoder for YroxDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        if data.len() > MAX_FILE_SIZE {
            return Err(RoxError::InvalidFormat(format!(
                "File too large: {} bytes (max 100MB)",
                data.len()
            )));
        }
        serde_yaml::from_slice(data)
            .map_err(|e| RoxError::InvalidFormat(format!("YROX parse error: {e}")))
    }
}

impl Encoder for YroxEncoder {
    fn encode_inner(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        let s = serde_yaml::to_string(chart).map_err(|e| RoxError::Serialize(e.to_string()))?;
        Ok(s.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::prelude::*;
    use rstest::rstest;

    #[rstest]
    fn test_yrox_roundtrip() {
        let mut chart = RoxChart::new(7);
        chart.metadata.artist = "Test Artist".into();
        chart.timing_points.push(TimingPoint::bpm(0, 140.0));
        chart.notes.push(Note::tap(500_000, 3));

        let encoded = YroxEncoder::encode(&chart).expect("encode failed");
        let decoded = YroxDecoder::decode(&encoded).expect("decode failed");

        assert_eq!(chart.key_count, decoded.key_count);
        assert_eq!(chart.metadata.artist, decoded.metadata.artist);
    }

    #[rstest]
    fn test_yrox_file_too_large() {
        let big = vec![0u8; 100 * 1024 * 1024 + 1];
        assert!(YroxDecoder::decode(&big).is_err());
    }
}
