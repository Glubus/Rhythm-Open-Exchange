# rox-core

The `rox` crate defines the canonical data model, codec traits, validation rules, and error type used across the entire workspace. It is `no_std + alloc` compatible — the `std` feature gates file I/O only.

---

## Model types

### `RoxChart`

The top-level chart structure. Every codec converts to and from this type.

```rust
pub struct RoxChart {
    pub version: u8,           // ROX schema version
    pub key_count: u8,         // number of columns (lanes)
    pub metadata: Metadata,
    pub timing_points: Vec<TimingPoint>,
    pub notes: Vec<Note>,
    pub hitsounds: Vec<Hitsound>,
}
```

### `Metadata`

```rust
pub struct Metadata {
    pub title: CompactString,
    pub artist: CompactString,
    pub creator: CompactString,
    pub difficulty_name: CompactString,
    pub audio_file: CompactString,
    pub difficulty_value: Option<f32>,   // star rating / difficulty number
    pub chart_id: Option<u64>,           // upstream ID (e.g. beatmap ID)
    pub chartset_id: Option<u64>,        // upstream set ID
    pub audio_offset_us: i64,            // global audio offset in microseconds
    pub preview_time_us: i64,            // preview start in microseconds
    pub background_file: Option<CompactString>,
    pub source: Option<CompactString>,   // game / pack origin
    pub tags: Vec<CompactString>,
    pub is_coop: bool,                   // true → chart uses two-player key layout
}
```

`CompactString` avoids a heap allocation for strings ≤ 24 bytes on 64-bit targets. Use `.as_str()` to get a `&str`.

### `TimingPoint`

```rust
pub enum TimingPoint {
    Bpm {
        time_us: i64,   // absolute time in microseconds
        bpm: f32,       // beats per minute
        signature: u8,  // time signature numerator (e.g. 4 for 4/4)
    },
    Sv {
        time_us: i64,       // absolute time in microseconds
        scroll_speed: f32,  // multiplier applied to scroll velocity
    },
}
```

All timing points must be sorted by `time_us` ascending. At least one `Bpm` point is required, and it must not come after the first note.

### `Note`

```rust
pub struct Note {
    pub time_us: i64,                   // absolute hit time in microseconds
    pub note_type: NoteType,
    pub hitsound_index: Option<u16>,    // index into RoxChart::hitsounds
    pub column: u8,                     // zero-based column index
}
```

Notes must be sorted by `time_us` ascending. No two notes in the same column may overlap.

### `NoteType`

```rust
pub enum NoteType {
    Tap,
    Hold  { duration_us: i64 },   // duration must be > 0
    Burst { duration_us: i64 },   // short hold treated as a burst; duration must be > 0
    Mine,                          // penalty note (do not hit)
}
```

---

## Codec traits

Both traits follow the **Template Method** pattern. The public entry points (`decode` / `encode`) call validation before or after the format-specific work. The validation step is not overridable so every codec is guaranteed to produce and consume valid charts.

### `Decoder`

```rust
pub trait Decoder {
    /// Format-specific parsing. Must not call validate.
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart>;

    /// Calls decode_inner then validate. Cannot be overridden.
    fn decode(data: &[u8]) -> RoxResult<RoxChart>;

    /// Reads file from disk then calls decode. Requires `std` feature.
    fn decode_from_path(path: impl AsRef<Path>) -> RoxResult<RoxChart>;
}
```

### `Encoder`

```rust
pub trait Encoder {
    /// Format-specific serialization. Must not call validate.
    fn encode_inner(chart: &RoxChart) -> RoxResult<Vec<u8>>;

    /// Calls validate then encode_inner. Cannot be overridden.
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>>;

    /// Calls encode then writes bytes to disk. Requires `std` feature.
    fn encode_to_path(chart: &RoxChart, path: impl AsRef<Path>) -> RoxResult<()>;
}
```

Why validation is non-overridable: a codec author implementing `decode_inner` should not have to think about global invariants. Centralising validation in the trait default implementation ensures every codec enforces the same rules automatically, and callers can rely on any successfully-returned `RoxChart` being valid.

---

## Validation rules

Validation runs on every `decode` call (after parsing) and every `encode` call (before serialisation). A violation returns the appropriate `RoxError` variant.

| # | Rule | Error |
|---|------|-------|
| 1 | `key_count > 0` | `InvalidFormat("key_count must be > 0")` |
| 2 | `is_coop` requires even `key_count` | `InvalidFormat("coop requires even key_count")` |
| 3 | `timing_points` sorted by `time_us` ascending | `InvalidFormat("timing points not sorted")` |
| 4 | At least one `TimingPoint::Bpm` present | `NoBpmTimingPoint` |
| 5 | First BPM point ≤ first note time | `BpmAfterFirstNote` |
| 6 | `notes` sorted by `time_us` ascending | `InvalidFormat("notes not sorted")` |
| 7 | All note columns < `key_count` | `InvalidColumn` |
| 8 | No overlapping notes per column (hold/burst end ≤ next note start) | `InvalidFormat("overlapping notes in column N")` |
| 9 | Hold/burst `duration_us > 0` | `InvalidFormat("hold/burst duration must be > 0")` |

---

## Error type

`RoxError` is the single error type used by every crate in the workspace.

```rust
pub enum RoxError {
    InvalidFormat(String),   // general format violation (includes rule descriptions above)
    InvalidColumn,           // note column ≥ key_count
    BpmAfterFirstNote,       // first BPM timing point occurs after the first note
    NoBpmTimingPoint,        // timing_points contains no Bpm variant
    Serialize(String),       // serialisation failure (encode path)
    Deserialize(String),     // deserialisation failure (decode path)
    Io(std::io::Error),      // file I/O error (std-gated)
}
```

`RoxResult<T>` is `Result<T, RoxError>`.

---

## `prelude` module

The prelude re-exports the most commonly used items:

```rust
use rox::prelude::*;
// Re-exports: RoxChart, Metadata, TimingPoint, Note, NoteType, Hitsound,
//             Decoder, Encoder, RoxError, RoxResult
```

---

## `convert` and `convert_file` utilities

Two convenience functions in `rox` handle in-memory and path-based conversion between any two codecs:

```rust
// In-memory: decode with D, encode with E
pub fn convert<D: Decoder, E: Encoder>(data: &[u8]) -> RoxResult<Vec<u8>>;

// Path-based: reads input, decodes with D, encodes with E, writes output
// Requires `std` feature
pub fn convert_file<D: Decoder, E: Encoder>(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> RoxResult<()>;
```

Example:

```rust
use rox::{convert_file};
use rox_formats::osu::OsuDecoder;
use rox_formats::rox_native::RoxNativeCodec;

convert_file::<OsuDecoder, RoxNativeCodec>("chart.osu", "chart.rox")?;
```

---

## `no_std` strategy

`rox`, the core format parsers in `rox-formats`, and `rox-analysis` are all `no_std + alloc` compatible by default.

Feature gates:

| Feature | What it unlocks |
|---------|----------------|
| `std` (default off) | `decode_from_path`, `encode_to_path`, `convert_file`, `Io` error variant |
| `std` in `rox-formats` | Quaver (serde_yaml), YROX, ROX native compression (zstd), memory-mapped I/O (memmap2) |

On WASM targets, zstd compression is bypassed and the native `.rox` format stores uncompressed rkyv bytes. All other codecs remain fully functional.
