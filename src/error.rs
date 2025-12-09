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

    #[error("Invalid hold duration {duration_us}µs at time {time_us}µs (must be > 0)")]
    InvalidHoldDuration { time_us: i64, duration_us: i64 },

    #[error("Timing points not sorted by time (found {time_us}µs after {prev_time_us}µs)")]
    TimingPointsNotSorted { prev_time_us: i64, time_us: i64 },

    #[error("Overlapping notes on column {column} at time {time_us}µs")]
    OverlappingNotes { column: u8, time_us: i64 },

    #[error("No BPM timing point found (at least one is required)")]
    NoBpmTimingPoint,

    #[error("First BPM timing point at {bpm_time_us}µs is after first note at {note_time_us}µs")]
    BpmAfterFirstNote { bpm_time_us: i64, note_time_us: i64 },

    #[error("Parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },
}
