#![allow(clippy::doc_markdown)]
//! StepMania (`.sm`) format converter.
//!
//! Supports dance-single (4K), dance-solo (6K), and dance-double (8K) charts.

pub mod decoder;
pub mod encoder;
pub mod parser;
pub mod types;

pub use decoder::SmDecoder;
pub use encoder::SmEncoder;
