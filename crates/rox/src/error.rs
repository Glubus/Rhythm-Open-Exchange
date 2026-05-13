#[cfg(not(feature = "std"))]
use alloc::string::String;

use thiserror::Error;

/// Result type alias for ROX operations.
pub type RoxResult<T> = Result<T, RoxError>;

/// All errors that can occur during ROX encoding, decoding, or validation.
#[derive(Debug, Error)]
pub enum RoxError {
    #[cfg(feature = "std")]
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialize(String),

    #[error("deserialization error: {0}")]
    Deserialize(String),

    #[error("invalid format: {0}")]
    InvalidFormat(String),

    #[error("unsupported version: {0}")]
    UnsupportedVersion(u8),

    #[error("invalid column {column} for {key_count}K chart")]
    InvalidColumn { column: u8, key_count: u8 },

    #[error("invalid hold/burst duration {duration_us}µs at time {time_us}µs (must be > 0)")]
    InvalidHoldDuration { time_us: i64, duration_us: i64 },

    #[error("timing points not sorted: found {time_us}µs after {prev_time_us}µs")]
    TimingPointsNotSorted { prev_time_us: i64, time_us: i64 },

    #[error("overlapping notes on column {column} at time {time_us}µs")]
    OverlappingNotes { column: u8, time_us: i64 },

    #[error("notes not sorted: found {time_us}µs after {prev_time_us}µs")]
    NotesNotSorted { prev_time_us: i64, time_us: i64 },

    #[error("no BPM timing point found (at least one is required when notes exist)")]
    NoBpmTimingPoint,

    #[error("first BPM timing point at {bpm_time_us}µs is after first note at {note_time_us}µs")]
    BpmAfterFirstNote { bpm_time_us: i64, note_time_us: i64 },

    #[error("parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },

    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("key count must be > 0")]
    InvalidKeyCount,

    #[error("coop mode requires even key count, got {0}")]
    InvalidCoopKeyCount(u8),
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(RoxError::InvalidKeyCount, "key count")]
    #[case(RoxError::NoBpmTimingPoint, "bpm")]
    #[case(RoxError::InvalidColumn { column: 4, key_count: 4 }, "column 4")]
    #[case(RoxError::InvalidCoopKeyCount(3), "3")]
    fn test_error_display_contains_fragment(#[case] err: RoxError, #[case] fragment: &str) {
        let msg = err.to_string().to_lowercase();
        assert!(msg.contains(fragment), "expected '{fragment}' in '{msg}'");
    }
}
