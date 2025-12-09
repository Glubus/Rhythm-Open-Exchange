# Rhythm Open Exchange (ROX)

A universal, compact binary format for Vertical Scrolling Rhythm Games (VSRG). ROX serves as a pivot format for converting between different rhythm game formats like osu!mania, Quaver, StepMania, Etterna, and BMS.

> "so i'm just trying to make a ffmpeg of vsrg game idk where i'm going"

## Overview

ROX is designed to be:

- **Compact** — Uses bincode for minimal file size with variable-length integer encoding
- **Precise** — Microsecond timestamp precision (i64) for accurate timing
- **Universal** — Supports all common VSRG features across different games
- **Verifiable** — BLAKE3 content hashing for integrity verification 

## Features

- Support for any key count (4K, 7K, 9K, etc.)
- Multiple note types: Tap, Hold (LN), Burst/Roll, Mine
- BPM changes and Scroll Velocity (SV) modifications
- Keysound support for BMS/O2Jam style charts
- Comprehensive metadata (title, artist, difficulty, tags, etc.)
- Content-based hashing for chart identification

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
rhythm-open-exchange = "0.1"
```

Or clone and build locally:

```bash
git clone https://github.com/Glubus/Rhythm-Open-Exchange.git
cd Rhythm-Open-Exchange
cargo build --release
```

## Quick Start

### Creating a Chart

```rust
use rhythm_open_exchange::{RoxChart, Note, TimingPoint, Metadata};

// Create a 4K chart
let mut chart = RoxChart::new(4);

// Set metadata
chart.metadata = Metadata {
    title: "My Song".into(),
    artist: "Artist Name".into(),
    creator: "Your Name".into(),
    difficulty_name: "Hard".into(),
    audio_file: "audio.ogg".into(),
    ..Default::default()
};

// Add a BPM timing point at the start
chart.timing_points.push(TimingPoint::bpm(0, 180.0));

// Add notes (time in microseconds, column index)
chart.notes.push(Note::tap(1_000_000, 0));      // Tap at 1s, column 0
chart.notes.push(Note::tap(1_500_000, 1));      // Tap at 1.5s, column 1
chart.notes.push(Note::hold(2_000_000, 500_000, 2)); // Hold at 2s, 0.5s duration
```

### Encoding and Decoding

```rust
use rhythm_open_exchange::{RoxCodec, Encoder, Decoder};

// Encode to bytes
let bytes = RoxCodec::encode(&chart)?;

// Save to file
RoxCodec::encode_to_path(&chart, "chart.rox")?;

// Load from file
let loaded = RoxCodec::decode_from_path("chart.rox")?;

// Decode from bytes
let decoded = RoxCodec::decode(&bytes)?;
```

### Chart Hashing

```rust
// Get full BLAKE3 hash (64 hex characters)
let hash = chart.hash();

// Get short hash for display (16 hex characters)
let short = chart.short_hash();
println!("Chart ID: {}", short);
```

### Converting osu!mania files

```rust
use rhythm_open_exchange::codec::formats::osu::{OsuDecoder, OsuEncoder};
use rhythm_open_exchange::codec::{Decoder, Encoder};

// Load .osu file
let chart = OsuDecoder::decode_from_path("song.osu")?;
println!("Loaded: {} [{}]", chart.metadata.title, chart.metadata.difficulty_name);

// Convert to .rox (compact binary)
RoxCodec::encode_to_path(&chart, "output/song.rox")?;

