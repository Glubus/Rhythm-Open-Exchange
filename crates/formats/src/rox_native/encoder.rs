#![warn(clippy::pedantic)]

use rox::codec::Encoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{ROX_MAGIC, RoxChart};
use rkyv::rancor::Error as RkyvError;

use super::RoxNativeCodec;

/// zstd compression level: 3 = fast with good ratio.
#[cfg(not(target_arch = "wasm32"))]
const COMPRESSION_LEVEL: i32 = 3;

impl Encoder for RoxNativeCodec {
    fn encode_inner(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        let delta = delta_encode_notes(chart);
        let serialized = rkyv::to_bytes::<RkyvError>(&delta)
            .map_err(|e| RoxError::Serialize(e.to_string()))?;
        let compressed = compress(&serialized)?;
        let mut out = ROX_MAGIC.to_vec();
        out.extend(compressed);
        Ok(out)
    }
}

/// Store delta timestamps instead of absolute — improves zstd compression ratio.
fn delta_encode_notes(chart: &RoxChart) -> RoxChart {
    let mut result = chart.clone();
    let mut last_time: i64 = 0;
    for note in &mut result.notes {
        let abs = note.time_us;
        note.time_us = abs - last_time;
        last_time = abs;
    }
    result
}

#[cfg(not(target_arch = "wasm32"))]
fn compress(data: &[u8]) -> RoxResult<Vec<u8>> {
    use std::io::Write as _;
    let mut enc = zstd::stream::Encoder::new(Vec::new(), COMPRESSION_LEVEL)?;
    enc.write_all(data)?;
    enc.finish().map_err(RoxError::Io)
}

#[cfg(target_arch = "wasm32")]
fn compress(data: &[u8]) -> RoxResult<Vec<u8>> {
    Ok(data.to_vec())
}
