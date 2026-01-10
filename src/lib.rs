//! # Rhythm Open Exchange (ROX)
//!
//! A universal, compact binary format for VSRG (Vertical Scrolling Rhythm Games).
//! Serves as a pivot format for converting between different rhythm game formats.

#![warn(clippy::pedantic)]

pub mod codec;
pub mod error;
pub mod model;
pub mod prelude;

// Re-exports for convenience
pub use codec::{
    Decoder, Encoder, RoxCodec, auto_convert, auto_decode, auto_encode, from_bytes, from_string,
};
pub use error::{RoxError, RoxResult};
pub use model::{Hitsound, Metadata, Note, NoteType, RoxChart, TimingPoint};
