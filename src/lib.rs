//! # Rhythm Open Exchange (ROX)
//!
//! A universal, compact binary format for VSRG (Vertical Scrolling Rhythm Games).
//! Serves as a pivot format for converting between different rhythm game formats.

pub mod codec;
pub mod error;
pub mod model;

// Re-exports for convenience
pub use codec::{Decoder, Encoder, RoxCodec};
pub use error::{RoxError, RoxResult};
pub use model::{Hitsound, Metadata, Note, NoteType, RoxChart, TimingPoint};
