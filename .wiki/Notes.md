# Notes

Notes are the core gameplay elements in a VSRG chart. ROX supports four note types to cover all common rhythm game mechanics.

## Structure

```rust
pub struct Note {
    pub time_us: i64,
    pub column: u8,
    pub note_type: NoteType,
    pub hitsound_index: Option<u16>,
}

pub enum NoteType {
    Tap,
    Hold { duration_us: i64 },
    Burst { duration_us: i64 },
    Mine,
}
```

## Fields

| Field | Type | Description |
|-------|------|-------------|
| `time_us` | `i64` | Hit timing in microseconds |
| `column` | `u8` | Column index (0-indexed) |
| `note_type` | `NoteType` | Type of note (Tap, Hold, Burst, Mine) |
| `hitsound_index` | `Option<u16>` | Index into chart hitsounds (for keysounded notes) |

## Note Types

### Tap

Single press note. The most common note type.

```rust
use rhythm_open_exchange::Note;

let tap = Note::tap(1_000_000, 0); // At 1 second, column 0
```

### Hold (Long Note / LN)

Must be held for a duration. Release timing may be judged.

```rust
// Start at 2s, hold for 500ms, column 1
let hold = Note::hold(2_000_000, 500_000, 1);

assert!(hold.is_hold());
assert_eq!(hold.duration_us(), 500_000);
assert_eq!(hold.end_time_us(), 2_500_000);
```

### Burst (Roll)

Rapid tapping during the duration. Used in games like DDR/ITG.

```rust
// Start at 3s, roll for 300ms, column 2
let burst = Note::burst(3_000_000, 300_000, 2);

assert!(burst.is_burst());
```

### Mine

Avoid note. Hitting this note penalizes the player.

```rust
let mine = Note::mine(4_000_000, 3);

assert!(mine.is_mine());
assert_eq!(mine.duration_us(), 0); // Mines have no duration
```

## Constructor Methods

| Method | Description |
|--------|-------------|
| `Note::tap(time_us, column)` | Create a tap note |
| `Note::hold(time_us, duration_us, column)` | Create a hold note |
| `Note::burst(time_us, duration_us, column)` | Create a burst note |
| `Note::mine(time_us, column)` | Create a mine |

## Utility Methods

| Method | Return Type | Description |
|--------|-------------|-------------|
| `is_hold()` | `bool` | True if this is a hold note |
| `is_burst()` | `bool` | True if this is a burst note |
| `is_mine()` | `bool` | True if this is a mine |
| `duration_us()` | `i64` | Duration (0 for tap/mine) |
| `end_time_us()` | `i64` | Start time + duration |

## Keysounds

Notes can reference hitsounds for BMS/O2Jam style charts:

```rust
use rhythm_open_exchange::{RoxChart, Note, Hitsound};

let mut chart = RoxChart::new(7);

// Add hitsound samples
chart.hitsounds.push(Hitsound::new("piano_c4.wav"));
chart.hitsounds.push(Hitsound::new("piano_d4.wav"));

// Create notes with keysounds
let mut note1 = Note::tap(0, 0);
note1.hitsound_index = Some(0); // Links to piano_c4.wav

let mut note2 = Note::tap(500_000, 1);
note2.hitsound_index = Some(1); // Links to piano_d4.wav

chart.notes.push(note1);
chart.notes.push(note2);
```

## Column Mapping

Columns are 0-indexed. The valid range depends on `key_count`:

| Key Count | Valid Columns |
|-----------|---------------|
| 4K | 0, 1, 2, 3 |
| 5K | 0, 1, 2, 3, 4 |
| 7K | 0, 1, 2, 3, 4, 5, 6 |
| 7K+1 | 0, 1, 2, 3, 4, 5, 6, 7 |

### Validation

Charts are validated before encoding:

```rust
let mut chart = RoxChart::new(4);
chart.notes.push(Note::tap(0, 5)); // Column 5 invalid for 4K!

let result = chart.validate();
assert!(result.is_err()); // InvalidColumn error
```

## Format Mapping

### osu!mania

| osu! | ROX |
|------|-----|
| Circle | Tap |
| Slider (LN) | Hold |
| N/A | Burst |
| N/A | Mine |

osu! column calculation:
```
column = floor(x * key_count / 512)
```

### Quaver

| Quaver | ROX |
|--------|-----|
| HitObject (EndTime = 0) | Tap |
| HitObject (EndTime > 0) | Hold |
| N/A | Burst |
| N/A | Mine |

### StepMania

| SM | ROX |
|----|-----|
| 1 | Tap |
| 2/3 + 3 | Hold (head + tail) |
| 4 | Burst (roll) |
| M | Mine |

## Example: Dense Pattern

```rust
use rhythm_open_exchange::{RoxChart, Note, TimingPoint};

let mut chart = RoxChart::new(4);
chart.timing_points.push(TimingPoint::bpm(0, 200.0));

// 16th note stream (4 notes per beat at 200 BPM)
// One beat = 300ms, one 16th = 75ms = 75_000 microseconds
let spacing = 75_000i64;

for i in 0..32 {
    let time = i * spacing;
    let column = (i % 4) as u8;
    chart.notes.push(Note::tap(time, column));
}

assert_eq!(chart.note_count(), 32);
```
