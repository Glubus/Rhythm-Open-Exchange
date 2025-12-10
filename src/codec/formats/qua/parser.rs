//! Parser for Quaver .qua files using serde_yaml.

use super::types::QuaChart;
use crate::error::{RoxError, RoxResult};

/// Parse a .qua file into a `QuaChart`.
///
/// # Errors
///
/// Returns an error if:
/// - The data is not valid UTF-8
/// - The YAML is malformed
pub fn parse(data: &[u8]) -> RoxResult<QuaChart> {
    let content = std::str::from_utf8(data)
        .map_err(|e| RoxError::InvalidFormat(format!("Invalid UTF-8: {e}")))?;

    serde_yaml::from_str(content).map_err(|e| RoxError::InvalidFormat(format!("Invalid YAML: {e}")))
}
