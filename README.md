# Rhythm Open Exchange (ROX)

A universal, compact binary format for Vertical Scrolling Rhythm Games (VSRG). ROX serves as a pivot format for converting between different rhythm game formats like osu!mania, Quaver, StepMania, Etterna, and BMS.

> "so i'm just trying to make a ffmpeg of vsrg game idk where i'm going"

## Overview

ROX is designed to be:

- **Compact** — Uses rkyv zero-copy serialization with zstd compression
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
- **C# bindings** for Unity/Godot/.NET integration

## Installation

### Rust

Add to your `Cargo.toml`:

```toml
[dependencies]
rhythm-open-exchange = "0.4"
```

### C# / .NET

```bash
dotnet add package RhythmOpenExchange
```

### From Source

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

### Auto-Converting Formats

```rust
use rhythm_open_exchange::{auto_decode, auto_encode, auto_convert};

// Load any supported format (auto-detected from extension)
let chart = auto_decode("chart.osu")?;

// Export to any format
auto_encode(&chart, "chart.sm")?;

// Or convert directly
auto_convert("input.osu", "output.qua")?;
```

### C# Usage

```csharp
using RhythmOpenExchange;

// Load a chart
byte[] data = File.ReadAllBytes("chart.osu");
using var chart = RoxChart.FromBytes(data);

Console.WriteLine($"{chart.Title} by {chart.Artist}");
Console.WriteLine($"{chart.KeyCount}K - {chart.NoteCount} notes");

// Convert to StepMania
string? sm = chart.ToString(RoxFormat.Sm);
File.WriteAllText("chart.sm", sm);
```

## Supported Formats

| Format | Extension | Read | Write |
|--------|-----------|------|-------|
| ROX (native binary) | `.rox` | ✅ | ✅ |
| osu!mania | `.osu` | ✅ | ✅ |
| osu!taiko | `.osu` | ✅ | ❌ |
| StepMania / Etterna | `.sm` | ✅ | ✅ |
| Quaver | `.qua` | ✅ | ✅ |
| Friday Night Funkin' | `.json` | ✅ | ✅ |

### Planned

- Malody (`.mc`)
- BMS (`.bms/.bme/.bml`)
- O2Jam (`.ojn/.ojm`)
- Clone Hero (`.chart/.mid`)

## CLI Tool

```bash
# Convert a file
cargo run --bin rox -- convert input.osu output.sm

# Validate a file
cargo run --bin rox -- validate chart.sm
```

## Performance

ROX is built for extreme efficiency. Benchmarks on a 50,000 note chart (4K):

| Metric | .osu Format | .rox Format | Improvement |
|--------|-------------|-------------|-------------|
| **File Size** | 1.55 MB | **50 KB** | **97% Smaller** |
| **Decode Speed** | ~26 ms | **~2.7 ms** | **10x Faster** |
| **Encode Speed** | N/A | **~4.2 ms** | Lightning Fast |

## Development

### Prerequisites

- Rust 1.85+ (2024 edition)
- Cargo
- (Optional) just - for running QA checks

### Building

```bash
cargo build --release
```

### Running QA Checks

```bash
# If you have 'just' installed
just qa

# Or manually
cargo check --all-targets
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Project Structure

```text
rhythm-open-exchange/
├── src/
│   ├── lib.rs              # Library entry point
│   ├── api.rs              # FFI API for C#/native bindings
│   ├── error.rs            # Error types
│   ├── codec/
│   │   ├── mod.rs          # Codec module
│   │   ├── auto.rs         # Auto-detection & conversion
│   │   ├── rox.rs          # Native codec (rkyv + zstd)
│   │   └── formats/        # Format converters
│   │       ├── osu/        # osu!mania & osu!taiko
│   │       ├── sm/         # StepMania
│   │       ├── qua/        # Quaver
│   │       └── fnf/        # Friday Night Funkin'
│   └── model/              # Data structures
├── bindings/
│   └── csharp/             # C# NuGet package
├── tests/                  # Test suite
├── examples/               # Usage examples
├── assets/                 # Test assets
└── justfile                # QA automation
```

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Run QA checks (`just qa` or manual commands above)
4. Commit your changes (`git commit -m 'Add amazing feature'`)
5. Push and open a Pull Request

## License

This project is licensed under the MIT License.

## See Also

- [C# Bindings Documentation](bindings/csharp/README.md)
- [Wiki Documentation](.wiki/)
- [osu!mania](https://osu.ppy.sh/wiki/en/Game_mode/osu%21mania)
- [Quaver](https://quavergame.com/)
- [Etterna](https://etternaonline.com/)