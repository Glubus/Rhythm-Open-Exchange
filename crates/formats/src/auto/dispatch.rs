use rox::prelude::RoxResult;

/// Automatically detect format and decode audio data.
///
/// # Errors
///
/// Returns `RoxError` if the input format cannot be detected or decoding fails.
pub fn auto_decode(_: &[u8]) -> RoxResult<()> { unimplemented!() }

/// Automatically encode audio data.
///
/// # Errors
///
/// Returns `RoxError` if encoding fails.
pub fn auto_encode(_: &()) -> RoxResult<Vec<u8>> { unimplemented!() }

/// Automatically detect format and convert audio data to another format.
///
/// # Errors
///
/// Returns `RoxError` if format detection or conversion fails.
pub fn auto_convert(_: &[u8], _: &str) -> RoxResult<Vec<u8>> { unimplemented!() }
