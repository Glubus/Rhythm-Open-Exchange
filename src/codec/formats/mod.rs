//! Format converters for various rhythm game formats.
//!
//! Each format has its own submodule with:
//! - `parser` - File parsing logic
//! - `encoder` - ROX -> Format conversion
//! - `decoder` - Format -> ROX conversion
//!
//! See `formats/README.md` for guidelines on implementing new formats.

pub mod osu;
pub mod sm;
pub mod taiko;

pub use osu::{OsuDecoder, OsuEncoder};
pub use sm::{SmDecoder, SmEncoder};
pub use taiko::TaikoDecoder;
