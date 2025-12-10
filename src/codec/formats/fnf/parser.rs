//! Parser for Friday Night Funkin' .json chart files.

use super::types::FnfChart;
use crate::error::{RoxError, RoxResult};

/// Parse a FNF .json file into an `FnfChart`.
///
/// # Errors
///
/// Returns an error if:
/// - The data is not valid UTF-8
/// - The JSON is malformed
pub fn parse(data: &[u8]) -> RoxResult<FnfChart> {
    let content = std::str::from_utf8(data)
        .map_err(|e| RoxError::InvalidFormat(format!("Invalid UTF-8: {e}")))?;

    serde_json::from_str(content).map_err(|e| RoxError::InvalidFormat(format!("Invalid JSON: {e}")))
}
