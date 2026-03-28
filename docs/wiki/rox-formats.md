# rox-formats

The `rox-formats` crate provides decoders and encoders for every supported rhythm-game chart format. All parsers target `no_std + alloc` by default; the `std` feature enables YAML support, file I/O helpers, and native compression.

---

## Format overview

| Format | Extension | Decoder | Encoder | `std` required |
|--------|-----------|---------|---------|----------------|
| osu!mania | `.osu` | `OsuDecoder` | `OsuEncoder` | No |
| osu!taiko | `.osu` | `TaikoDecoder` | — | No |
| StepMania | `.sm` | `SmDecoder` | `SmEncoder` | No |
| Quaver | `.qua` | `QuaDecoder` | `QuaEncoder` | Yes (serde_yaml) |
| Friday Night Funkin' | `.json` | `FnfDecoder` | `FnfEncoder` | No |
| JROX (JSON ROX) | `.jrox` | `JroxDecoder` | `JroxEncoder` | No |
| YROX (YAML ROX) | `.yrox` | `YroxDecoder` | `YroxEncoder` | Yes (serde_yaml) |
| ROX native | `.rox` | `RoxNativeCodec` | `RoxNativeCodec` | No (zstd needs `std`) |

---

## Format details

### osu!mania

Standard osu!mania `.osu` files. The decoder handles mania-specific sections (`[HitObjects]`, `[TimingPoints]`, `[Metadata]`, `[General]`).

```rust
use rox_formats::osu::{OsuDecoder, OsuEncoder, OsuDecodeOptions};
use rox::traits::{Decoder, Encoder};

// Standard decode
let chart = OsuDecoder::decode(&data)?;

// Decode with options
let opts = OsuDecodeOptions { re_arrange_bpm: true };
let chart = OsuDecoder::decode_with_options(&data, &opts)?;

// Encode
let bytes = OsuEncoder::encode(&chart)?;
```

#### `OsuDecodeOptions`

```rust
pub struct OsuDecodeOptions {
    pub re_arrange_bpm: bool,
}
```

`re_arrange_bpm`: some osu! files place the first BPM timing point slightly after the first hit object, which violates ROX validation rule 5. When `true`, the decoder shifts the first BPM point to `first_note_time - 1µs` to satisfy the constraint and emits a `tracing::warn!` noting the adjustment. Default is `false` — the decode will return `BpmAfterFirstNote` without the option.

### osu!taiko

Decode-only. Taiko charts are mapped to a 4-column mania layout:

| Taiko input | Column |
|-------------|--------|
| Don (small) | 0 |
| Kat (small) | 1 |
| Don (large) | 2 |
| Kat (large) | 3 |

```rust
use rox_formats::taiko::TaikoDecoder;
use rox::traits::Decoder;

let chart = TaikoDecoder::decode(&data)?;
assert_eq!(chart.key_count, 4);
```

No encoder is provided — taiko output is out of scope.

### StepMania

`.sm` text format (StepMania 3/5 `.sm` files, not `.ssc`).

```rust
use rox_formats::sm::{SmDecoder, SmEncoder};
use rox::traits::{Decoder, Encoder};

let chart = SmDecoder::decode(&data)?;
let bytes = SmEncoder::encode(&chart)?;
```

Quirks:
- Only `dance-single` (4K) and `dance-double` (8K) step types are decoded.
- BPM is read from the `#BPMS` tag; stops are converted to SV points.
- Metadata fields map from `#TITLE`, `#ARTIST`, `#CREDIT`, `#DIFFICULTY`, `#METER`.

### Quaver

`.qua` YAML files. Requires `std` feature (uses `serde_yaml`).

```rust
use rox_formats::qua::{QuaDecoder, QuaEncoder};
use rox::traits::{Decoder, Encoder};

let chart = QuaDecoder::decode(&data)?;
let bytes = QuaEncoder::encode(&chart)?;
```

Quirks:
- SVs are decoded as `TimingPoint::Sv` entries.
- `TimingPointsOnly` and `Normal` difficulties are both supported.
- Hitsound samples from the `.qua` `HitSounds` field map to `hitsound_index`.

### Friday Night Funkin'

`.json` files. FNF charts encode notes per-section with `mustHitSection` controlling which side plays which notes.

