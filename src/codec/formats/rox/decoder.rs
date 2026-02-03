use std::io::Read;

use rkyv::rancor::Error as RkyvError;

use crate::codec::Decoder;
use crate::error::{RoxError, RoxResult};
use crate::model::{RoxChart, ROX_MAGIC};

use super::{RoxCodec, MAX_FILE_SIZE};

/// Decompress data (zstd on native, passthrough on WASM).
#[cfg(not(target_arch = "wasm32"))]
fn decompress(data: &[u8]) -> RoxResult<Vec<u8>> {
    let mut decoder = zstd::stream::Decoder::new(data)?;
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

#[cfg(target_arch = "wasm32")]
fn decompress(data: &[u8]) -> RoxResult<Vec<u8>> {
    // No compression on WASM - data is already uncompressed
    Ok(data.to_vec())
}

/// Decode delta-encoded note timestamps back to absolute timestamps.
fn delta_decode_notes(chart: &mut RoxChart) {
    let mut accumulated_time: i64 = 0;

    for note in &mut chart.notes {
        accumulated_time += note.time_us; // Add delta
        note.time_us = accumulated_time;
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

        if data.len() > MAX_FILE_SIZE {
            return Err(RoxError::InvalidFormat(format!(
                "File too large: {} bytes (max {}MB)",
                data.len(),
                MAX_FILE_SIZE / 1024 / 1024
            )));
        }

        // Decompress the data after magic bytes
        let decompressed = decompress(&data[4..])?;

        // Deserialize the chart with rkyv
        let mut chart: RoxChart = rkyv::from_bytes::<RoxChart, RkyvError>(&decompressed)
            .map_err(|e| RoxError::Deserialize(e.to_string()))?;

        // Restore absolute timestamps from deltas
        delta_decode_notes(&mut chart);

        Ok(chart)
    }
}
