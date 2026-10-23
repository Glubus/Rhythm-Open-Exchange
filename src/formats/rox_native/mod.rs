#![warn(clippy::pedantic)]

pub mod decoder;
pub mod encoder;

pub use decoder::RoxNativeCodec;

pub(super) const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;
