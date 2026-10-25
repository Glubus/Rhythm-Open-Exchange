#![warn(clippy::pedantic)]

use rkyv::rancor::Error as RkyvError;
use rkyv::Archived;
use rkyv::util::AlignedVec;
use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{ROX_MAGIC, RoxChart};

use super::MAX_FILE_SIZE;

/// Largest archive a `.rox` frame may inflate to (64 MiB). Past it the file is refused with
/// [`RoxError::InvalidFormat`] before any more memory is spent, so a zstd bomb cannot exhaust RAM.
/// The largest real chart seen is a few MB archived.
pub const MAX_DECOMPRESSED: usize = 64 * 1024 * 1024;
/// Most notes a `.rox` file may declare (checked on the validated archive, before it is copied out).
pub const MAX_NOTES: usize = 4_000_000;
/// Most timing points a `.rox` file may declare.
pub const MAX_TIMING_POINTS: usize = 1_000_000;
/// Most hitsounds a `.rox` file may declare.
pub const MAX_HITSOUNDS: usize = 1_000_000;

/// Native ROX binary codec: rkyv serialization + zstd compression + delta timestamps.
///
/// Decoding is always checked: the zstd frame is inflated into a bounded, 16-byte-aligned buffer
/// ([`MAX_DECOMPRESSED`]) and the rkyv archive is validated with `bytecheck` (pointers, lengths,
/// enum tags, UTF-8) before anything is read, so bytes from the network or any other untrusted
/// source cannot cause undefined behaviour. There is no unchecked path.
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
        let archived = rkyv::access::<Archived<RoxChart>, RkyvError>(&decompressed)
            .map_err(|e| RoxError::Deserialize(e.to_string()))?;
        check_counts(archived)?;
        let mut chart = rkyv::deserialize::<RoxChart, RkyvError>(archived)
            .map_err(|e| RoxError::Deserialize(e.to_string()))?;
        delta_decode_notes(&mut chart);
        Ok(chart)
    }
}

fn check_counts(archived: &Archived<RoxChart>) -> RoxResult<()> {
    let too_many = |what: &str, n: usize, max: usize| {
        Err(RoxError::InvalidFormat(format!(
            "Invalid ROX file: {n} {what} (max {max})"
        )))
    };
    if archived.notes.len() > MAX_NOTES {
        return too_many("notes", archived.notes.len(), MAX_NOTES);
    }
    if archived.timing_points.len() > MAX_TIMING_POINTS {
        return too_many(
            "timing points",
            archived.timing_points.len(),
            MAX_TIMING_POINTS,
        );
    }
    if archived.hitsounds.len() > MAX_HITSOUNDS {
        return too_many("hitsounds", archived.hitsounds.len(), MAX_HITSOUNDS);
    }
    Ok(())
}

fn delta_decode_notes(chart: &mut RoxChart) {
    let mut acc: i64 = 0;
    for note in &mut chart.notes {
        acc = acc.wrapping_add(note.time_us);
        note.time_us = acc;
    }
}

/// Inflate the zstd frame into an aligned buffer, refusing more than [`MAX_DECOMPRESSED`] bytes.
#[cfg(not(target_arch = "wasm32"))]
fn decompress(data: &[u8]) -> RoxResult<AlignedVec> {
    use std::io::Read as _;
    let mut dec = zstd::stream::Decoder::new(data)?;
    let mut out = AlignedVec::new();
    let mut chunk = [0_u8; 16 * 1024];
    loop {
        let n = dec.read(&mut chunk)?;
        if n == 0 {
            return Ok(out);
        }
        if out.len() + n > MAX_DECOMPRESSED {
            return Err(RoxError::InvalidFormat(format!(
                "Invalid ROX file: decompressed size exceeds {MAX_DECOMPRESSED} bytes"
            )));
        }
        out.extend_from_slice(&chunk[..n]);
    }
}

