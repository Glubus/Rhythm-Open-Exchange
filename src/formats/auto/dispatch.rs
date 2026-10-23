#![warn(clippy::pedantic)]

use std::path::Path;

use rox::codec::{Decoder, Encoder};
use rox::error::{RoxError, RoxResult};
use rox::model::RoxChart;

use super::detect::{DetectedFormat, detect};
use crate::formats::{
    FnfDecoder, FnfEncoder, JroxDecoder, JroxEncoder, McDecoder, McEncoder, OsuDecoder, OsuEncoder,
    QuaDecoder, QuaEncoder, RoxNativeCodec, SmDecoder, SmEncoder, SscDecoder, SscEncoder,
    YroxDecoder, YroxEncoder,
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

/// Decode chart `index` of a file: the `index`-th playable chart of a `.sm`/`.ssc` file (the
/// numbering `sm::parse_file` gives), the only chart of any other format.
///
/// # Errors
///
/// Returns an error if the file does not decode or has no such chart.
pub fn auto_decode_variant(path: impl AsRef<Path>, index: usize) -> RoxResult<RoxChart> {
    let path = path.as_ref();
    if !crate::formats::sm::is_stepmania(path) {
        return if index == 0 {
            auto_decode(path)
        } else {
            Err(RoxError::InvalidFormat(format!(
                "{} has a single chart, not chart {index}",
                path.display()
            )))
        };
    }
    let file = crate::formats::sm::parse_file(path)?;
    match file.charts.get(index) {
        Some(chart) => Ok(crate::formats::sm::decoder::from_chart(&file, chart)),
        None if index == 0 => crate::formats::sm::ssc_decoder::first_chart(&file),
        None => Err(RoxError::InvalidFormat(format!(
            "{} has {} playable charts, not chart {index}",
            path.display(),
            file.charts.len()
        ))),
    }
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
        DetectedFormat::Osu => OsuDecoder::decode(data),
        DetectedFormat::Sm => SmDecoder::decode(data),
        DetectedFormat::Ssc => SscDecoder::decode(data),
        DetectedFormat::Qua => QuaDecoder::decode(data),
        DetectedFormat::Fnf => FnfDecoder::decode(data),
        DetectedFormat::Jrox => JroxDecoder::decode(data),
        DetectedFormat::Yrox => YroxDecoder::decode(data),
        DetectedFormat::Rox => RoxNativeCodec::decode(data),
        DetectedFormat::Mc => McDecoder::decode(data),
    }
}

fn encode_bytes(fmt: DetectedFormat, chart: &RoxChart) -> RoxResult<Vec<u8>> {
    match fmt {
        DetectedFormat::Osu => OsuEncoder::encode(chart),
        DetectedFormat::Sm => SmEncoder::encode(chart),
        DetectedFormat::Ssc => SscEncoder::encode(chart),
        DetectedFormat::Qua => QuaEncoder::encode(chart),
        DetectedFormat::Fnf => FnfEncoder::encode(chart),
        DetectedFormat::Jrox => JroxEncoder::encode(chart),
        DetectedFormat::Yrox => YroxEncoder::encode(chart),
        DetectedFormat::Rox => RoxNativeCodec::encode(chart),
        DetectedFormat::Mc => McEncoder::encode(chart),
    }
}
