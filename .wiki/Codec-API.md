# Codec API

The codec API provides traits and implementations for encoding/decoding ROX charts to various formats.

## Traits

### Encoder

Convert a `RoxChart` to bytes, file, or string:

```rust
pub trait Encoder {
    /// Encode a chart to raw bytes.
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>>;

    /// Encode a chart to a file path.
    fn encode_to_path(chart: &RoxChart, path: impl AsRef<Path>) -> RoxResult<()>;

    /// Encode a chart to a String (for text-based formats like .osu).
    fn encode_to_string(chart: &RoxChart) -> RoxResult<String>;
}
```


### Decoder

Convert bytes or file to a `RoxChart`:

```rust
pub trait Decoder {
    /// Decode a chart from raw bytes.
    fn decode(data: &[u8]) -> RoxResult<RoxChart>;

    /// Decode a chart from a file path.
    fn decode_from_path(path: impl AsRef<Path>) -> RoxResult<RoxChart>;
}
```

## RoxCodec

The native ROX format codec:

```rust
use rhythm_open_exchange::{RoxCodec, RoxChart, Encoder, Decoder};

let chart = RoxChart::new(4);

// Encode to bytes
let bytes = RoxCodec::encode(&chart)?;

// Encode to file
RoxCodec::encode_to_path(&chart, "chart.rox")?;

// Decode from bytes
let decoded = RoxCodec::decode(&bytes)?;

// Decode from file
let loaded = RoxCodec::decode_from_path("chart.rox")?;
```

## Binary Format

RoxCodec uses bincode with:

| Setting | Value |
|---------|-------|
| Byte order | Little-endian |
| Integer encoding | Variable-length |
| String encoding | Length-prefixed |

### File Structure

```
┌──────────────────────────────────────┐
│ Magic Bytes (4 bytes)                │
│ "ROX\0" = [0x52, 0x4F, 0x58, 0x00]   │
├──────────────────────────────────────┤
│ Bincode-encoded RoxChart             │
│ - version: u8                        │
│ - key_count: u8                      │
│ - metadata: Metadata                 │
│ - timing_points: Vec<TimingPoint>    │
│ - notes: Vec<Note>                   │
│ - hitsounds: Vec<Hitsound>           │
└──────────────────────────────────────┘
```

## Error Handling

The `RoxResult<T>` type alias wraps `Result<T, RoxError>`:

```rust
pub type RoxResult<T> = Result<T, RoxError>;

pub enum RoxError {
    Io(std::io::Error),
    Decode(bincode::error::DecodeError),
    Encode(bincode::error::EncodeError),
    InvalidFormat(String),
    UnsupportedVersion(u8),
    InvalidColumn { column: u8, key_count: u8 },
}
```

### Common Errors

| Error | Cause |
|-------|-------|
| `InvalidFormat` | Missing/wrong magic bytes |
| `Decode` | Corrupted or invalid data |
| `InvalidColumn` | Note column >= key_count |
| `Io` | File read/write failure |

## Validation

Charts are validated before encoding:

```rust
let mut chart = RoxChart::new(4);
chart.notes.push(Note::tap(0, 5)); // Invalid column!

match RoxCodec::encode(&chart) {
    Ok(_) => println!("Encoded successfully"),
    Err(RoxError::InvalidColumn { column, key_count }) => {
        println!("Column {} invalid for {}K", column, key_count);
    }
    Err(e) => println!("Other error: {}", e),
}
```

## Implementing Custom Codecs

To support other formats, implement `Encoder` and/or `Decoder`:

```rust
use rhythm_open_exchange::{Decoder, Encoder, RoxChart, RoxResult, RoxError};

pub struct OsuCodec;

impl Decoder for OsuCodec {
    fn decode(data: &[u8]) -> RoxResult<RoxChart> {
        let content = std::str::from_utf8(data)
            .map_err(|e| RoxError::InvalidFormat(e.to_string()))?;
        
        let mut chart = RoxChart::new(4); // Parse key count from [Difficulty]
        
        // Parse [General], [Metadata], [Editor], [Difficulty]
        // Parse [TimingPoints]
        // Parse [HitObjects]
        
        Ok(chart)
    }
}

impl Encoder for OsuCodec {
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        let mut output = String::new();
        
        output.push_str("osu file format v14\n\n");
        output.push_str("[General]\n");
        // ... format chart data
        
        Ok(output.into_bytes())
    }
}
```

## Batch Processing

Process multiple files:

```rust
use std::path::Path;
use rhythm_open_exchange::{RoxCodec, Decoder, Encoder};

fn convert_directory(input_dir: &Path, output_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(input_dir)? {
        let path = entry?.path();
        
        if path.extension().map_or(false, |e| e == "rox") {
            let chart = RoxCodec::decode_from_path(&path)?;
            
            let output_path = output_dir.join(
                path.file_stem().unwrap()
            ).with_extension("rox");
            
            RoxCodec::encode_to_path(&chart, output_path)?;
        }
    }
    
    Ok(())
}
```

## Size Optimization

ROX is designed for minimal file size:

| Component | Optimization |
|-----------|--------------|
| Integers | Variable-length encoding |
| Strings | Length-prefixed (no null terminators) |
| Optionals | Single byte discriminant |
| Enums | Minimal discriminant size |

Typical sizes:

| Chart Complexity | Approximate Size |
|-----------------|------------------|
| Simple (100 notes) | ~500 bytes |
| Medium (500 notes) | ~2 KB |
| Dense (2000 notes) | ~8 KB |
| Marathon (5000+ notes) | ~20 KB |
