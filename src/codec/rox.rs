//! Native ROX binary codec.

use bincode::config;

use crate::error::{RoxError, RoxResult};
use crate::model::{ROX_MAGIC, RoxChart};

use super::{Decoder, Encoder};

/// Native ROX format codec using bincode for compact binary serialization.
pub struct RoxCodec;

impl RoxCodec {
    /// Get the bincode configuration for ROX format.
    fn config() -> impl config::Config {
        config::standard()
            .with_little_endian()
            .with_variable_int_encoding()
    }
}

impl Decoder for RoxCodec {
    fn decode(data: &[u8]) -> RoxResult<RoxChart> {
        // Check magic bytes
        if data.len() < 4 || data[..4] != ROX_MAGIC {
            return Err(RoxError::InvalidFormat(
                "Invalid ROX file: missing magic bytes".into(),
            ));
        }

        // Decode the chart (skip magic bytes)
        let (chart, _): (RoxChart, _) = bincode::decode_from_slice(&data[4..], Self::config())?;

        Ok(chart)
    }
}

impl Encoder for RoxCodec {
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        // Validate before encoding
        chart.validate()?;

        // Start with magic bytes
        let mut data = ROX_MAGIC.to_vec();

        // Encode the chart
        let encoded = bincode::encode_to_vec(chart, Self::config())?;
        data.extend(encoded);

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Metadata, Note, TimingPoint};

    #[test]
    fn test_roundtrip() {
        let mut chart = RoxChart::new(4);
        chart.metadata = Metadata {
            title: "Test Song".into(),
            artist: "Test Artist".into(),
            creator: "Mapper".into(),
            difficulty_name: "Hard".into(),
            difficulty_value: Some(5.5),
            audio_file: "audio.mp3".into(),
            background_file: Some("bg.png".into()),
            preview_time_us: 30_000_000, // 30 seconds
            ..Default::default()
        };
        chart.timing_points.push(TimingPoint::bpm(0, 180.0));
        chart.timing_points.push(TimingPoint::sv(60_000_000, 1.5));
        chart.notes.push(Note::tap(1_000_000, 0));
        chart.notes.push(Note::tap(1_500_000, 1));
        chart.notes.push(Note::hold(2_000_000, 1_000_000, 2)); // 1s duration

        // Encode
        let encoded = RoxCodec::encode(&chart).expect("Failed to encode");

        // Check magic bytes
        assert_eq!(&encoded[..4], &ROX_MAGIC);

        // Decode
        let decoded = RoxCodec::decode(&encoded).expect("Failed to decode");

        // Verify equality
        assert_eq!(chart, decoded);
    }

    #[test]
    fn test_invalid_magic() {
        let bad_data = [0x00, 0x00, 0x00, 0x00, 0x01];
        let result = RoxCodec::decode(&bad_data);
        assert!(result.is_err());
    }
}
