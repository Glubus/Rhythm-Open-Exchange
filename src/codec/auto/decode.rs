use std::path::Path;

use crate::error::{RoxError, RoxResult};
use crate::model::RoxChart;

use super::super::Decoder;
#[cfg(feature = "compression")]
use super::super::formats::RoxCodec;
use super::super::formats::{
    FnfDecoder, JroxDecoder, OsuDecoder, QuaDecoder, SmDecoder, TaikoDecoder, YroxDecoder,
};
use super::types::InputFormat;

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
pub(crate) fn detect_osu_mode(data: &[u8]) -> u8 {
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