#[cfg(target_arch = "wasm32")]
fn decompress(data: &[u8]) -> RoxResult<AlignedVec> {
    if data.len() > MAX_DECOMPRESSED {
        return Err(RoxError::InvalidFormat(format!(
            "Invalid ROX file: size exceeds {MAX_DECOMPRESSED} bytes"
        )));
    }
    let mut out = AlignedVec::new();
    out.extend_from_slice(data);
    Ok(out)
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

    /// A chart with enough content to give the archive strings, tags and many notes.
    fn fixture() -> Vec<u8> {
        let mut chart = rox::model::RoxChart::new(4);
        chart.metadata.title = "Title \u{3042}".into();
        chart.metadata.tags = vec![
            "a".into(),
            "bb".into(),
            "a long tag that is not inline at all".into(),
        ];
        chart.timing_points.push(TimingPoint::bpm(0, 180.0));
        for i in 0..2_000_i64 {
            let col = u8::try_from(i % 4).unwrap();
            chart.notes.push(Note::tap(1_000_000 + i * 100_000, col));
        }
        RoxNativeCodec::encode(&chart).expect("encode failed")
    }

    fn compress_raw(raw: &[u8]) -> Vec<u8> {
        let mut out = ROX_MAGIC.to_vec();
        out.extend(zstd::bulk::compress(raw, 3).unwrap());
        out
    }

    fn inflate(file: &[u8]) -> Vec<u8> {
        zstd::bulk::decompress(&file[4..], MAX_DECOMPRESSED).unwrap()
    }

    #[test]
    fn test_fixture_decodes() {
        assert_eq!(
            RoxNativeCodec::decode(&fixture()).unwrap().notes.len(),
            2_000
        );
    }

    #[test]
    fn test_rejects_truncated() {
        let file = fixture();
        for cut in [4, 5, 10, file.len() / 2, file.len() - 1] {
            assert!(RoxNativeCodec::decode(&file[..cut]).is_err(), "cut {cut}");
        }
        // Truncated archive inside a valid zstd frame.
        let raw = inflate(&file);
        for cut in [0, 8, raw.len() / 2, raw.len() - 1] {
            assert!(
                RoxNativeCodec::decode(&compress_raw(&raw[..cut])).is_err(),
                "raw cut {cut}"
            );
        }
    }

    #[test]
    fn test_rejects_lying_lengths() {
        // The root object sits at the end of an rkyv archive: overwrite every 4-byte aligned
        // word of its tail so lengths and relative pointers lie.
        let raw = inflate(&fixture());
        let tail = raw.len().saturating_sub(64);
        let mut refused = 0;
        let mut tried = 0;
        for at in (tail..raw.len() - 3).step_by(4) {
            for value in [u32::MAX, 0x7FFF_FFF0, 0x0100_0000] {
                let mut bad = raw.clone();
                bad[at..at + 4].copy_from_slice(&value.to_le_bytes());
                tried += 1;
                if RoxNativeCodec::decode(&compress_raw(&bad)).is_err() {
                    refused += 1;
                }
            }
        }
        // Words that are padding or free data may stay valid, but lying lengths must be caught.
        assert!(refused * 2 > tried, "refused {refused} of {tried}");
    }

    #[test]
    fn test_rejects_zstd_bomb() {
        // Twice the limit of zeros compresses to a few KB.
        let mut out = ROX_MAGIC.to_vec();
        let mut enc = zstd::stream::Encoder::new(Vec::new(), 19).unwrap();
        let zeros = vec![0_u8; 1024 * 1024];
        for _ in 0..(2 * MAX_DECOMPRESSED / zeros.len()) {
            std::io::Write::write_all(&mut enc, &zeros).unwrap();
        }
        out.extend(enc.finish().unwrap());
        assert!(out.len() < 1024 * 1024);
        let err = RoxNativeCodec::decode(&out).unwrap_err();
        assert!(matches!(err, RoxError::InvalidFormat(m) if m.contains("exceeds")));
    }

    #[test]
    fn test_random_mutations_never_panic() {
        let file = fixture();
        let raw = inflate(&file);
        // xorshift: deterministic, no dependency.
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut refused = 0;
        for _ in 0..300 {
            // Flip bits of the compressed file (zstd has no checksum here, so some stay valid).
            let mut bad = file.clone();
            for _ in 0..=(next() % 3) {
                let at = 4 + usize::try_from(next()).unwrap() % (bad.len() - 4);
                bad[at] ^= 1 << (next() % 8);
            }
            let _ = RoxNativeCodec::decode(&bad);
            // Flip bits of the archive itself (valid zstd, damaged rkyv).
            let mut bad = raw.clone();
            for _ in 0..=(next() % 4) {
                let at = usize::try_from(next()).unwrap() % bad.len();
                bad[at] ^= 1 << (next() % 8);
            }
            if RoxNativeCodec::decode(&compress_raw(&bad)).is_err() {
                refused += 1;
            }
        }
        assert!(refused > 0);
    }

    #[rstest]
    fn test_rejects_invalid_magic() {
        assert!(RoxNativeCodec::decode(b"BAD!compressed").is_err());
    }
}
