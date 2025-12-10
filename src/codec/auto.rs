//! Auto-detection module for format conversion based on file extension.
//!
//! Provides automatic decoding and encoding based on file extensions.

use std::path::Path;

use crate::error::{RoxError, RoxResult};
use crate::model::RoxChart;

use super::formats::{
    FnfDecoder, FnfEncoder, OsuDecoder, OsuEncoder, QuaDecoder, QuaEncoder, SmDecoder, SmEncoder,
    TaikoDecoder,
};
use super::rox::RoxCodec;
use super::{Decoder, Encoder};

/// Supported input format extensions for decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    /// Native ROX binary format (`.rox`)
    Rox,
    /// osu!mania format (`.osu`)
    Osu,
    /// osu!taiko format (`.osu` with mode detection)
    Taiko,
    /// `StepMania` format (`.sm`)
    Sm,
    /// Quaver format (`.qua`)
    Qua,
    /// Friday Night Funkin' format (`.json`)
    Fnf,
}

/// Supported output format extensions for encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Native ROX binary format (`.rox`)
    Rox,
    /// osu!mania format (`.osu`)
    Osu,
    /// `StepMania` format (`.sm`)
    Sm,
    /// Quaver format (`.qua`)
    Qua,
    /// Friday Night Funkin' format (`.json`)
    Fnf,
}

impl InputFormat {
    /// All supported input extensions.
    pub const EXTENSIONS: &'static [(&'static str, Self)] = &[
        ("rox", Self::Rox),
        ("osu", Self::Osu),
        ("sm", Self::Sm),
        ("qua", Self::Qua),
        ("json", Self::Fnf),
    ];

    /// Detect format from file extension.
    ///
    /// # Errors
    ///
    /// Returns an error if the extension is not recognized.
    pub fn from_extension(ext: &str) -> RoxResult<Self> {
        let ext_lower = ext.to_lowercase();
        for (e, format) in Self::EXTENSIONS {
            if *e == ext_lower {
                return Ok(*format);
            }
        }
        Err(RoxError::UnsupportedFormat(format!(
            "Unknown input extension: .{ext}"
        )))
    }

    /// Detect format from file path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path has no extension or it's not recognized.
    pub fn from_path(path: impl AsRef<Path>) -> RoxResult<Self> {
        let path = path.as_ref();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| RoxError::InvalidFormat("No file extension".into()))?;
        Self::from_extension(ext)
    }
}

impl OutputFormat {
    /// All supported output extensions.
    pub const EXTENSIONS: &'static [(&'static str, Self)] = &[
        ("rox", Self::Rox),
        ("osu", Self::Osu),
        ("sm", Self::Sm),
        ("qua", Self::Qua),
        ("json", Self::Fnf),
    ];

    /// Detect format from file extension.
    ///
    /// # Errors
    ///
    /// Returns an error if the extension is not recognized.
    pub fn from_extension(ext: &str) -> RoxResult<Self> {
        let ext_lower = ext.to_lowercase();
        for (e, format) in Self::EXTENSIONS {
            if *e == ext_lower {
                return Ok(*format);
            }
        }
        Err(RoxError::UnsupportedFormat(format!(
            "Unknown output extension: .{ext}"
        )))
    }

    /// Detect format from file path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path has no extension or it's not recognized.
    pub fn from_path(path: impl AsRef<Path>) -> RoxResult<Self> {
        let path = path.as_ref();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| RoxError::InvalidFormat("No file extension".into()))?;
        Self::from_extension(ext)
    }
}

/// Decode a chart from a file, auto-detecting the format from the extension.
///
/// # Example
/// ```ignore
/// use rox::codec::auto_decode;
///
/// let chart = auto_decode("chart.osu")?;  // Detects .osu format
/// let chart = auto_decode("chart.sm")?;   // Detects .sm format
/// let chart = auto_decode("chart.rox")?;  // Detects .rox format
/// ```
///
/// # Errors
///
/// Returns an error if decoding fails or the extension is not recognized.
pub fn auto_decode(path: impl AsRef<Path>) -> RoxResult<RoxChart> {
    let path = path.as_ref();
    let format = InputFormat::from_path(path)?;
    let data = std::fs::read(path)?;

    match format {
        InputFormat::Rox => RoxCodec::decode(&data),
        InputFormat::Osu => OsuDecoder::decode(&data),
        InputFormat::Taiko => TaikoDecoder::decode(&data),
        InputFormat::Sm => SmDecoder::decode(&data),
        InputFormat::Qua => QuaDecoder::decode(&data),
        InputFormat::Fnf => FnfDecoder::decode(&data),
    }
}

/// Decode chart data with a specific format.
///
/// # Errors
///
/// Returns an error if decoding fails.
pub fn decode_with_format(data: &[u8], format: InputFormat) -> RoxResult<RoxChart> {
    match format {
        InputFormat::Rox => RoxCodec::decode(data),
        InputFormat::Osu => OsuDecoder::decode(data),
        InputFormat::Taiko => TaikoDecoder::decode(data),
        InputFormat::Sm => SmDecoder::decode(data),
        InputFormat::Qua => QuaDecoder::decode(data),
        InputFormat::Fnf => FnfDecoder::decode(data),
    }
}

/// Encode a chart to a file, auto-detecting the format from the extension.
///
/// # Example
/// ```ignore
/// use rox::codec::auto_encode;
///
/// auto_encode(&chart, "output.osu")?;  // Encodes as .osu
/// auto_encode(&chart, "output.sm")?;   // Encodes as .sm
/// auto_encode(&chart, "output.rox")?;  // Encodes as .rox
/// ```
///
/// # Errors
///
/// Returns an error if encoding fails or the extension is not recognized.
pub fn auto_encode(chart: &RoxChart, path: impl AsRef<Path>) -> RoxResult<()> {
    let path = path.as_ref();
    let format = OutputFormat::from_path(path)?;

    let data = match format {
        OutputFormat::Rox => RoxCodec::encode(chart)?,
        OutputFormat::Osu => OsuEncoder::encode(chart)?,
        OutputFormat::Sm => SmEncoder::encode(chart)?,
        OutputFormat::Qua => QuaEncoder::encode(chart)?,
        OutputFormat::Fnf => FnfEncoder::encode(chart)?,
    };

    std::fs::write(path, data)?;
    Ok(())
}

/// Encode a chart to bytes with a specific format.
///
/// # Errors
///
/// Returns an error if encoding fails.
pub fn encode_with_format(chart: &RoxChart, format: OutputFormat) -> RoxResult<Vec<u8>> {
    match format {
        OutputFormat::Rox => RoxCodec::encode(chart),
        OutputFormat::Osu => OsuEncoder::encode(chart),
        OutputFormat::Sm => SmEncoder::encode(chart),
        OutputFormat::Qua => QuaEncoder::encode(chart),
        OutputFormat::Fnf => FnfEncoder::encode(chart),
    }
}

/// Convert a file from one format to another, auto-detecting both formats.
///
/// # Example
/// ```ignore
/// use rox::codec::auto_convert;
///
/// auto_convert("chart.osu", "chart.sm")?;   // osu → sm
/// auto_convert("chart.sm", "chart.rox")?;   // sm → rox
/// auto_convert("chart.rox", "chart.osu")?;  // rox → osu
/// ```
///
/// # Errors
///
/// Returns an error if conversion fails or extensions are not recognized.
pub fn auto_convert(input: impl AsRef<Path>, output: impl AsRef<Path>) -> RoxResult<()> {
    let chart = auto_decode(input)?;
    auto_encode(&chart, output)
}
