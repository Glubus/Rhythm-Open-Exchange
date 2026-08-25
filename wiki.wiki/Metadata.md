# Metadata

`Metadata` holds all chart information that is not gameplay data.

## Structure

```rust
pub struct Metadata {
    pub chart_id: Option<u64>,
    pub chartset_id: Option<u64>,
    pub title: CompactString,
    pub artist: CompactString,
    pub creator: CompactString,
    pub difficulty_name: CompactString,
    pub difficulty_value: Option<f32>,      // OD, overall difficulty rating, etc.
    pub audio_file: CompactString,          // relative path
    pub background_file: Option<CompactString>,
    pub audio_offset_us: i64,              // global audio offset in µs
    pub preview_time_us: i64,              // preview start in µs
    pub preview_duration_us: i64,          // preview length (default: 15s)
    pub source: Option<CompactString>,     // game or album of origin
    pub genre: Option<CompactString>,
    pub language: Option<CompactString>,
    pub tags: Vec<CompactString>,
    pub is_coop: bool,                     // requires even key_count
}
```

## Defaults

| Field | Default |
|-------|---------|
| `difficulty_name` | `"Normal"` |
| `preview_duration_us` | `15_000_000` (15s) |
| `audio_offset_us` | `0` |
| `is_coop` | `false` |
| All optional fields | `None` / empty |

## Notes

- Strings use `CompactString` — inline storage for strings ≤ 24 bytes, heap otherwise. Implements `Into<CompactString>` from `&str` and `String`.
- `key_count` is on `RoxChart`, not `Metadata`, because it is a gameplay property.
- `is_coop` splits the key columns between two players (e.g. 8K → 4K each). The validator enforces even `key_count` when `is_coop` is true.

## Example

```rust
use rox::model::Metadata;

let mut meta = Metadata::default();
meta.title = "Night of Knights".into();
meta.artist = "xi".into();
meta.creator = "Kawawa".into();
meta.difficulty_name = "INFINITE".into();
meta.difficulty_value = Some(9.8);
meta.audio_file = "nightofknights.mp3".into();
meta.preview_time_us = 90_000_000; // 90 seconds
```
