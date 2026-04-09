# rox-analysis + CLI redesign (v0.7.0)

## Context

Rewrite branch `rewrite/v0.7.0`. The `rox-analysis` crate and `rox-cli` binary are empty stubs.
Reference implementation lives on `origin/main` (v0.6.2) in `src/analysis/` and `src/bin/rox.rs`.

## Decisions

- **No pattern recognition** — `pattern_recognition/` from main is excluded. It is opinionated,
  expensive, and has no place in a general-purpose interchange library.
- **xxHash (xxh3) replaces BLAKE3** — chart hashing is purely for deduplication/identification,
  not adversarial threat models. xxh3 128-bit is faster, lighter, and honest about the use case.
- **Dependency stack** — `rox-analysis` adds only `xxhash-rust`. `rkyv` and `zstd` are already
  present in the workspace via `rox-formats`.
- **CLI moves to `apps/rox-cli/`** — separates application binaries from library crates.

## `rox-analysis` crate

### Modules

| Module    | Functions                                               |
|-----------|---------------------------------------------------------|
| `bpm`     | `bpm_min`, `bpm_max`, `bpm_mode`                        |
| `nps`     | `nps`, `density`, `highest_nps`, `lowest_nps`, `highest_drain_time` |
| `pattern` | `polyphony`, `lane_balance`                             |
| `hash`    | `hash`, `notes_hash`, `timings_hash`, `short_hash`      |

### Trait

```rust
pub trait RoxAnalysis {
    fn bpm_min(&self) -> f64;
    fn bpm_max(&self) -> f64;
    fn bpm_mode(&self) -> f64;

    fn nps(&self) -> f64;
    fn density(&self, segments: usize) -> Vec<f64>;
    fn highest_nps(&self, window_s: f64) -> f64;
    fn lowest_nps(&self, window_s: f64) -> f64;
    fn highest_drain_time(&self) -> f64;

    fn polyphony(&self) -> HashMap<u32, u32>;
    fn lane_balance(&self) -> Vec<u32>;

    fn hash(&self) -> String;
    fn notes_hash(&self) -> String;
    fn timings_hash(&self) -> String;
    fn short_hash(&self) -> String;
}
```

Implemented on `RoxChart`. Exported from `rox_analysis::prelude`.

### Hash implementation

Serialize chart with `rkyv`, feed bytes into `xxh3_128`, return lowercase hex string.
`short_hash` returns first 16 hex chars (64-bit prefix).

### Dependencies

```toml
[dependencies]
rox.workspace = true
rkyv = { version = "0.8", default-features = false, features = ["alloc"] }
xxhash-rust = { version = "0.8", features = ["xxh3"] }
```

## `apps/rox-cli/` binary

### Commands

| Command                   | Description                        |
|---------------------------|------------------------------------|
| `rox convert <in> <out>`  | Decode input, encode output        |
| `rox info <file>`         | Print metadata + analysis stats    |
| `rox validate <file>`     | Validate chart structure           |
| `rox version`             | Print version                      |
| `rox help`                | Print help                         |

`info` output sections: Metadata, Statistics, Hashes, Analysis (BPM, NPS, drain, polyphony, lane balance).
No `-aa` / advanced analysis flag.

### Dependencies

```toml
[dependencies]
rox.workspace = true
rox-formats.workspace = true
rox-analysis.workspace = true
```

## Workspace changes

- Add `apps/rox-cli` to `[workspace.members]`
- Remove `crates/cli` from `[workspace.members]` (keep the crate stub or delete it)
- Add `rox-analysis` to `[workspace.dependencies]`
