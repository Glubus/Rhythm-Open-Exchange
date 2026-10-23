# Timing Points

A `TimingPoint` marks a change in tempo or scroll speed at a given time.

## Variants

```rust
pub enum TimingPoint {
    Bpm { time_us: i64, bpm: f32, signature: u8 },
    Sv  { time_us: i64, scroll_speed: f32 },
}
```

- **Bpm** — changes the tempo. `signature` is the time signature numerator (default: 4 for 4/4).
- **Sv** — changes the scroll speed multiplier without affecting timing.

## Constructors

```rust
TimingPoint::bpm(time_us, bpm)        // 4/4 time signature
TimingPoint::sv(time_us, scroll_speed)
```

## Utility Methods

```rust
tp.time_us()          // time regardless of variant
tp.is_bpm()
tp.is_sv()
tp.bpm_value()        // Some(bpm) for Bpm, None for Sv
tp.scroll_speed()     // Some(speed) for Sv, None for Bpm
```

## Validation Rules

- Timing points must be sorted by `time_us` ascending
- At least one `Bpm` point is required when the chart has notes
- The first `Bpm` point must be at or before the first note's `time_us`

The `BpmAfterFirstNote` error occurs when a beatmap has its first BPM timing
point placed after the first note. The `OsuDecodeOptions::re_arrange_bpm` flag
can repair this automatically (emits a `tracing::warn!` when applied).

## Example

```rust
use rox::model::TimingPoint;

// 180 BPM starting at time 0
let tp1 = TimingPoint::bpm(0, 180.0);

// BPM changes at 30 seconds
let tp2 = TimingPoint::bpm(30_000_000, 200.0);

// Scroll speed halved at 15 seconds
let sv = TimingPoint::sv(15_000_000, 0.5);
```
