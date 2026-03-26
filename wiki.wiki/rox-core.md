# rox — Core Model and Codec Traits

The `rox` crate defines the data model and codec abstractions for ROX. It has zero dependencies on any specific rhythm game format.

## Model

### RoxChart

The central type. Contains all data for a single chart.

```rust
pub struct RoxChart {
    pub version: u8,           // ROX_VERSION = 3
    pub key_count: u8,         // number of columns — structural, not metadata
    pub metadata: Metadata,
    pub timing_points: Vec<TimingPoint>,
    pub notes: Vec<Note>,
    pub hitsounds: Vec<Hitsound>,
}
```

Create with `RoxChart::new(key_count)`. `key_count` is a structural property, not metadata — it was moved out of `Metadata` in v0.7.0.

### TimingPoint

An enum with two variants (changed from struct in v0.7.0):

```rust
pub enum TimingPoint {
    Bpm { time_us: i64, bpm: f32, signature: u8 },
    Sv  { time_us: i64, scroll_speed: f32 },
}
```

Use constructors `TimingPoint::bpm(time_us, bpm)` and `TimingPoint::sv(time_us, speed)`.
Access time with `tp.time_us()`, check variant with `tp.is_bpm()` / `tp.is_sv()`.

### Note / NoteType

```rust
pub enum NoteType { Tap, Hold { duration_us: i64 }, Burst { duration_us: i64 }, Mine }
pub struct Note { pub time_us: i64, pub note_type: NoteType, pub hitsound_index: Option<u16>, pub column: u8 }
```

Constructors: `Note::tap(time_us, col)`, `Note::hold(time_us, duration_us, col)`, `Note::burst(...)`, `Note::mine(...)`.

### Hitsound

```rust
pub struct Hitsound { pub file: CompactString, pub volume: Option<u8> }
```

`volume` is clamped to `[0, 100]`. Use `Hitsound::new(file)` or `Hitsound::with_volume(file, volume)`.

### Metadata

Holds chart title, artist, difficulty info, and coop flag. Does **not** include `key_count` — that lives on `RoxChart`.

## Validation

`chart.validate()` runs all validators in order. Called automatically by `Encoder::encode()` and `Decoder::decode()`.

Validators in order:
1. `validate_metadata` — `key_count > 0`, coop requires even key count
2. `validate_timing_points` — sorted by time, at least one BPM point if notes exist, BPM point before first note
3. `validate_notes_sorted` — all notes globally sorted by `time_us`
4. `validate_note_columns` — all columns within `[0, key_count)`
5. `validate_note_overlaps` — no two notes on the same column overlap in time
6. `validate_note_durations` — hold/burst notes must have `duration_us > 0`

## Codec Traits

### Template Method Pattern

```rust
pub trait Encoder {
    fn encode_inner(chart: &RoxChart) -> RoxResult<Vec<u8>>;
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>> { /* validate + encode_inner */ }
}

pub trait Decoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart>;
    fn decode(data: &[u8]) -> RoxResult<RoxChart> { /* decode_inner + validate */ }
}
```

Implement `encode_inner` / `decode_inner`. Never override `encode` / `decode`.

### Format Trait

```rust
pub trait Format {
    const EXTENSIONS: &'static [&'static str];
    fn supports_extension(ext: &str) -> bool; // case-insensitive
}
```

Use `#[derive(rox_macros::Format)]` with `#[format(extensions = ["ext"])]`.

### Convert Utilities

```rust
pub fn convert<D: Decoder, E: Encoder>(data: &[u8]) -> RoxResult<Vec<u8>>
pub fn convert_file<D: Decoder, E: Encoder>(input: impl AsRef<Path>, output: impl AsRef<Path>) -> RoxResult<()>
```

`convert_file` is `std`-gated.

## `no_std` Support

`rox` supports `no_std + alloc`. Disable default features and enable `alloc`:

```toml
rox = { version = "0.7", default-features = false, features = ["alloc"] }
```

File I/O helpers (`decode_from_path`, `encode_to_path`, `convert_file`) require the `std` feature.
