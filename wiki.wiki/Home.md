# Rhythm Open Exchange (ROX)

**ROX** is a universal pivot format for VSRG (Vertical Scrolling Rhythm Games). It converts between game-specific chart formats using a shared in-memory representation — `RoxChart`.

## Crates

| Crate | Purpose |
|-------|---------|
| `rox` | Core model, codec traits, validation — `no_std` compatible |
| `rox-formats` | Encoders/decoders for all supported formats |
| `rox-analysis` | Chart analysis — BPM, NPS, pattern recognition *(planned)* |
| `rox-cli` | `rox` command-line tool *(planned)* |
| `rox-macros` | `#[derive(Format)]` macro |

## Quick Start

```rust
use rox::prelude::*;
use rox::formats::{OsuDecoder, RoxNativeCodec};

// Decode an osu!mania beatmap
let data = std::fs::read("song.osu")?;
let chart = OsuDecoder::decode(&data)?;

// Re-encode as native ROX binary
let encoded = RoxNativeCodec::encode(&chart)?;
std::fs::write("song.rox", encoded)?;
```

## Convert Between Formats

```rust
use rox::codec::convert;
use rox::formats::{OsuDecoder, SmEncoder};

let osu_data = std::fs::read("song.osu")?;
let sm_data = convert::<OsuDecoder, SmEncoder>(&osu_data)?;
```

Auto-detect by file extension (requires `std`):

```rust
use rox::formats::auto_convert;

auto_convert("song.osu", "song.sm")?;
```

## Documentation

- [Notes](Notes) — Tap, Hold, Burst, Mine
- [Timing Points](Timing-Points) — BPM and scroll velocity
- [Metadata](Metadata) — Song info, difficulty, media paths
- [Hitsounds](Hitsounds) — Per-note keysound system
- [Codec API](Codec-API) — Encoder/Decoder traits and validation
- [Format Converters](Format-Converters) — Supported formats and conversion matrix
- [Performance Optimizations](Performance-Optimizations) — Benchmarks, rkyv+zstd
- [Decisions](Decisions) — Architectural decisions log
