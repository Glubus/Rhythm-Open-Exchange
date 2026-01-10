//! Codec module for encoding/decoding ROX format.
//!
//! This module provides:
//! - [`Decoder`] and [`Encoder`] traits for format conversion
//! - [`RoxCodec`] for native ROX binary format
//! - Auto-detection functions for automatic format handling by extension
//!
//! # Auto-Detection Example
//! ```ignore
//! use rox::codec::{auto_decode, auto_encode, auto_convert};
//!
//! // Decode any supported format
//! let chart = auto_decode("chart.osu")?;
//!
//! // Encode to any supported format
//! auto_encode(&chart, "output.sm")?;
//!
//! // Convert between formats in one call
//! auto_convert("input.osu", "output.rox")?;
//! ```

mod auto;
pub mod formats;
mod rox;
mod traits;

pub use auto::{
    InputFormat, OutputFormat, auto_convert, auto_decode, auto_encode, decode_with_format,
    encode_with_format, from_bytes, from_string,
};
pub use rox::RoxCodec;
pub use traits::{Decoder, Encoder, Format, convert, convert_file};
