//! # Rhythm Open Exchange (ROX)
//!
//! A universal, compact binary format for VSRG (Vertical Scrolling Rhythm Games).
//! Can be used as a pivot format for converting between different rhythm game formats.

#![warn(clippy::pedantic)]

#[cfg(feature = "analysis")]
pub mod analysis;
pub mod codec;
pub mod error;
pub mod model;
pub mod prelude;

#[cfg(test)]
pub mod test_utils;

// Re-exports for convenience
#[cfg(feature = "compression")]
pub use codec::RoxCodec;
pub use codec::{
    Decoder, Encoder, InputFormat, OutputFormat, auto_convert, auto_decode, auto_encode,
    encode_with_format, from_bytes, from_string,
};
pub use error::{RoxError, RoxResult};
pub use model::{Hitsound, Metadata, Note, NoteType, RoxChart, TimingPoint};
