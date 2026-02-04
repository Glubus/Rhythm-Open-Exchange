# Timing Points

Timing points define BPM (tempo) and scroll velocity changes throughout a chart.

## Structure

```rust
pub struct TimingPoint {
    pub time_us: i64,
    pub bpm: f32,
    pub signature: u8,
    pub is_inherited: bool,
    pub scroll_speed: f32,
}
```

## Fields

| Field | Type | Description |
|-------|------|-------------|
| `time_us` | `i64` | Position in microseconds |
| `bpm` | `f32` | Beats per minute (0 for inherited points) |
| `signature` | `u8` | Time signature numerator (default: 4) |
| `is_inherited` | `bool` | True if this is a scroll velocity change |
| `scroll_speed` | `f32` | Scroll multiplier (1.0 = normal) |

## Types of Timing Points

### BPM Changes (Uninherited)

BPM timing points define the tempo of the song. They affect:
- Beat snapping in editors
- Measure lines
- Metronome timing

```rust
use rhythm_open_exchange::TimingPoint;

// 180 BPM starting at 0ms
let bpm_point = TimingPoint::bpm(0, 180.0);

assert_eq!(bpm_point.is_inherited, false);
assert_eq!(bpm_point.bpm, 180.0);
assert_eq!(bpm_point.scroll_speed, 1.0);
```

### Scroll Velocity (Inherited/SV)

SV points change how fast notes scroll without affecting the BPM:

```rust
// At 30 seconds, slow down to 50% speed
let sv_slow = TimingPoint::sv(30_000_000, 0.5);

// At 60 seconds, speed up to 150%
let sv_fast = TimingPoint::sv(60_000_000, 1.5);

assert!(sv_slow.is_inherited);
assert_eq!(sv_slow.scroll_speed, 0.5);
```

## Common SV Values

| Value | Effect |
|-------|--------|
| 0.25 | Quarter speed (very slow) |
| 0.5 | Half speed |
| 1.0 | Normal speed |
| 1.5 | 150% speed |
| 2.0 | Double speed |
| 3.0+ | Very fast (typically for emphasis) |

## Time Signatures

The `signature` field represents the numerator of the time signature (beats per measure):

| Value | Time Signature |
|-------|---------------|
| 3 | 3/4 (waltz) |
| 4 | 4/4 (common time) |
| 5 | 5/4 |
| 6 | 6/4 |
| 7 | 7/4 |

The denominator is always assumed to be 4.

## Example Usage

```rust
use rhythm_open_exchange::{RoxChart, TimingPoint};

let mut chart = RoxChart::new(4);

// Initial BPM
chart.timing_points.push(TimingPoint::bpm(0, 160.0));

// Slow section at 30s
chart.timing_points.push(TimingPoint::sv(30_000_000, 0.6));

// BPM change at 60s
chart.timing_points.push(TimingPoint::bpm(60_000_000, 180.0));

// Speed up at 90s
chart.timing_points.push(TimingPoint::sv(90_000_000, 1.2));

// Final chorus at 120s - double speed
chart.timing_points.push(TimingPoint::sv(120_000_000, 2.0));
```

## Format Mapping

### osu!mania

In osu!, timing points use milliseconds and beat length:

```
[TimingPoints]
0,333.33,4,1,0,100,1,0      // Uninherited: 180 BPM
30000,-66.67,4,1,0,100,0,0  // Inherited: 1.5x SV
```

Conversion:
- `msPerBeat > 0` → Uninherited (BPM = 60000 / msPerBeat)
- `msPerBeat < 0` → Inherited (SV = -100 / msPerBeat)

### Quaver

Quaver uses explicit BPM values and SV multipliers:

```yaml
TimingPoints:
  - StartTime: 0
    Bpm: 180

SliderVelocities:
  - StartTime: 30000
    Multiplier: 1.5
```

## Diagram

```
Time (seconds)
├─────────────────────────────────────────────────────────────────►
│
│  BPM=160      SV=0.6      BPM=180      SV=1.2       SV=2.0
│    ▼            ▼           ▼            ▼            ▼
│    │            │           │            │            │
├────┼────────────┼───────────┼────────────┼────────────┼───────►
0s              30s         60s          90s         120s
```
