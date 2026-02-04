//! Parser for Quaver .qua files using `serde_yaml`.

use super::types::QuaChart;
use crate::error::{RoxError, RoxResult};

// Safety limit: 100MB
const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Parse a .qua file into a `QuaChart`.
///
/// # Why this design?
/// We use `serde_yaml` because .qua files are valid YAML.
/// Writing a custom YAML parser is error-prone and unnecessary.
/// A strict size limit is enforced to prevent "YAML bomb" attacks or memory exhaustion.
///
/// # Errors
///
/// Returns an error if:
/// - The data is not valid UTF-8
/// - The YAML is malformed
/// - The file is larger than 100MB
pub fn parse(data: &[u8]) -> RoxResult<QuaChart> {
    if data.len() > MAX_FILE_SIZE {
        return Err(RoxError::InvalidFormat(format!(
            "File too large: {} bytes (max {}MB)",
            data.len(),
            MAX_FILE_SIZE / 1024 / 1024
        )));
    }

    let content = std::str::from_utf8(data)
        .map_err(|e| RoxError::InvalidFormat(format!("Invalid UTF-8: {e}")))?;

    serde_yaml::from_str(content).map_err(|e| RoxError::InvalidFormat(format!("Invalid YAML: {e}")))
}
