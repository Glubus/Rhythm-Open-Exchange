# Rhythm Open Exchange (ROX) — v0.7.0

ROX is a Rust workspace providing a unified model and codec layer for rhythm-game chart formats. It targets `no_std + alloc` environments where possible and exposes a single canonical data model (`RoxChart`) that all format codecs convert to and from.

---

## Workspace structure

| Crate | Path | Purpose |
|-------|------|---------|
| `rox` | `crates/rox` | Core model + codec traits (no_std) |
| `rox-macros` | `crates/macros` | `#[derive(Format)]` proc-macro |
| `rox-formats` | `crates/formats` | All format parsers and encoders |
| `rox-analysis` | `crates/analysis` | BPM, NPS, hash, pattern analysis _(planned)_ |
| `rox-cli` | `crates/cli` | `rox` binary _(planned)_ |
| `rox-uniffi` | `bindings/uniffi` | UniFFI bindings (out of scope) |
| `rox-test-utils` | `crates/test-utils` | Dev-only test fixtures |

---

## Quick start

### Add to `Cargo.toml`

```toml
[dependencies]
rox = "0.7"
rox-formats = "0.7"
```

Enable file I/O and YAML-based formats:

```toml
rox-formats = { version = "0.7", features = ["std"] }
```

### Decode a file

```rust
use rox_formats::osu::OsuDecoder;
use rox::traits::Decoder;

let data = std::fs::read("chart.osu")?;
let chart = OsuDecoder::decode(&data)?;
println!("{} — {}", chart.metadata.artist, chart.metadata.title);
```

### Encode a chart

```rust
use rox_formats::rox_native::RoxNativeCodec;
use rox::traits::Encoder;

let bytes = RoxNativeCodec::encode(&chart)?;
std::fs::write("chart.rox", bytes)?;
```

### Convert between formats (std-gated)

```rust
use rox_formats::auto::auto_convert;
use std::path::Path;

auto_convert(Path::new("chart.osu"), Path::new("chart.rox"))?;
```

Or from file paths using the decode/encode path helpers:

```rust
use rox_formats::osu::OsuDecoder;
use rox_formats::rox_native::RoxNativeCodec;
use rox::traits::{Decoder, Encoder};

let chart = OsuDecoder::decode_from_path("chart.osu")?;
RoxNativeCodec::encode_to_path(&chart, "chart.rox")?;
```

---

## Further reading

| Page | Contents |
|------|---------|
| [rox-core](rox-core.md) | Model types, codec traits, validation, error handling, no_std strategy |
| [rox-formats](rox-formats.md) | All formats, API details, quirks, benchmarks, auto dispatch |
| [rox-analysis](rox-analysis.md) | BPM/NPS/hash/pattern analysis API (planned) |
| [rox-cli](rox-cli.md) | `rox` CLI commands and flags (planned) |
| [Adding a Format](Adding-a-Format.md) | Step-by-step guide for contributing a new format codec |
