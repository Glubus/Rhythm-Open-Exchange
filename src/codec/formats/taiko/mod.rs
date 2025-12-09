//! osu!taiko format converter.
//!
//! Converts osu!taiko (`.osu` Mode 1) to 4K mania format.

pub mod decoder;
pub mod parser;

pub mod types;

pub use decoder::TaikoDecoder;
