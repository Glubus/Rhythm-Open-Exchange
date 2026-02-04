//! Parser for Friday Night Funkin' .json chart files.

use super::types::FnfChart;
use crate::error::{RoxError, RoxResult};

// Safety limit: 100MB
const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Parse a FNF .json file into an `FnfChart`.
///
/// # Why this design?
/// We use `serde_json` for robust and compliant JSON parsing.
/// While slightly heavier than a custom regex parser, JSON structure in FNF can vary slightly,
/// and `serde` handles these edge cases (whitespace, encoding) reliably.
/// The `MAX_FILE_SIZE` check prevents memory exhaustion from malicious inputs.
///
/// # Errors
///
/// Returns an error if:
/// - The data is not valid UTF-8
/// - The JSON is malformed
/// - The file is larger than 100MB
pub fn parse(data: &[u8]) -> RoxResult<FnfChart> {
    if data.len() > MAX_FILE_SIZE {
        return Err(RoxError::InvalidFormat(format!(
            "File too large: {} bytes (max {}MB)",
            data.len(),
            MAX_FILE_SIZE / 1024 / 1024
        )));
    }

    let content = std::str::from_utf8(data)
        .map_err(|e| RoxError::InvalidFormat(format!("Invalid UTF-8: {e}")))?;

    serde_json::from_str(content).map_err(|e| RoxError::InvalidFormat(format!("Invalid JSON: {e}")))
}
