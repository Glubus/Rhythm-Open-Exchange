//! Error types for ROX format operations.

use thiserror::Error;

/// Result type alias for ROX operations.
pub type RoxResult<T> = Result<T, RoxError>;

/// Errors that can occur during ROX encoding/decoding.
#[derive(Debug, Error)]
pub enum RoxError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Decode error: {0}")]
    Decode(#[from] bincode::error::DecodeError),

    #[error("Encode error: {0}")]
    Encode(#[from] bincode::error::EncodeError),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u8),

    #[error("Invalid column index {column} for {key_count}K chart")]
    InvalidColumn { column: u8, key_count: u8 },
}
