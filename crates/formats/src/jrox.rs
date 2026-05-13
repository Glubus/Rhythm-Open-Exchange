#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{format, string::ToString, vec::Vec};

use rox::codec::{Decoder, Encoder};
use rox::error::{RoxError, RoxResult};
use rox::model::RoxChart;
use rox_macros::Format;

const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// JROX (JSON ROX) decoder.
#[derive(Format)]
#[format(extensions = ["jrox"])]
pub struct JroxDecoder;

/// JROX (JSON ROX) encoder.
#[derive(Format)]
#[format(extensions = ["jrox"])]
pub struct JroxEncoder;

impl Decoder for JroxDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        if data.len() > MAX_FILE_SIZE {
            return Err(RoxError::InvalidFormat(format!(
                "File too large: {} bytes (max 100MB)",
                data.len()
            )));
        }
        serde_json::from_slice(data)
            .map_err(|e| RoxError::InvalidFormat(format!("JROX parse error: {e}")))
    }
}

impl Encoder for JroxEncoder {
    fn encode_inner(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        serde_json::to_vec_pretty(chart).map_err(|e| RoxError::Serialize(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::prelude::*;
    use rstest::rstest;

    #[rstest]
    fn test_jrox_roundtrip() {
        let mut chart = RoxChart::new(4);
        chart.metadata.title = "Test".into();
        chart.timing_points.push(TimingPoint::bpm(0, 180.0));
        chart.notes.push(Note::tap(1_000_000, 0));

        let encoded = JroxEncoder::encode(&chart).expect("encode failed");
        let decoded = JroxDecoder::decode(&encoded).expect("decode failed");

        assert_eq!(chart.key_count, decoded.key_count);
        assert_eq!(chart.metadata.title, decoded.metadata.title);
        assert_eq!(chart.notes.len(), decoded.notes.len());
    }

    #[rstest]
    fn test_jrox_file_too_large() {
        let big = vec![0u8; 100 * 1024 * 1024 + 1];
        assert!(JroxDecoder::decode(&big).is_err());
    }

    #[rstest]
    fn test_jrox_extensions() {
        use rox::codec::Format;
        assert!(JroxDecoder::supports_extension("jrox"));
        assert!(JroxDecoder::supports_extension("JROX"));
        assert!(!JroxDecoder::supports_extension("osu"));
    }
}
