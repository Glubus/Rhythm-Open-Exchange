# rox-analysis

> **Status: planned for v0.7.0 — not yet implemented.**

The `rox-analysis` crate will expose a `RoxAnalysis` extension trait on `RoxChart` providing BPM statistics, notes-per-second metrics, cryptographic hashes, and pattern analysis. The crate targets `no_std + alloc` by default.

---

## `RoxAnalysis` trait

```rust
pub trait RoxAnalysis {
    // BPM
    fn bpm_min(&self) -> f64;
    fn bpm_max(&self) -> f64;
    fn bpm_mode(&self) -> f64;

    // NPS
    fn nps(&self) -> f64;
    fn density(&self, segments: usize) -> Vec<f64>;
    fn highest_nps(&self, window_s: f64) -> f64;
    fn lowest_nps(&self, window_s: f64) -> f64;
    fn highest_drain_time(&self) -> f64;

    // Hashing
    fn hash(&self) -> String;
    fn notes_hash(&self) -> String;
    fn timings_hash(&self) -> String;
    fn short_hash(&self) -> String;

    // Patterns
    fn polyphony(&self) -> HashMap<u32, u32>;
    fn lane_balance(&self) -> Vec<u32>;
    fn pattern_analysis(&self) -> AnalysisResult;
}
```

---

## BPM

| Method | Description |
|--------|-------------|
| `bpm_min()` | Lowest BPM value across all `TimingPoint::Bpm` entries |
| `bpm_max()` | Highest BPM value across all `TimingPoint::Bpm` entries |
| `bpm_mode()` | BPM weighted by the duration it is active |

**Mode computation:** each BPM segment's weight is `segment_duration / total_chart_duration`. The BPM with the highest cumulative weight is returned. In charts with two BPMs of equal duration, the lower BPM wins as a tiebreaker (stable sort by bpm ascending before aggregation).

---

## NPS (notes per second)

| Method | Description |
|--------|-------------|
| `nps()` | Average NPS over the full chart duration |
| `density(segments)` | Splits chart duration into N equal time windows; returns average NPS per window |
| `highest_nps(window_s)` | Sliding window of `window_s` seconds; returns peak NPS found |
| `lowest_nps(window_s)` | Sliding window of `window_s` seconds; returns lowest non-zero NPS |
| `highest_drain_time()` | Length in seconds of the longest continuous window with at least one note per second |

**Sliding window:** the window advances by one note at a time rather than by fixed time steps. This makes the result independent of time resolution and is O(n) with a two-pointer approach.

---

## Hashing

All hashes use [BLAKE3](https://github.com/BLAKE3-team/BLAKE3). Inputs are serialised as little-endian bytes in a deterministic order before hashing.

| Method | Input |
|--------|-------|
| `hash()` | Full chart: metadata + timing points + notes |
| `notes_hash()` | Only `notes` (time_us, note_type, column) — ignores hitsound indices |
| `timings_hash()` | Only `timing_points` (time_us, bpm/sv value) |
| `short_hash()` | First 16 hex characters of `hash()` — suitable for display |

Hitsound data is excluded from all hashes so that re-skinned charts with identical note patterns produce the same `notes_hash`.

---

## Patterns

### `polyphony() -> HashMap<u32, u32>`

Returns a map from chord size to occurrence count.

- Chord size 1 → single notes hit simultaneously with no other note in that timestamp.
- Chord size N → N notes sharing the same `time_us`.

Example: `{ 1: 800, 2: 120, 4: 30 }` means 800 single notes, 120 jumps, 30 hands/quads.

### `lane_balance() -> Vec<u32>`

Returns a vector of length `key_count` where each element is the total number of notes in that column. Useful for detecting imbalanced charts.

---

## Advanced pattern analysis

```rust
pub fn pattern_analysis(&self) -> AnalysisResult;
```

`AnalysisResult` will be a structured report including (exact fields TBD during implementation):

- Stream detection (consecutive 1/16 or faster runs of single notes)
- Jack detection (repeated notes in the same column)
- Roll detection (alternating columns)
- Chord density over time
- Estimated difficulty category

The exact `AnalysisResult` structure will be stabilised when the crate is implemented. Until then, treat this API as provisional.
