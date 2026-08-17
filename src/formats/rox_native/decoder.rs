#![warn(clippy::pedantic)]

use rkyv::rancor::Error as RkyvError;
use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{ROX_MAGIC, RoxChart};

use super::MAX_FILE_SIZE;

/// Native ROX binary codec: rkyv serialization + zstd compression + delta timestamps.
pub struct RoxNativeCodec;
crate::impl_format!(RoxNativeCodec, ["rox"]);

impl Decoder for RoxNativeCodec {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        if data.len() < 4 || data[..4] != ROX_MAGIC {
            return Err(RoxError::InvalidFormat(
                "Invalid ROX file: missing magic bytes".into(),
            ));
        }
        if data.len() > MAX_FILE_SIZE {
            return Err(RoxError::InvalidFormat(format!(
                "File too large: {} bytes (max 100MB)",
                data.len()
            )));
        }
        let decompressed = decompress(&data[4..])?;
        // SAFETY: data was produced by RoxNativeCodec::encode_inner using rkyv::to_bytes
        let mut chart = unsafe { rkyv::from_bytes_unchecked::<RoxChart, RkyvError>(&decompressed) }
            .map_err(|e| RoxError::Deserialize(e.to_string()))?;
        delta_decode_notes(&mut chart);
        Ok(chart)
    }
}

fn delta_decode_notes(chart: &mut RoxChart) {
    let mut acc: i64 = 0;
    for note in &mut chart.notes {
        acc += note.time_us;
        note.time_us = acc;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn decompress(data: &[u8]) -> RoxResult<Vec<u8>> {
    use std::io::Read as _;
    let mut dec = zstd::stream::Decoder::new(data)?;
    let mut out = Vec::new();
    dec.read_to_end(&mut out)?;
    Ok(out)
}

#[cfg(target_arch = "wasm32")]
fn decompress(data: &[u8]) -> RoxResult<Vec<u8>> {
    Ok(data.to_vec())
}

#[cfg(test)]
mod tests {
    use rox::codec::{Decoder, Encoder};
    use rox::model::{Note, TimingPoint};
    use rstest::rstest;

    use super::*;

    #[rstest]
    fn test_roundtrip() {
        let mut chart = rox::model::RoxChart::new(4);
        chart.timing_points.push(TimingPoint::bpm(0, 180.0));
        chart.notes.push(Note::tap(1_000_000, 0));
        chart.notes.push(Note::hold(2_000_000, 500_000, 1));

        let encoded = RoxNativeCodec::encode(&chart).expect("encode failed");
        assert_eq!(&encoded[..4], &ROX_MAGIC);

        let decoded = RoxNativeCodec::decode(&encoded).expect("decode failed");
        assert_eq!(chart.key_count, decoded.key_count);
        assert_eq!(chart.notes.len(), decoded.notes.len());
        assert_eq!(chart.notes[0].time_us, decoded.notes[0].time_us);
        assert_eq!(chart.notes[1].time_us, decoded.notes[1].time_us);
    }

    #[rstest]
    fn test_rejects_invalid_magic() {
        assert!(RoxNativeCodec::decode(b"BAD!compressed").is_err());
    }
}
