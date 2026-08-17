#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{format, string::ToString};

use super::types::FnfChart;
use rox::error::{RoxError, RoxResult};

const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Parse an FNF `.json` file into an [`FnfChart`].
/// # Errors
/// Returns an error if data exceeds 100MB or JSON is malformed.
pub fn parse(data: &[u8]) -> RoxResult<FnfChart> {
    if data.len() > MAX_FILE_SIZE {
        return Err(RoxError::InvalidFormat(format!(
            "File too large: {} bytes (max 100MB)",
            data.len()
        )));
    }
    serde_json::from_slice(data)
        .map_err(|e| RoxError::InvalidFormat(format!("FNF parse error: {e}")))
}
