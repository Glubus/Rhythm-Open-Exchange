//! Prelude module for convenient imports.
//!
//! Import everything you need with a single use statement:
//! ```
//! use rhythm_open_exchange::prelude::*;
//! ```

pub use crate::codec::{
    Decoder, Encoder, Format, InputFormat, OutputFormat, RoxCodec, auto_convert, auto_decode,
    auto_encode, from_bytes, from_string,
};
pub use crate::error::{RoxError, RoxResult};
pub use crate::model::{Hitsound, Metadata, Note, NoteType, RoxChart, TimingPoint};
