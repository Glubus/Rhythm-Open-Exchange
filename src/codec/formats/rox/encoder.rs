use std::io::Write;

use rkyv::rancor::Error as RkyvError;

use crate::codec::Encoder;
use crate::error::{RoxError, RoxResult};
use crate::model::{RoxChart, ROX_MAGIC};

use super::RoxCodec;

/// Compression level for zstd (1-22, higher = better compression but slower).
/// Level 3 provides fast compression with good ratio.
#[cfg(not(target_arch = "wasm32"))]
const COMPRESSION_LEVEL: i32 = 3;

/// Compress data (zstd on native, passthrough on WASM).
#[cfg(not(target_arch = "wasm32"))]
fn compress(data: &[u8]) -> RoxResult<Vec<u8>> {
    let mut encoder = zstd::stream::Encoder::new(Vec::new(), COMPRESSION_LEVEL)?;
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

#[cfg(target_arch = "wasm32")]
fn compress(data: &[u8]) -> RoxResult<Vec<u8>> {
    // No compression on WASM - just return data as-is
    Ok(data.to_vec())
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

impl Encoder for RoxCodec {
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        // Validate before encoding
        chart.validate()?;

        // Apply delta encoding for better compression
        let delta_chart = delta_encode_notes(chart);

        // Serialize the chart with rkyv
        let encoded = rkyv::to_bytes::<RkyvError>(&delta_chart)
            .map_err(|e| RoxError::Serialize(e.to_string()))?;

        // Compress the encoded data
        let compressed = compress(&encoded)?;

        // Start with magic bytes, then compressed data
        let mut data = ROX_MAGIC.to_vec();
        data.extend(compressed);

        Ok(data)
    }
}