// Or export back to .osu
OsuEncoder::encode_to_path(&chart, "output/song_converted.osu")?;
```

## API Reference


### Core Types

| Type | Description |
|------|-------------|
| `RoxChart` | Main chart container with metadata, timing, notes, and hitsounds |
| `Metadata` | Song and chart information (title, artist, difficulty, etc.) |
| `Note` | A single note with type, timing, column, and optional keysound |
| `NoteType` | Enum: `Tap`, `Hold`, `Burst`, `Mine` |
| `TimingPoint` | BPM or scroll velocity change |
| `Hitsound` | Keysound sample reference |

### Traits

| Trait | Description |
|-------|-------------|
| `Encoder` | Encode a chart to bytes or file |
| `Decoder` | Decode a chart from bytes or file |

### Note Constructors

```rust
Note::tap(time_us, column)           // Single tap
Note::hold(time_us, duration_us, column)  // Long note
Note::burst(time_us, duration_us, column) // Roll/burst
Note::mine(time_us, column)          // Mine (avoid)
```

### TimingPoint Constructors

```rust
TimingPoint::bpm(time_us, bpm)       // BPM change
TimingPoint::sv(time_us, multiplier) // Scroll velocity change
```

## File Format

ROX files use the `.rox` extension and have the following structure:

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0x00 | Magic | `[u8; 4]` | `ROX\0` (0x524F5800) |
| 0x04 | Data | zstd + bincode | Compressed, serialized RoxChart |

The chart data is serialized using bincode with:
- Little-endian byte order
- Variable-length integer encoding
- Zstd compression (level 3)

## Performance

ROX is built for extreme efficiency. Benchmarks on a massive 50,000 note chart (4K) show:

| Metric | .osu Format | .rox Format | Improvement |
|--------|-------------|-------------|-------------|
| **File Size** | 1.55 MB | **50 KB** | **97% Smaller** |
| **Decode Speed** | ~26 ms | **~2.7 ms** | **10x Faster** |
| **Encode Speed** | N/A | **~4.2 ms** | Lightning Fast |

*Benchmarks run on release build with `cargo bench`.*

## Roadmap

### Format Converters

| Format | Status | Import | Export |
|--------|--------|--------|--------|
| osu!mania (.osu) | **Implemented** | Yes | Yes |
| osu!taiko (.osu) | **Implemented** | Yes | No |
| StepMania / Etterna (.sm/.ssc) | **Implemented** | Yes | Yes |
| Quaver (.qua) | Planned | Yes | Yes |
| Friday Night Funkin' (.json) | Planned | Yes | Yes |
| Malody (.mc) | Planned | Yes | Yes |
| RoBeats | Planned | Yes | Yes |
| Clone Hero (.chart/.mid) | Planned (Experimental) | Yes | Yes |
| BMS (.bms/.bme/.bml) | Planned | Yes | No |
| O2Jam (.ojn/.ojm) | Planned | Yes | No |

### Alternative Formats

| Extension | Format | Use Case |
|-----------|--------|----------|
| `.rox` | Binary (zstd compressed) | Production, distribution |
| `.jrox` | JSON | Human-readable, debugging |
| `.yrox` | YAML | Human-readable, editing |

### Planned Features

- Chart difficulty calculation
- Pattern analysis utilities
- Timing section helpers
- Batch conversion CLI tool
- WebAssembly support

## Development

### Prerequisites

- Rust 1.85+ (2024 edition)
- Cargo

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

### Running Specific Tests

```bash
cargo test codec    # Run codec tests
cargo test model    # Run model tests
cargo test integration  # Run integration tests
```

## Project Structure

```text
rhythm-open-exchange/
├── src/
│   ├── lib.rs              # Library entry point and re-exports
│   ├── error.rs            # Error types (RoxError, RoxResult)
│   ├── codec/
│   │   ├── mod.rs          # Codec module
│   │   ├── traits.rs       # Encoder/Decoder traits
│   │   ├── rox.rs          # RoxCodec (with zstd + delta encoding)
│   │   └── formats/        # Format converters
│   │       └── osu/        # osu!mania (.osu) converter
│   └── model/
│       ├── chart.rs        # RoxChart struct
│       ├── metadata.rs     # Metadata struct
│       ├── note.rs         # Note and NoteType
│       ├── timing.rs       # TimingPoint
│       └── hitsound.rs     # Hitsound
├── tests/
│   ├── common/             # Test utilities
│   ├── codec/formats/osu/  # osu format tests
│   ├── codec_tests.rs
│   ├── model_tests.rs
│   └── integration_tests.rs
├── examples/               # Usage examples
├── assets/                 # Test assets (.osu files)
├── output/                 # Generated files (gitignored)
├── .wiki/                  # Documentation wiki
└── README.md
```


## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Code Style

- Follow Rust standard formatting (`cargo fmt`)
- Ensure tests pass (`cargo test`)
- Add tests for new functionality
- Update documentation as needed

## License

This project is licensed under the MIT License. See the LICENSE file for details.

## See Also

- [Wiki Documentation](.wiki/) — Detailed format specification
- [osu!mania](https://osu.ppy.sh/wiki/en/Game_mode/osu%21mania) — Popular VSRG
- [Quaver](https://quavergame.com/) — Competitive VSRG
- [Etterna](https://etternaonline.com/) — Advanced VSRG