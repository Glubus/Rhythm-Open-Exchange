# Format Converters

All format codecs live in the `rox-formats` crate. Each implements `Encoder`, `Decoder`, or both via the `Encoder`/`Decoder` traits from `rox`.

## Supported Formats

| Format | Extension | Decode | Encode | `no_std` | Notes |
|--------|-----------|--------|--------|----------|-------|
| osu!mania | `.osu` | ✓ | ✓ | ✓ | Mania mode only (mode=3) |
| StepMania | `.sm` | ✓ | ✓ | ✓ | |
| Quaver | `.qua` | ✓ | ✓ | — | Requires `std` (serde_yaml) |
| Friday Night Funkin' | `.json` | ✓ | ✓ | ✓ | |
| osu!taiko | `.osu` | ✓ | — | ✓ | Decode only |
| JROX (JSON ROX) | `.jrox` | ✓ | ✓ | ✓ | Debug/interchange format |
| YROX (YAML ROX) | `.yrox` | ✓ | ✓ | — | Requires `std` |
| ROX Native | `.rox` | ✓ | ✓ | partial | zstd disabled on wasm32 |

## ROX Native Format

`RoxNativeCodec` is the primary binary format:

- **Magic**: `ROX\0` (`[0x52, 0x4F, 0x58, 0x00]`) at file start
- **Serialization**: [rkyv](https://rkyv.org/) zero-copy
- **Compression**: zstd level 3 (disabled on wasm32)
- **Timestamps**: delta-encoded for better compression ratio

```rust
use rox::formats::RoxNativeCodec;
use rox::codec::{Encoder, Decoder};

let encoded = RoxNativeCodec::encode(&chart)?;
let decoded = RoxNativeCodec::decode(&encoded)?;
```

## Auto-Detect (std only)

```rust
use rox::formats::{auto_decode, auto_encode, auto_convert};

// Decode by file extension
let chart = auto_decode("song.osu")?;

// Encode by file extension
auto_encode(&chart, "song.rox")?;

// Convert directly
auto_convert("song.osu", "song.sm")?;
```

## osu! Special Options

The osu! decoder exposes `OsuDecodeOptions` for lenient decoding:

```rust
use rox::formats::{OsuDecoder, OsuDecodeOptions};

let opts = OsuDecodeOptions {
    re_arrange_bpm: true,  // shift first BPM to just before first note
};
let chart = OsuDecoder::decode_with_options(&data, &opts)?;
```

`re_arrange_bpm` silently repairs beatmaps where the first BPM timing point
appears after the first note. A `tracing::warn!` is emitted when applied.

## FNF Side Filtering

FNF charts contain notes for both player and opponent. Use `FnfSide` to filter:

```rust
use rox::formats::{FnfDecoder, FnfSide};

// Default: FnfSide::Both (8K, all notes)
let chart = FnfDecoder::decode(&data)?;

// Player only (4K)
// use FnfDecoder::decode_with_side if implemented
```

## Adding a New Format

1. Create `src/formats/<name>/` with `decoder.rs`, `encoder.rs`, `mod.rs`
2. Derive `Format` on your struct: `#[derive(Format)] #[format(extensions = ["ext"])]`
3. Implement `Decoder::decode_inner` and/or `Encoder::encode_inner`
4. Export from `src/formats/mod.rs`
5. Register in `src/formats/auto/detect.rs`
6. Add unit tests (both levels: unit + integration in `tests/integration_test.rs`)
