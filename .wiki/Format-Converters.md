# Format Converters

This directory contains converters for various rhythm game formats to/from ROX.

## Directory Structure

Each format should follow this structure:

```
formats/
├── mod.rs              # Module exports
├── README.md           # This file
└── {format_name}/
    ├── mod.rs          # Format module exports
    ├── parser/
    │   ├── mod.rs      # Parser exports
    │   └── {module}.rs # Parsing logic (file reading, data extraction)
    ├── encoder/
    │   ├── mod.rs      # Encoder exports
    │   └── {module}.rs # ROX -> Format conversion
    └── decoder/
        ├── mod.rs      # Decoder exports
        └── {module}.rs # Format -> ROX conversion
```

## Example: osu!mania

```
formats/
└── osu/
    ├── mod.rs
    ├── parser/
    │   ├── mod.rs
    │   ├── general.rs      # [General] section
    │   ├── metadata.rs     # [Metadata] section
    │   ├── difficulty.rs   # [Difficulty] section
    │   ├── timing.rs       # [TimingPoints] section
    │   └── hitobjects.rs   # [HitObjects] section
    ├── encoder/
    │   ├── mod.rs
    │   └── osu_encoder.rs  # RoxChart -> .osu
    └── decoder/
        ├── mod.rs
        └── osu_decoder.rs  # .osu -> RoxChart
```

## Creating a New Converter

### 1. Create the Directory Structure

```bash
mkdir -p src/codec/formats/{format_name}/{parser,encoder,decoder}
```

### 2. Implement the Decoder (Format -> ROX)

```rust
// formats/{format_name}/decoder/{format}_decoder.rs

use crate::error::RoxResult;
use crate::model::RoxChart;
use crate::codec::Decoder;

pub struct {Format}Decoder;

impl Decoder for {Format}Decoder {
    fn decode(data: &[u8]) -> RoxResult<RoxChart> {
        // 1. Parse the raw file format
        let parsed = super::parser::parse(data)?;
        
        // 2. Convert to RoxChart
        let mut chart = RoxChart::new(parsed.key_count);
        
        // 3. Map metadata
        chart.metadata.title = parsed.title;
        chart.metadata.artist = parsed.artist;
        // ... etc
        
        // 4. Convert timing points
        for tp in parsed.timing_points {
            chart.timing_points.push(convert_timing_point(tp));
        }
        
        // 5. Convert notes
        for note in parsed.notes {
            chart.notes.push(convert_note(note));
        }
        
        Ok(chart)
    }
}
```

### 3. Implement the Encoder (ROX -> Format)

```rust
// formats/{format_name}/encoder/{format}_encoder.rs

use crate::error::RoxResult;
use crate::model::RoxChart;
use crate::codec::Encoder;

pub struct {Format}Encoder;

impl Encoder for {Format}Encoder {
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        let mut output = String::new();
        
        // 1. Write header/metadata
        write_metadata(&mut output, &chart.metadata)?;
        
        // 2. Write timing points
        write_timing_points(&mut output, &chart.timing_points)?;
        
        // 3. Write notes
        write_notes(&mut output, &chart.notes, chart.key_count)?;
        
        Ok(output.into_bytes())
    }
}
```

### 4. Register in mod.rs

```rust
// formats/{format_name}/mod.rs

pub mod parser;
pub mod encoder;
pub mod decoder;

pub use encoder::{Format}Encoder;
pub use decoder::{Format}Decoder;
```

```rust
// formats/mod.rs

pub mod osu;
pub mod quaver;
// ... etc

pub use osu::{OsuEncoder, OsuDecoder};
pub use quaver::{QuaverEncoder, QuaverDecoder};
```

## Conversion Guidelines

### Timestamps

- ROX uses **microseconds** (`i64`)
- osu! uses **milliseconds** (`i32`)
- Quaver uses **milliseconds** (`f32`)

```rust
// osu! -> ROX
let time_us = osu_time_ms as i64 * 1000;

// ROX -> osu!
let time_ms = (rox_time_us / 1000) as i32;
```

### Note Types

| ROX | osu! | Quaver | StepMania |
|-----|------|--------|-----------|
| Tap | Circle | HitObject (EndTime=0) | 1 |
| Hold | Slider (LN) | HitObject (EndTime>0) | 2+3 |
| Burst | N/A | N/A | 4 |
| Mine | N/A | N/A | M |

### Column Mapping

osu! uses X position, convert with:
```rust
let column = (x * key_count / 512) as u8;
let x = column as i32 * 512 / key_count + 256 / key_count;
```

### Timing Points

| ROX | osu! | Quaver |
|-----|------|--------|
| `bpm` | `60000 / beatLength` | `Bpm` |
| `scroll_speed` | `-100 / beatLength` (inherited) | `Multiplier` |
| `is_inherited` | `uninherited == 0` | N/A (separate list) |

## Testing

Each format should have tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_roundtrip() {
        let original_data = include_bytes!("testdata/sample.osu");
        
        // Decode to ROX
        let chart = OsuDecoder::decode(original_data).unwrap();
        
        // Encode back
        let encoded = OsuEncoder::encode(&chart).unwrap();
        
        // Decode again
        let chart2 = OsuDecoder::decode(&encoded).unwrap();
        
        // Compare (some loss is acceptable)
        assert_eq!(chart.notes.len(), chart2.notes.len());
    }
}
```

## Supported Formats

| Format | Extension | Decode | Encode | Status |
|--------|-----------|--------|--------|--------|
| osu!mania | `.osu` | ✓ | ✓ | **Implemented** |
| Quaver | `.qua` | ✓ | ✓ | Planned |
| StepMania | `.sm` | ✓ | ✓ | **Implemented** |
| Etterna | `.sm` | ✓ | ✓ | Planned |
| Taiko | `.osu` | ✓ | ✓ | **Implemented** |
| BMS | `.bms/.bme` | ✓ | ✗ | Planned |
| O2Jam | `.ojn` | ✓ | ✗ | Planned |