```rust
use rox_formats::fnf::{FnfDecoder, FnfEncoder, FnfSide};
use rox::traits::{Decoder, Encoder};

// Default: Player side only
let chart = FnfDecoder::decode(&data)?;

// Specify side explicitly
let chart = FnfDecoder::decode_with_side(&data, FnfSide::Opponent)?;
let chart = FnfDecoder::decode_with_side(&data, FnfSide::Both)?;

let bytes = FnfEncoder::encode(&chart)?;
```

#### `FnfSide`

```rust
pub enum FnfSide {
    Player,    // columns 0–3 (default)
    Opponent,  // columns 4–7 remapped to 0–3
    Both,      // all 8 columns; sets key_count = 8 and metadata.is_coop = true
}
```

When `FnfSide::Both` is used the chart will have `key_count = 8` and `is_coop = true`.

### JROX (JSON ROX)

A JSON serialisation of the `RoxChart` model. Human-readable, suitable for debugging or tooling that cannot use binary formats. `no_std` compatible.

```rust
use rox_formats::jrox::{JroxDecoder, JroxEncoder};
use rox::traits::{Decoder, Encoder};

let chart = JroxDecoder::decode(&data)?;
let bytes = JroxEncoder::encode(&chart)?;
```

### YROX (YAML ROX)

A YAML serialisation of the `RoxChart` model. Requires `std` feature.

```rust
use rox_formats::yrox::{YroxDecoder, YroxEncoder};
use rox::traits::{Decoder, Encoder};

let chart = YroxDecoder::decode(&data)?;
let bytes = YroxEncoder::encode(&chart)?;
```

### ROX native (`.rox`)

The canonical binary format for long-term storage and fast loading. A single type `RoxNativeCodec` implements both `Decoder` and `Encoder`.

```rust
use rox_formats::rox_native::RoxNativeCodec;
use rox::traits::{Decoder, Encoder};

let chart = RoxNativeCodec::decode(&data)?;
let bytes = RoxNativeCodec::encode(&chart)?;
```

#### Internals

| Property | Value |
|----------|-------|
| Magic bytes | `ROX\0` (4 bytes at offset 0) |
| Serialisation | [rkyv](https://github.com/rkyv/rkyv) zero-copy binary |
| Compression | zstd level 3 (passthrough on WASM) |
| Timestamp encoding | Delta-encoded note timestamps for better compression ratio |
| Size limit | 100 MB enforced on decode |
| Schema version constant | `ROX_VERSION = 3` |

The `ROX_VERSION = 3` bump is a breaking change from v2 — it reflects the migration of `TimingPoint` from a struct to an enum. Files from v2 will fail to decode.

Delta encoding: before compression, note `time_us` values are stored as differences from the previous note rather than absolute values. This dramatically reduces entropy for patterns with consistent timing intervals.

---

## Benchmark results

Measured on test assets with 50 000 notes:

| Format | Decode | Encode | File size (50K notes) |
|--------|--------|--------|-----------------------|
| rox (native) | ~900 µs | ~1.9 ms | 65 KB |
| osu (text) | ~7.1 ms | ~2.0 ms | 1 532 KB |
| jrox (JSON) | ~7.8 ms | ~6.3 ms | 7 633 KB |
| sm | ~107 µs | ~183 µs | — |
| qua (YAML) | ~1.95 ms | ~1.0 ms | — |
| fnf | ~2.6 µs | ~1.3 µs | — |

---

## Auto dispatch (std-gated)

When the `std` feature is enabled, three top-level functions select the correct codec by file extension:

```rust
use rox_formats::auto::{auto_decode, auto_encode, auto_convert};
use std::path::Path;

// Decode: extension → decoder
let chart = auto_decode(Path::new("chart.osu"))?;

// Encode: extension → encoder
auto_encode(&chart, Path::new("chart.rox"))?;

// Convert: read, decode, encode, write in one call
auto_convert(Path::new("chart.sm"), Path::new("chart.rox"))?;
```

Unknown extensions return `RoxError::InvalidFormat`.

---

## Adding a new format

See [Adding a Format](Adding-a-Format.md) for the complete step-by-step guide, including file layout, trait implementation, testing strategy, and documentation requirements.
