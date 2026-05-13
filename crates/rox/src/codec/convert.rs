#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use super::traits::{Decoder, Encoder};
use crate::error::RoxResult;

/// Convert bytes from format D to format E, using ROX as the pivot.
///
/// # Errors
///
/// Returns an error if decoding or encoding fails.
pub fn convert<D: Decoder, E: Encoder>(data: &[u8]) -> RoxResult<Vec<u8>> {
    let chart = D::decode(data)?;
    E::encode(&chart)
}

/// Convert a file from format D to format E.
///
/// # Errors
///
/// Returns an error if reading, decoding, encoding, or writing fails.
#[cfg(feature = "std")]
pub fn convert_file<D: Decoder, E: Encoder>(
    input: impl AsRef<std::path::Path>,
    output: impl AsRef<std::path::Path>,
) -> RoxResult<()> {
    let chart = D::decode_from_path(input)?;
    E::encode_to_path(&chart, output)
}
