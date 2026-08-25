# Codec API

The codec API defines how chart data flows in and out of the `RoxChart` model.

## Traits

### Encoder

Implement `encode_inner`. The `encode` method calls `validate()` first — do not override it.

```rust
pub trait Encoder {
    fn encode_inner(chart: &RoxChart) -> RoxResult<Vec<u8>>;

    // Validates then calls encode_inner
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>>;

    // Writes to a file path (requires std)
    fn encode_to_path(chart: &RoxChart, path: impl AsRef<Path>) -> RoxResult<()>;

    // Returns UTF-8 string (for text formats like .osu, .sm)
    fn encode_to_string(chart: &RoxChart) -> RoxResult<String>;
}
```

### Decoder

Implement `decode_inner`. The `decode` method calls `validate()` after — do not override it.

```rust
pub trait Decoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart>;

    // Calls decode_inner then validates
    fn decode(data: &[u8]) -> RoxResult<RoxChart>;

    // Reads from a file path (requires std)
    fn decode_from_path(path: impl AsRef<Path>) -> RoxResult<RoxChart>;
}
```

### Format

Provides file extension metadata. Use `#[derive(Format)]` to avoid boilerplate.

```rust
pub trait Format {
    const EXTENSIONS: &'static [&'static str];
    fn supports_extension(ext: &str) -> bool;
}
```

```rust
#[derive(Format)]
#[format(extensions = ["osu"])]
pub struct OsuDecoder;
```

## Conversion Utilities

```rust
// Convert bytes from format D to format E (ROX as pivot)
pub fn convert<D: Decoder, E: Encoder>(data: &[u8]) -> RoxResult<Vec<u8>>;

// Convert files by path (requires std)
pub fn convert_file<D: Decoder, E: Encoder>(input: &Path, output: &Path) -> RoxResult<()>;
```

## Validation

`RoxChart::validate()` runs automatically on every `encode` and `decode` call. It checks:

| Rule | Error |
|------|-------|
| `key_count > 0` | `InvalidKeyCount` |
| Co-op requires even key count | `InvalidCoopKeyCount` |
| Timing points sorted | `TimingPointsNotSorted` |
| At least one BPM point when notes exist | `NoBpmTimingPoint` |
| First BPM ≤ first note time | `BpmAfterFirstNote` |
| Notes sorted by time | `NotesNotSorted` |
| Column < key_count for all notes | `InvalidColumn` |
| No overlapping notes per column | `OverlappingNotes` |
| Hold/burst duration > 0 | `InvalidHoldDuration` |

## Error Types

```rust
pub enum RoxError {
    Io(std::io::Error),              // file read/write
    Serialize(String),               // encoding failed
    Deserialize(String),             // decoding failed
    InvalidFormat(String),           // missing magic, bad structure
    UnsupportedVersion(u8),
    InvalidColumn { column: u8, key_count: u8 },
    InvalidHoldDuration { time_us: i64, duration_us: i64 },
    TimingPointsNotSorted { prev_time_us: i64, time_us: i64 },
    OverlappingNotes { column: u8, time_us: i64 },
    NotesNotSorted { prev_time_us: i64, time_us: i64 },
    NoBpmTimingPoint,
    BpmAfterFirstNote { bpm_time_us: i64, note_time_us: i64 },
    ParseError { line: usize, message: String },
    UnsupportedFormat(String),
    InvalidKeyCount,
    InvalidCoopKeyCount(u8),
}
```

## Implementing a New Format

See [Format Converters](Format-Converters) for the full guide.

```rust
use rox::codec::{Decoder, Encoder};
use rox::error::{RoxError, RoxResult};
use rox::model::RoxChart;
pub struct XyzDecoder;
rox::impl_format!(XyzDecoder, ["xyz"]);

impl Decoder for XyzDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        let content = std::str::from_utf8(data)
            .map_err(|e| RoxError::InvalidFormat(e.to_string()))?;
        let mut chart = RoxChart::new(4);
        // parse content into chart...
        Ok(chart)
    }
}
```
