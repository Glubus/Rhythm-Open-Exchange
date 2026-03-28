# Notes

A `Note` represents a single playfield event in a chart.

## Structure

```rust
pub struct Note {
    pub time_us: i64,                  // start time in microseconds
    pub column: u8,                    // 0-indexed column
    pub note_type: NoteType,           // Tap, Hold, Burst, or Mine
    pub hitsound_index: Option<u16>,   // index into RoxChart.hitsounds
}
```

## Note Types

```rust
pub enum NoteType {
    Tap,
    Hold  { duration_us: i64 },   // must hold for duration
    Burst { duration_us: i64 },   // rapid tapping (roll)
    Mine,                          // avoid hitting
}
```

## Constructors

```rust
Note::tap(time_us, column)
Note::hold(time_us, duration_us, column)
Note::burst(time_us, duration_us, column)
Note::mine(time_us, column)
```

All constructors set `hitsound_index = None`.

## Utility Methods

```rust
note.duration_us()    // 0 for Tap/Mine, duration for Hold/Burst
note.end_time_us()    // time_us + duration_us()
note.is_hold()
note.is_burst()
note.is_mine()
```

## Validation Rules

- Notes must be sorted by `time_us` ascending
- `column` must be `< key_count`
- No two notes on the same column may overlap (end_time of one ≤ start_time of next)
- `duration_us > 0` for Hold and Burst

## Format Mapping

| Format | Tap | Hold | Burst | Mine |
|--------|-----|------|-------|------|
| osu!mania | HitCircle | LongNote | — | — |
| Quaver | HitObject (Normal) | HitObject (LongNote) | — | — |
| StepMania | `1` | `2`/`3` | `4` | `M` |
| FNF | note section | — | — | — |
| Taiko | don/kat | — | — | — |
