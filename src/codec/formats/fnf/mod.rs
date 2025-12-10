//! Friday Night Funkin' .json format codec.
//!
//! FNF charts are JSON files containing:
//! - Song metadata (name, BPM, speed, characters)
//! - Sections with notes organized by `mustHitSection`
//! - Notes as `[time_ms, lane, duration_ms]` arrays
//!
//! # Side Selection
//!
//! FNF has two sides: player and opponent. When decoding:
//! - `FnfSide::Player` - Extract player notes only (4K)
//! - `FnfSide::Opponent` - Extract opponent notes only (4K)
//! - `FnfSide::Both` - Both sides combined (8K)

pub mod decoder;
pub mod encoder;
pub mod parser;
pub mod types;

pub use decoder::FnfDecoder;
pub use encoder::FnfEncoder;
pub use types::FnfSide;
