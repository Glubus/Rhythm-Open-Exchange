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
- Keysound support for BMS/O2Jam style charts (BMS and O2Jam are read: see `src/formats/bms/mod.rs` and `src/formats/o2jam/mod.rs` for the layout and timing decisions)
- Comprehensive metadata (title, artist, difficulty, tags, etc.)
- Content-based hashing for chart identification
- **C# bindings** for Unity/Godot/.NET integration

## Installation

### Rust

Add to your `Cargo.toml`:

```toml
[dependencies]
rox = { package = "rhythm-open-exchange", version = "0.8" }
```

The default build includes every codec and chart analysis. For a smaller model-only
build, disable defaults; enable modules individually as needed:

```toml
[dependencies]
rox = { package = "rhythm-open-exchange", version = "0.8", default-features = false, features = ["formats"] }
```

Available features: `std`, `formats` (all codecs and auto-detection), and
`analysis` (BPM, NPS, and content hashing). `formats-std` is retained as an
alias for `formats`.

### C# / .NET

```bash
dotnet add package RhythmOpenExchange --version 0.5.0
```

### From Source

```bash
git clone https://github.com/Glubus/Rhythm-Open-Exchange.git
cd Rhythm-Open-Exchange
cargo build --release
```

## Quick Start
<!-- slide -->
### Creating a Chart

```rust
use rox::{Metadata, Note, RoxChart, TimingPoint};

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
use rox::formats::{auto_convert, auto_decode, auto_encode};

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

| ROX (native binary) | `.rox` | ✅ | ✅ |
| osu!mania | `.osu` | ✅ | ✅ |
| osu!taiko | `.osu` | ✅ | ❌ |
| StepMania / Etterna | `.sm/.ssc` | ✅ | ✅ |
| Quaver | `.qua` | ✅ | ✅ |
| BMS family (Prism's addition) | `.bms/.bme/.bml/.pms/.bmson` | ✅ | ❌ |
| O2Jam (Prism's addition) | `.ojn` (charts), `.ojm` (sample container, `OjmContainer`) | ✅ | ❌ |
| Friday Night Funkin' (legacy `{"song": ...}`, Psych) | `.json` | ✅ | ✅ | (Experimental) |
| Clone Hero (Prism's addition) | `.chart` and `notes.mid` (5-lane tracks: guitar, co-op guitar, bass, rhythm, keys; a `.mid` is a chart only when it is NAMED `notes.mid`: `MidDecoder`), `song.ini` (`SongIni`), `.sng` package (`SngPackage`, `notes.mid` or `notes.chart`) | ✅ | ❌ |
| Friday Night Funkin' V-Slice (Prism's addition) | `<song>-chart.json` + `<song>-metadata.json`, `.fnfc` package | ✅ | ❌ |
| DDR (Prism's addition) | `.ssq` (Single and Double charts, no title or audio in the file) | ✅ | ❌ |
| StepMania edits (Prism's addition) | `.edit` (read with the song's `.sm`/`.ssc`: `EditDecoder`; `auto_decode` of a lone `.edit` fails with `edit needs its song`) | ✅ | ❌ |
| Dance With Intensity (Prism's addition) | `.dwi` (every Single, Solo and Double chart of the file; `DwiDecoder`) | ✅ | ❌ |
| Kick It Up / Direct Move (Prism's addition) | `.ksf` (one `pump-single` or `pump-halfdouble` chart per file, read with its file name: `KsfDecoder`) | ✅ | ❌ |
| Project SEKAI (Prism's addition) | `.sus` (Sliding Universal Score, one chart per file: `SusDecoder`) and `.usc` (Universal Sekai Chart, JSON, one chart per file: `UscDecoder`); 12 columns, slides as one hold, traces and guides dropped (`SekaiReport`) | ✅ | ❌ |
| BanG Dream (Prism's addition) | Bestdori chart (JSON array of `BPM`/`Single`/`Long`/`Slide`/`Directional`: `BestdoriDecoder`; a `.bestdori`/`.bestdori.json` name, or `<difficulty>.json` told by content) and BanG! Simulator `.txt` (`SimulatorDecoder`; told by content): one chart per file, 7 columns, slides as one hold, flick/directional/skill/charge counted and dropped (`BandoriReport`); ROX never claims a plain `.json` or `.txt` by extension | ✅ | ❌ |
| Rhythm Master (Prism's addition) | `.imd` (classic mode, one difficulty per file: `ImdDecoder`; 4, 5 or 6 columns from the `<song>_<4\|5\|6>k_<ez\|nm\|hd>` name or from the notes; chain slides and flicks simplified and counted in `ImdReport`; the BPM table as timing points; told by content, `looks_like_imd`); `.mde` and the `imdjson` `.json` are not read | ✅ | ❌ |
| DTXMania (Prism's addition) | `.dtx` (drums, guitar and bass of one song in one file: `DtxDecoder`, one variant per instrument that has chips; drums 1 to 9 columns from `LC HH SD HT BD LT FT CY RD` with the left pedal folded onto the bass drum, guitar and bass 4 or 6 columns `Open R G B [Y P]`; holds on `2C`/`2D`; hidden chips, pictures, bar lines and the like dropped and counted in `DtxReport`; the backing track and every chip's sample in the audio plan); `set.def`, `.gda` and `.g2d` are not read | ✅ | ❌ |
| SOUND VOLTEX (Prism's addition) | `.ksh` (K-Shoot MANIA, one difficulty per file: `KshDecoder`; 6 columns, BT A-D then FX-L and FX-R, chip = tap, long = hold; the analog lasers (not supported for now), FX effects, camera and scroll events dropped and counted in `KshReport`; `stop` moves no time; told by content, `looks_like_ksh`: `.ksh` is also the Korn shell extension); `.kson` and `.vox` are not read | ✅ | ❌ |

## Performance

ROX is built for extreme efficiency. Benchmarks on a 50,000 note chart (4K):

| Metric | .osu Format | .rox Format | Improvement |
|--------|-------------|-------------|-------------|
| **File Size** | 1.55 MB | **50 KB** | **97% Smaller** |
| **Decode Speed** | ~26 ms | **~0.82 ms** | **30x Faster** |
| **Encode Speed** | N/A | **~1.69 ms** | Lightning Fast |

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
cargo check --all-targets --all-features
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Project Structure

```text
rhythm-open-exchange/
├── src/                    # Core library (Rust)
├── benches/                # Criterion benchmarks
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

- [Wiki Documentation](wiki.wiki/Home.md)
- [osu!mania](https://osu.ppy.sh/wiki/en/Game_mode/osu%21mania)
- [Quaver](https://quavergame.com/)
- [Etterna](https://etternaonline.com/)
- [Quattern](https://github.com/Leinadix/Quattern/tree/main)
