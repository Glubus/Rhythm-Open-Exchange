#![warn(clippy::pedantic)]
use super::types::QuaChart;
use rox::error::{RoxError, RoxResult};

const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Parse a `.qua` file into a [`QuaChart`].
/// # Errors
/// Returns an error if data exceeds 100MB or YAML is malformed.
pub fn parse(data: &[u8]) -> RoxResult<QuaChart> {
    if data.len() > MAX_FILE_SIZE {
        return Err(RoxError::InvalidFormat(format!(
            "File too large: {} bytes (max 100MB)",
            data.len()
        )));
    }
    let content = std::str::from_utf8(data)
        .map_err(|e| RoxError::InvalidFormat(format!("Invalid UTF-8: {e}")))?;
    serde_yaml::from_str(content).map_err(|e| RoxError::InvalidFormat(format!("Invalid YAML: {e}")))
}
