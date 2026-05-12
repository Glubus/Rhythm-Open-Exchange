#![warn(clippy::pedantic)]

use std::path::Path;

use rox::codec::{Decoder, Encoder};
use rox::error::RoxResult;
use rox::model::RoxChart;

use super::detect::{DetectedFormat, detect};
use crate::{
    FnfDecoder, FnfEncoder, JroxDecoder, JroxEncoder, OsuDecoder, OsuEncoder, QuaDecoder,
    QuaEncoder, McDecoder, McEncoder, RoxNativeCodec, SmDecoder, SmEncoder, YroxDecoder, YroxEncoder,
};

/// Decode a file to a `RoxChart`, detecting the format from the file extension.
///
/// # Errors
///
/// Returns `UnsupportedFormat` if the extension is unknown, or a decode error on failure.
pub fn auto_decode(path: impl AsRef<Path>) -> RoxResult<RoxChart> {
    let path = path.as_ref();
    let fmt = detect(path)?;
    let data = std::fs::read(path)?;
    decode_bytes(fmt, &data)
}

/// Decode in-memory bytes to a `RoxChart`, detecting the format from a path hint
/// (the bytes are not read from disk; only the extension of `path_hint` is used).
///
/// # Errors
///
/// Returns `UnsupportedFormat` if the extension is unknown, or a decode error on failure.
pub fn auto_decode_bytes(path_hint: impl AsRef<Path>, bytes: &[u8]) -> RoxResult<RoxChart> {
    let fmt = detect(path_hint.as_ref())?;
    decode_bytes(fmt, bytes)
}

/// Encode a `RoxChart` to a file, detecting the format from the file extension.
///
/// # Errors
///
/// Returns `UnsupportedFormat` if the extension is unknown, or an encode error on failure.
pub fn auto_encode(chart: &RoxChart, path: impl AsRef<Path>) -> RoxResult<()> {
    let path = path.as_ref();
    let fmt = detect(path)?;
    let data = encode_bytes(fmt, chart)?;
    std::fs::write(path, data)?;
    Ok(())
}

/// Convert a file from one format to another, detecting both from file extensions.
///
/// # Errors
///
/// Returns `UnsupportedFormat` if either extension is unknown, or a codec error on failure.
pub fn auto_convert(input: impl AsRef<Path>, output: impl AsRef<Path>) -> RoxResult<()> {
    let chart = auto_decode(input)?;
    auto_encode(&chart, output)
}

fn decode_bytes(fmt: DetectedFormat, data: &[u8]) -> RoxResult<RoxChart> {
    match fmt {
        DetectedFormat::Osu  => OsuDecoder::decode(data),
        DetectedFormat::Sm   => SmDecoder::decode(data),
        DetectedFormat::Qua  => QuaDecoder::decode(data),
        DetectedFormat::Fnf  => FnfDecoder::decode(data),
        DetectedFormat::Jrox => JroxDecoder::decode(data),
        DetectedFormat::Yrox => YroxDecoder::decode(data),
        DetectedFormat::Rox  => RoxNativeCodec::decode(data),
        DetectedFormat::Mc   => McDecoder::decode(data),
    }
}

fn encode_bytes(fmt: DetectedFormat, chart: &RoxChart) -> RoxResult<Vec<u8>> {
    match fmt {
        DetectedFormat::Osu  => OsuEncoder::encode(chart),
        DetectedFormat::Sm   => SmEncoder::encode(chart),
        DetectedFormat::Qua  => QuaEncoder::encode(chart),
        DetectedFormat::Fnf  => FnfEncoder::encode(chart),
        DetectedFormat::Jrox => JroxEncoder::encode(chart),
        DetectedFormat::Yrox => YroxEncoder::encode(chart),
        DetectedFormat::Rox  => RoxNativeCodec::encode(chart),
        DetectedFormat::Mc   => McEncoder::encode(chart),
    }
}
