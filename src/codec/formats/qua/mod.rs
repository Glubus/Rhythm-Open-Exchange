//! Quaver .qua format codec.
//!
//! Quaver files are YAML-based and contain:
//! - Metadata (title, artist, creator, etc.)
//! - Timing points (BPM changes)
//! - Slider velocities (scroll speed changes)
//! - Hit objects (notes and holds)

pub mod decoder;
pub mod encoder;
pub mod parser;
pub mod types;

pub use decoder::QuaDecoder;
pub use encoder::QuaEncoder;
