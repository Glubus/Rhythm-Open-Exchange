//! Native ROX binary codec with optional zstd compression.
#![cfg(feature = "compression")]

// Safety limit: 100MB to prevent memory exhaustion
// Pub(crate) so decoder and tests can access it
pub(crate) const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Native ROX format codec using rkyv for zero-copy binary serialization
/// and zstd for compression (native only). Uses delta encoding for note timestamps.
pub struct RoxCodec;

mod decoder;
mod encoder;

#[cfg(test)]
mod tests;
