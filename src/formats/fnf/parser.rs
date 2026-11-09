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
    // The game's own legacy charts (v0.2.x) end with NUL padding after the object: the chart is the first JSON value.
    let mut values = serde_json::Deserializer::from_slice(data).into_iter::<FnfChart>();
    match values.next() {
        Some(chart) => chart.map_err(|e| RoxError::InvalidFormat(format!("FNF parse error: {e}"))),
        None => Err(RoxError::InvalidFormat("FNF parse error: empty file".to_string())),
    }
}
