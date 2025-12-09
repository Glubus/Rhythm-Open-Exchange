# Rhythm Open Exchange (ROX)

**ROX** is a compact binary format for **VSRG** (Vertical Scrolling Rhythm Games). It serves as a universal pivot format for converting between different rhythm game formats.

## Features

- **Compact Binary** — Uses [bincode](https://github.com/bincode-org/bincode) for minimal file size
- **Microsecond Precision** — `i64` timestamps for accurate timing
- **VSRG Focused** — Optimized for games like osu!mania, Quaver, Etterna, StepMania
- **Keysound Support** — Optional hitsounds per note for BMS/O2Jam compatibility
- **Content Hash** — BLAKE3 hash for integrity verification

## File Structure

| Field | Type | Description |
|-------|------|-------------|
| Magic | `[u8; 4]` | `ROX\0` (0x524F5800) |
| Version | `u8` | Format version (currently 1) |
| KeyCount | `u8` | Number of columns (4K, 7K, etc.) |
| Metadata | `Metadata` | Title, artist, difficulty, etc. |
| TimingPoints | `Vec<TimingPoint>` | BPM and SV changes |
| Notes | `Vec<Note>` | All notes in the chart |
| Hitsounds | `Vec<Hitsound>` | Keysound samples |

## Note Types

| Type | Description |
|------|-------------|
| `Tap` | Single tap note |
| `Hold { duration_us }` | Long note (must hold) |
| `Burst { duration_us }` | Roll note (rapid tapping) |
| `Mine` | Avoid note |

## Documentation

- [Metadata](Metadata) — Song info, difficulty, media paths
- [Timing Points](Timing-Points) — BPM and scroll velocity
- [Notes](Notes) — Tap, Hold, Burst, Mine
- [Hitsounds](Hitsounds) — Keysound system
- [Codec API](Codec-API) — Encoder/Decoder traits

## Quick Start

```rust
use rhythm_open_exchange::{RoxChart, RoxCodec, Encoder, Decoder, Note, TimingPoint};

// Create a chart
let mut chart = RoxChart::new(4); // 4K
chart.metadata.title = "My Song".into();
chart.timing_points.push(TimingPoint::bpm(0, 180.0));
chart.notes.push(Note::tap(1_000_000, 0)); // 1s at column 0

// Encode
let bytes = RoxCodec::encode(&chart)?;

// Decode
let decoded = RoxCodec::decode(&bytes)?;

// Hash
println!("{}", chart.short_hash());
```

## Supported Conversions

| Format | Import | Export | Status |
|--------|--------|--------|--------|
| osu!mania (.osu) | Planned | Planned | Not started |
| Quaver (.qua) | Planned | Planned | Not started |
| StepMania (.sm/.ssc) | Planned | Planned | Not started |
| Etterna | Planned | Planned | Not started |
| BMS (.bms/.bme/.bml) | Planned | No | Not started |
| O2Jam (.ojn/.ojm) | Planned | No | Not started |

## Project Links

- [GitHub Repository](https://github.com/your-username/Rhythm-Open-Exchange)
- [README](../README.md)

## License

MIT
