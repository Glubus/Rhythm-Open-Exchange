//! Codec module for encoding/decoding ROX format.

mod rox;
mod traits;

pub use rox::RoxCodec;
pub use traits::{Decoder, Encoder};
