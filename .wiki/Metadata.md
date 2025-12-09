# Metadata

The `Metadata` struct contains all descriptive information about a chart and its associated media files.

## Fields

### Song Information

| Field | Type | Description |
|-------|------|-------------|
| `title` | `String` | Song title |
| `artist` | `String` | Song artist or composer |
| `creator` | `String` | Chart creator/mapper name |

### Difficulty

| Field | Type | Description |
|-------|------|-------------|
| `difficulty_name` | `String` | Named difficulty (e.g., "Easy", "Hard", "GRAVITY") |
| `difficulty_value` | `Option<f32>` | Numeric difficulty rating (format-dependent) |

### Media Files

| Field | Type | Description |
|-------|------|-------------|
| `audio_file` | `String` | Relative path to the audio file |
| `background_file` | `Option<String>` | Relative path to background image |

### Audio Timing

| Field | Type | Description |
|-------|------|-------------|
| `audio_offset_us` | `i64` | Global audio offset in microseconds |
| `preview_time_us` | `i64` | Song preview start time in microseconds |
| `preview_duration_us` | `i64` | Preview duration (default: 15 seconds) |

### Categorization

| Field | Type | Description |
|-------|------|-------------|
| `source` | `Option<String>` | Source (anime, game, original, etc.) |
| `genre` | `Option<String>` | Music genre (electronic, rock, etc.) |
| `language` | `Option<String>` | Language code (JP, EN, KR, etc.) |
| `tags` | `Vec<String>` | Search/categorization tags |

## Default Values

```rust
Metadata {
    title: "",
    artist: "",
    creator: "",
    difficulty_name: "Normal",
    difficulty_value: None,
    audio_file: "",
    background_file: None,
    audio_offset_us: 0,
    preview_time_us: 0,
    preview_duration_us: 15_000_000, // 15 seconds
    source: None,
    genre: None,
    language: None,
    tags: [],
}
```

## Example

```rust
use rhythm_open_exchange::Metadata;

let metadata = Metadata {
    title: "Galaxy Collapse".into(),
    artist: "Kurokotei".into(),
    creator: "Shoegazer".into(),
    difficulty_name: "Cataclysmic Hypernova".into(),
    difficulty_value: Some(9.99),
    audio_file: "audio.ogg".into(),
    background_file: Some("bg.jpg".into()),
    audio_offset_us: -5000, // -5ms offset
    preview_time_us: 60_000_000, // 60 seconds
    preview_duration_us: 20_000_000, // 20 seconds
    source: Some("BMS".into()),
    genre: Some("Speedcore".into()),
    language: Some("JP".into()),
    tags: vec!["marathon".into(), "stream".into()],
};
```

## Format Mapping

### From osu!mania (.osu)

| osu! | ROX |
|------|-----|
| Title | `title` |
| Artist | `artist` |
| Creator | `creator` |
| Version | `difficulty_name` |
| OverallDifficulty | `difficulty_value` |
| AudioFilename | `audio_file` |
| Background event | `background_file` |
| PreviewTime | `preview_time_us * 1000` |
| Source | `source` |
| Tags | `tags` (split by space) |

### From Quaver (.qua)

| Quaver | ROX |
|--------|-----|
| Title | `title` |
| Artist | `artist` |
| Creator | `creator` |
| DifficultyName | `difficulty_name` |
| AudioFile | `audio_file` |
| BackgroundFile | `background_file` |
| SongPreviewTime | `preview_time_us * 1000` |
| Source | `source` |
| Tags | `tags` |
| Genre | `genre` |
