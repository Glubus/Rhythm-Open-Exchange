#[cfg(not(feature = "std"))]
use alloc::{string::String, string::ToString, vec::Vec};

use crate::error::RoxResult;
use crate::model::RoxChart;

/// Implemented by format structs. Use `#[derive(rox_macros::Format)]` to avoid boilerplate.
pub trait Format {
    const EXTENSIONS: &'static [&'static str];

    #[must_use]
    fn supports_extension(ext: &str) -> bool {
        Self::EXTENSIONS
            .iter()
            .any(|&e| e.eq_ignore_ascii_case(ext))
    }
}

/// Encode a `RoxChart` to bytes. Implement `encode_inner` — do not override `encode`.
pub trait Encoder {
    /// # Errors
    /// Returns an error if encoding fails.
    fn encode_inner(chart: &RoxChart) -> RoxResult<Vec<u8>>;

    /// # Errors
    /// Returns an error if validation or encoding fails.
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        chart.validate()?;
        Self::encode_inner(chart)
    }

    #[cfg(feature = "std")]
    /// # Errors
    /// Returns an error if encoding or file writing fails.
    fn encode_to_path(chart: &RoxChart, path: impl AsRef<std::path::Path>) -> RoxResult<()> {
        let data = Self::encode(chart)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// # Errors
    /// Returns an error if encoding fails or the output is not valid UTF-8.
    fn encode_to_string(chart: &RoxChart) -> RoxResult<String> {
        let data = Self::encode(chart)?;
        String::from_utf8(data).map_err(|e| crate::error::RoxError::InvalidFormat(e.to_string()))
    }
}

/// Decode bytes to a `RoxChart`. Implement `decode_inner` — do not override `decode`.
pub trait Decoder {
    /// # Errors
    /// Returns an error if decoding fails.
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart>;

    /// # Errors
    /// Returns an error if decoding or validation fails.
    fn decode(data: &[u8]) -> RoxResult<RoxChart> {
        let chart = Self::decode_inner(data)?;
        chart.validate()?;
        Ok(chart)
    }

    #[cfg(feature = "std")]
    /// # Errors
    /// Returns an error if file reading or decoding fails.
    fn decode_from_path(path: impl AsRef<std::path::Path>) -> RoxResult<RoxChart> {
        let data = std::fs::read(path)?;
        Self::decode(&data)
    }
}
