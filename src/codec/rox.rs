//! Native ROX binary codec with zstd compression.

use std::io::{Read, Write};

use bincode::config;

use crate::error::{RoxError, RoxResult};
use crate::model::{ROX_MAGIC, RoxChart};

use super::{Decoder, Encoder};

/// Compression level for zstd (1-22, higher = better compression but slower).
/// Level 3 provides fast compression with good ratio.
const COMPRESSION_LEVEL: i32 = 3;

/// Native ROX format codec using bincode for compact binary serialization
/// and zstd for compression. Uses delta encoding for note timestamps.
pub struct RoxCodec;

impl RoxCodec {
    /// Get the bincode configuration for ROX format.
    fn config() -> impl config::Config {
        config::standard()
            .with_little_endian()
            .with_variable_int_encoding()
    }

    /// Compress data using zstd.
    fn compress(data: &[u8]) -> RoxResult<Vec<u8>> {
        let mut encoder = zstd::stream::Encoder::new(Vec::new(), COMPRESSION_LEVEL)?;
        encoder.write_all(data)?;
        Ok(encoder.finish()?)
    }

    /// Decompress data using zstd.
    fn decompress(data: &[u8]) -> RoxResult<Vec<u8>> {
        let mut decoder = zstd::stream::Decoder::new(data)?;
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        Ok(decompressed)
    }

    /// Apply delta encoding to note timestamps for better compression.
    /// Returns a chart with delta-encoded timestamps.
    fn delta_encode_notes(chart: &RoxChart) -> RoxChart {
        let mut result = chart.clone();
        let mut last_time: i64 = 0;

        for note in &mut result.notes {
            let original_time = note.time_us;
            note.time_us = original_time - last_time; // Store delta
            last_time = original_time;
        }

        result
    }

    /// Decode delta-encoded note timestamps back to absolute timestamps.
    fn delta_decode_notes(chart: &mut RoxChart) {
        let mut accumulated_time: i64 = 0;

        for note in &mut chart.notes {
            accumulated_time += note.time_us; // Add delta
            note.time_us = accumulated_time;
        }
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

        // Decompress the data after magic bytes
        let decompressed = Self::decompress(&data[4..])?;

        // Decode the chart
        let (mut chart, _): (RoxChart, _) =
            bincode::decode_from_slice(&decompressed, Self::config())?;

        // Restore absolute timestamps from deltas
        Self::delta_decode_notes(&mut chart);

        Ok(chart)
    }
}

impl Encoder for RoxCodec {
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        // Validate before encoding
        chart.validate()?;

        // Apply delta encoding for better compression
        let delta_chart = Self::delta_encode_notes(chart);

        // Encode the chart with bincode
        let encoded = bincode::encode_to_vec(&delta_chart, Self::config())?;

        // Compress the encoded data
        let compressed = Self::compress(&encoded)?;

        // Start with magic bytes, then compressed data
        let mut data = ROX_MAGIC.to_vec();
        data.extend(compressed);

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

    #[test]
    fn test_compression_reduces_size() {
        let mut chart = RoxChart::new(4);
        chart.timing_points.push(TimingPoint::bpm(0, 120.0));

        // Add many similar notes (compresses well)
        for i in 0i64..1000 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let column = (i % 4) as u8;
            chart.notes.push(Note::tap(i * 100_000, column));
        }

        let encoded = RoxCodec::encode(&chart).expect("Failed to encode");

        // With delta encoding, compression should be excellent
        assert!(
            encoded.len() < 2000,
            "Compressed size {} is larger than expected",
            encoded.len()
        );
    }

    #[test]
    fn test_delta_encoding() {
        let mut chart = RoxChart::new(4);
        chart.timing_points.push(TimingPoint::bpm(0, 120.0));
        chart.notes.push(Note::tap(1_000_000, 0));
        chart.notes.push(Note::tap(1_100_000, 1));
        chart.notes.push(Note::tap(1_200_000, 2));
        chart.notes.push(Note::tap(1_300_000, 3));

        let encoded = RoxCodec::encode(&chart).expect("Failed to encode");
        let decoded = RoxCodec::decode(&encoded).expect("Failed to decode");

        // Verify timestamps are correctly restored
        assert_eq!(decoded.notes[0].time_us, 1_000_000);
        assert_eq!(decoded.notes[1].time_us, 1_100_000);
        assert_eq!(decoded.notes[2].time_us, 1_200_000);
        assert_eq!(decoded.notes[3].time_us, 1_300_000);
    }
}
