//! Format converters for various rhythm game formats.
//!
//! Each format has its own submodule with:
//! - `parser` - File parsing logic
//! - `encoder` - ROX -> Format conversion
//! - `decoder` - Format -> ROX conversion
//!
//! See `formats/README.md` for guidelines on implementing new formats.

pub mod fnf;
pub mod jrox;
pub mod osu;
pub mod qua;
#[cfg(feature = "compression")]
pub mod rox;
pub mod sm;
pub mod taiko;
pub mod yrox;

pub use fnf::{FnfDecoder, FnfEncoder, FnfSide};
pub use jrox::{JroxDecoder, JroxEncoder};
pub use osu::{OsuDecoder, OsuEncoder};
pub use qua::{QuaDecoder, QuaEncoder};
#[cfg(feature = "compression")]
pub use rox::RoxCodec;
pub use sm::{SmDecoder, SmEncoder};
pub use taiko::TaikoDecoder;
pub use yrox::{YroxDecoder, YroxEncoder};
