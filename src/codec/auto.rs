//! Auto-detection module for format conversion based on file extension.
//!
//! Provides automatic decoding and encoding based on file extensions.

use std::path::Path;

use crate::error::{RoxError, RoxResult};
use crate::model::RoxChart;

#[cfg(feature = "compression")]
use super::formats::RoxCodec;
use super::formats::{
    FnfDecoder, FnfEncoder, JroxDecoder, JroxEncoder, OsuDecoder, OsuEncoder, QuaDecoder,
    QuaEncoder, SmDecoder, SmEncoder, TaikoDecoder, YroxDecoder, YroxEncoder,
};
use super::{Decoder, Encoder};

/// Supported input format extensions for decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    /// Native ROX binary format (`.rox`)
    #[cfg(feature = "compression")]
    Rox,
    /// JSON ROX format (`.jrox`)
    Jrox,
    /// YAML ROX format (`.yrox`)
    Yrox,
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
    #[cfg(feature = "compression")]
    Rox,
    /// JSON ROX format (`.jrox`)
    Jrox,
    /// YAML ROX format (`.yrox`)
    Yrox,
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
        #[cfg(feature = "compression")]
        ("rox", Self::Rox),
        ("jrox", Self::Jrox),
        ("yrox", Self::Yrox),
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
        #[cfg(feature = "compression")]
        ("rox", Self::Rox),
        ("jrox", Self::Jrox),
        ("yrox", Self::Yrox),
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
        #[cfg(feature = "compression")]
        InputFormat::Rox => RoxCodec::decode(&data),
        InputFormat::Jrox => JroxDecoder::decode(&data),
        InputFormat::Yrox => YroxDecoder::decode(&data),
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
        if line == "[Metadata] section" {
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
        #[cfg(feature = "compression")]
        InputFormat::Rox => <RoxCodec as Decoder>::decode(data),
        InputFormat::Jrox => <JroxDecoder as Decoder>::decode(data),
        InputFormat::Yrox => <YroxDecoder as Decoder>::decode(data),
        InputFormat::Osu => <OsuDecoder as Decoder>::decode(data),
        InputFormat::Taiko => <TaikoDecoder as Decoder>::decode(data),
        InputFormat::Sm => <SmDecoder as Decoder>::decode(data),
        InputFormat::Qua => <QuaDecoder as Decoder>::decode(data),
        InputFormat::Fnf => <FnfDecoder as Decoder>::decode(data),
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
        #[cfg(feature = "compression")]
        OutputFormat::Rox => RoxCodec::encode(chart)?,
        OutputFormat::Jrox => JroxEncoder::encode(chart)?,
        OutputFormat::Yrox => YroxEncoder::encode(chart)?,
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
        #[cfg(feature = "compression")]
        OutputFormat::Rox => RoxCodec::encode(chart),
        OutputFormat::Jrox => JroxEncoder::encode(chart),
        OutputFormat::Yrox => YroxEncoder::encode(chart),
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
/// 5. JROX (JSON)
/// 6. YROX (YAML)
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
    match decode_osu_by_mode(bytes) {
        Ok(chart) => return Ok(chart),
        Err(e) => tracing::debug!("Failed to auto-decode as osu: {}", e),
    }

    // Try StepMania
    match SmDecoder::decode(bytes) {
        Ok(chart) => return Ok(chart),
        Err(e) => tracing::debug!("Failed to auto-decode as StepMania: {}", e),
    }

    // Try Quaver
    match QuaDecoder::decode(bytes) {
        Ok(chart) => return Ok(chart),
        Err(e) => tracing::debug!("Failed to auto-decode as Quaver: {}", e),
    }

    // Try FNF
    match FnfDecoder::decode(bytes) {
        Ok(chart) => return Ok(chart),
        Err(e) => tracing::debug!("Failed to auto-decode as FNF: {}", e),
    }

    // Try JROX
    match JroxDecoder::decode(bytes) {
        Ok(chart) => return Ok(chart),
        Err(e) => tracing::debug!("Failed to auto-decode as JROX: {}", e),
    }

    // Try YROX
    match YroxDecoder::decode(bytes) {
        Ok(chart) => return Ok(chart),
        Err(e) => tracing::debug!("Failed to auto-decode as YROX: {}", e),
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
/// 6. JROX (JSON)
/// 7. YROX (YAML)
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
    #[cfg(feature = "compression")]
    match RoxCodec::decode(data) {
        Ok(chart) => return Ok(chart),
        Err(e) => tracing::debug!("Failed to auto-decode as ROX: {}", e),
    }

    // Try osu format (with mode detection)
    match decode_osu_by_mode(data) {
        Ok(chart) => return Ok(chart),
        Err(e) => tracing::debug!("Failed to auto-decode as osu: {}", e),
    }

    // Try StepMania
    match SmDecoder::decode(data) {
        Ok(chart) => return Ok(chart),
        Err(e) => tracing::debug!("Failed to auto-decode as StepMania: {}", e),
    }

    // Try Quaver
    match QuaDecoder::decode(data) {
        Ok(chart) => return Ok(chart),
        Err(e) => tracing::debug!("Failed to auto-decode as Quaver: {}", e),
    }

    // Try FNF
    match FnfDecoder::decode(data) {
        Ok(chart) => return Ok(chart),
        Err(e) => tracing::debug!("Failed to auto-decode as FNF: {}", e),
    }

    // Try JROX
    match JroxDecoder::decode(data) {
        Ok(chart) => return Ok(chart),
        Err(e) => tracing::debug!("Failed to auto-decode as JROX: {}", e),
    }

    // Try YROX
    match YroxDecoder::decode(data) {
        Ok(chart) => return Ok(chart),
        Err(e) => tracing::debug!("Failed to auto-decode as YROX: {}", e),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_input_format_detection() {
        assert_eq!(
            InputFormat::from_extension("osu").unwrap(),
            InputFormat::Osu
        );
        assert_eq!(
            InputFormat::from_extension("OSU").unwrap(),
            InputFormat::Osu
        );
        assert_eq!(InputFormat::from_extension("sm").unwrap(), InputFormat::Sm);
        #[cfg(feature = "compression")]
        assert_eq!(
            InputFormat::from_extension("rox").unwrap(),
            InputFormat::Rox
        );
        assert!(InputFormat::from_extension("mp3").is_err());
    }

    #[test]
    fn test_output_format_detection() {
        assert_eq!(
            OutputFormat::from_extension("osu").unwrap(),
            OutputFormat::Osu
        );
        assert_eq!(
            OutputFormat::from_extension("sm").unwrap(),
            OutputFormat::Sm
        );
        #[cfg(feature = "compression")]
        assert_eq!(
            OutputFormat::from_extension("rox").unwrap(),
            OutputFormat::Rox
        );
        assert!(OutputFormat::from_extension("mp3").is_err());
    }

    #[test]
    fn test_auto_decode_osu_mania() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.osu");
        let data = crate::test_utils::get_test_asset("osu/mania_7k.osu");
        std::fs::write(&path, data).unwrap();

        let chart = auto_decode(&path).unwrap();
        assert_eq!(chart.key_count(), 7);
    }

    #[test]
    fn test_auto_decode_sm() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.sm");
        let data = crate::test_utils::get_test_asset("stepmania/4k.sm");
        std::fs::write(&path, data).unwrap();

        let chart = auto_decode(&path).unwrap();
        assert_eq!(chart.key_count(), 4);
    }

    #[test]
    fn test_auto_encode_osu() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.osu");
        let chart = RoxChart::new(4);

        auto_encode(&chart, &path).unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("osu file format v14"));
    }

    #[test]
    fn test_from_string_detection() {
        let data = crate::test_utils::get_test_asset("osu/mania_7k.osu");
        let s = String::from_utf8(data).unwrap();
        let chart = from_string(&s).unwrap();
        assert_eq!(chart.key_count(), 7);
    }

    #[test]
    fn test_from_bytes_detection() {
        let data = crate::test_utils::get_test_asset("osu/mania_7k.osu");
        let chart = from_bytes(&data).unwrap();
        assert_eq!(chart.key_count(), 7);
    }

    #[test]
    fn test_detect_osu_mode() {
        let mania_data = b"Mode: 3\n[Metadata]";
        assert_eq!(detect_osu_mode(mania_data), 3);

        let taiko_data = b"Mode: 1\n[Metadata]";
        assert_eq!(detect_osu_mode(taiko_data), 1);

        let empty_data = b"";
        assert_eq!(detect_osu_mode(empty_data), 3); // Default
    }

    #[test]
    fn test_auto_convert() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("input.osu");
        let output = dir.path().join("output.sm");

        let data = crate::test_utils::get_test_asset("osu/mania_7k.osu");
        std::fs::write(&input, data).unwrap();

        auto_convert(&input, &output).unwrap();
        assert!(output.exists());
        let content = std::fs::read_to_string(&output).unwrap();
        assert!(content.contains("#TITLE:"));
    }
}
