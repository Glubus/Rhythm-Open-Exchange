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
/// For `.osu` files, the mode is detected from the `Mode:` field in `[General]`:
/// - Mode 1 (Taiko) → Uses `TaikoDecoder`
/// - Mode 3 (Mania) → Uses `OsuDecoder`
/// - Other modes are not supported
///
/// # Example
/// ```ignore
/// use rox::codec::auto_decode;
///
/// let chart = auto_decode("chart.osu")?;  // Detects .osu format and mode
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
        InputFormat::Osu | InputFormat::Taiko => decode_osu_by_mode(&data),
        InputFormat::Sm => SmDecoder::decode(&data),
        InputFormat::Qua => QuaDecoder::decode(&data),
        InputFormat::Fnf => FnfDecoder::decode(&data),
    }
}

/// Decode an osu! file by detecting its mode and using the appropriate decoder.
fn decode_osu_by_mode(data: &[u8]) -> RoxResult<RoxChart> {
    match detect_osu_mode(data) {
        1 => TaikoDecoder::decode(data),
        3 => OsuDecoder::decode(data),
        mode => Err(RoxError::UnsupportedFormat(format!(
            "osu! mode {mode} is not supported (only taiko=1 and mania=3)"
        ))),
    }
}

/// Detect the osu! game mode from file content.
/// Returns the mode number: 0=std, 1=taiko, 2=catch, 3=mania.
/// Defaults to 3 (mania) if not found.
fn detect_osu_mode(data: &[u8]) -> u8 {
    let Ok(content) = std::str::from_utf8(data) else {
        return 3; // Default to mania on invalid UTF-8
    };

    for line in content.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("Mode:")
            && let Ok(mode) = value.trim().parse::<u8>()
        {
            return mode;
        }
        // Stop parsing after [Metadata] section to avoid scanning entire file
        if line == "[Metadata]" {
            break;
        }
    }

    3 // Default to mania if Mode not found
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

/// Decode a chart from a string, auto-detecting the format.
///
/// Attempts to decode the chart data from the provided string using all available decoders
/// until one succeeds. Tries formats in the following order:
/// 1. Osu (with mode detection)
/// 2. `StepMania`
/// 3. Quaver (YAML)
/// 4. Friday Night Funkin' (JSON)
///
/// # Example
/// ```ignore
/// use rox::codec::from_string;
///
/// let osu_content = std::fs::read_to_string("chart.osu")?;
/// let chart = from_string(&osu_content)?;
/// ```
///
/// # Errors
///
/// Returns an error if all decoders fail to parse the input.
pub fn from_string(data: &str) -> RoxResult<RoxChart> {
    let bytes = data.as_bytes();

    // Try osu format (with mode detection)
    if let Ok(chart) = decode_osu_by_mode(bytes) {
        return Ok(chart);
    }

    // Try StepMania
    if let Ok(chart) = SmDecoder::decode(bytes) {
        return Ok(chart);
    }

    // Try Quaver
    if let Ok(chart) = QuaDecoder::decode(bytes) {
        return Ok(chart);
    }

    // Try FNF
    if let Ok(chart) = FnfDecoder::decode(bytes) {
        return Ok(chart);
    }

    Err(RoxError::InvalidFormat(
        "Failed to decode chart: no format decoder succeeded".into(),
    ))
}

/// Decode a chart from bytes, auto-detecting the format.
///
/// Attempts to decode the chart data from the provided bytes using all available decoders
/// until one succeeds. Tries formats in the following order:
/// 1. ROX binary format
/// 2. Osu (with mode detection)
/// 3. `StepMania`
/// 4. Quaver (YAML)
/// 5. Friday Night Funkin' (JSON)
///
/// # Example
/// ```ignore
/// use rox::codec::from_bytes;
///
/// let data = std::fs::read("chart.osu")?;
/// let chart = from_bytes(&data)?;
/// ```
///
/// # Errors
///
/// Returns an error if all decoders fail to parse the input.
pub fn from_bytes(data: &[u8]) -> RoxResult<RoxChart> {
    // Try ROX binary format first
    if let Ok(chart) = RoxCodec::decode(data) {
        return Ok(chart);
    }

    // Try osu format (with mode detection)
    if let Ok(chart) = decode_osu_by_mode(data) {
        return Ok(chart);
    }

    // Try StepMania
    if let Ok(chart) = SmDecoder::decode(data) {
        return Ok(chart);
    }

    // Try Quaver
    if let Ok(chart) = QuaDecoder::decode(data) {
        return Ok(chart);
    }

    // Try FNF
    if let Ok(chart) = FnfDecoder::decode(data) {
        return Ok(chart);
    }

    Err(RoxError::InvalidFormat(
        "Failed to decode chart: no format decoder succeeded".into(),
    ))
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
