# rox-analysis + CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `rox-analysis` with BPM/NPS/pattern/hash stats, and move the CLI to `apps/rox-cli/` with `convert`, `info`, `validate`, `version`, `help` commands.

**Architecture:** `rox-analysis` exposes a `RoxAnalysis` trait implemented on `RoxChart` with four focused modules (bpm, nps, pattern, hash). The CLI at `apps/rox-cli/` depends on `rox`, `rox-formats`, and `rox-analysis` — no pattern recognition, no heavy deps.

**Tech Stack:** Rust, `xxhash-rust` (xxh3 128-bit), `rkyv` 0.8 (for hash serialization, already in workspace), `rox-formats` (`auto_decode`, `auto_encode`).

---

## Model Reference

Key types (already in `rox` crate):

```rust
// RoxChart fields
chart.key_count: u8        // field, not method
chart.metadata: Metadata
chart.timing_points: Vec<TimingPoint>
chart.notes: Vec<Note>
chart.hitsounds: Vec<Hitsound>
chart.duration_us() -> i64
chart.note_count() -> usize
chart.validate() -> RoxResult<()>

// TimingPoint (enum, NOT a struct with is_inherited)
TimingPoint::Bpm { time_us, bpm, signature }
TimingPoint::Sv  { time_us, scroll_speed }
tp.time_us() -> i64
tp.is_bpm() -> bool
tp.bpm_value() -> Option<f32>

// Note
note.time_us: i64
note.column: u8
note.hitsound_index: Option<u16>
note.end_time_us() -> i64
note.duration_us() -> i64
Note::tap(time_us, column)
Note::hold(time_us, duration_us, column)
```

---

## File Map

**Created:**
- `apps/rox-cli/Cargo.toml`
- `apps/rox-cli/src/main.rs`
- `crates/analysis/src/bpm.rs`
- `crates/analysis/src/nps.rs`
- `crates/analysis/src/pattern.rs`
- `crates/analysis/src/hash.rs`

**Modified:**
- `Cargo.toml` (workspace members + deps)
- `crates/analysis/Cargo.toml`
- `crates/analysis/src/lib.rs`

**Removed from workspace (keep files, just remove from members):**
- `crates/cli` → replaced by `apps/rox-cli`

---

## Task 1: Workspace setup

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/analysis/Cargo.toml`
- Create: `apps/rox-cli/Cargo.toml`

- [ ] **Step 1: Update workspace `Cargo.toml`**

Replace the `[workspace]` members and add `rox-analysis` to workspace deps:

```toml
[workspace]
members = [
    "crates/rox",
    "crates/macros",
    "crates/formats",
    "crates/analysis",
    "crates/uniffi",
    "crates/test-utils",
    "apps/rox-cli",
]
resolver = "2"

[workspace.package]
version = "0.7.0"
edition = "2024"
license = "MIT"
repository = "https://github.com/Glubus/rhythm-open-exchange"

[workspace.dependencies]
rstest = "0.26.1"
rox = { path = "crates/rox" }
rox-macros = { path = "crates/macros" }
rox-formats = { path = "crates/formats" }
rox-analysis = { path = "crates/analysis" }
rox-test-utils = { path = "crates/test-utils" }
```

Note: `crates/cli` is removed from members (keep the directory, just not in workspace).

- [ ] **Step 2: Update `crates/analysis/Cargo.toml`**

```toml
[package]
name = "rox-analysis"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "ROX chart analysis — BPM, NPS, pattern stats, and fast hashing"

[lib]
crate-type = ["rlib"]

[dependencies]
rox = { workspace = true }
rkyv = { version = "0.8", default-features = false, features = ["alloc"] }
xxhash-rust = { version = "0.8", features = ["xxh3"] }

[dev-dependencies]
rstest.workspace = true
rox-test-utils.workspace = true
```

- [ ] **Step 3: Create `apps/rox-cli/Cargo.toml`**

```toml
[package]
name = "rox-cli"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "ROX CLI — convert, inspect, and validate VSRG chart files"

[[bin]]
name = "rox"
path = "src/main.rs"

[dependencies]
rox = { workspace = true }
rox-formats = { workspace = true }
rox-analysis = { workspace = true }
```

- [ ] **Step 4: Verify workspace compiles**

```bash
cargo check --workspace
```

Expected: no errors (analysis and cli are empty stubs, that's fine).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/analysis/Cargo.toml apps/rox-cli/Cargo.toml
git commit -m "chore: add apps/rox-cli, wire rox-analysis deps, remove crates/cli from workspace"
```

---

## Task 2: `rox-analysis` — hash module

**Files:**
- Create: `crates/analysis/src/hash.rs`

xxh3_128 produces a `u128`. Serialize with `rkyv::to_bytes`, feed into `xxh3_128`, format as 32-char lowercase hex. `short_hash` = first 16 chars.

- [ ] **Step 1: Write the failing tests in `crates/analysis/src/hash.rs`**

```rust
use rox::model::RoxChart;

/// Compute xxh3-128 hash of the full chart (rkyv-serialized).
pub fn hash(chart: &RoxChart) -> String {
    todo!()
}

/// Compute xxh3-128 hash of notes only.
pub fn notes_hash(chart: &RoxChart) -> String {
    todo!()
}

/// Compute xxh3-128 hash of timing points only.
pub fn timings_hash(chart: &RoxChart) -> String {
    todo!()
}

/// First 16 hex chars of the full hash (64-bit prefix).
pub fn short_hash(chart: &RoxChart) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::model::{Note, TimingPoint};
    use rstest::{fixture, rstest};

    #[fixture]
    fn chart_with_notes() -> RoxChart {
        let mut chart = RoxChart::new(4);
        chart.notes.push(Note::tap(1_000_000, 0));
        chart.notes.push(Note::tap(2_000_000, 1));
        chart.notes.push(Note::hold(3_000_000, 500_000, 2));
        chart
    }

    #[rstest]
    fn test_hash_returns_32_hex_chars(chart_with_notes: RoxChart) {
        let h = hash(&chart_with_notes);
        assert_eq!(h.len(), 32);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[rstest]
    fn test_hash_is_deterministic(chart_with_notes: RoxChart) {
        assert_eq!(hash(&chart_with_notes), hash(&chart_with_notes));
    }

    #[rstest]
    fn test_hash_changes_with_content(chart_with_notes: RoxChart) {
        let mut other = chart_with_notes.clone();
        other.notes.push(Note::tap(9_000_000, 3));
        assert_ne!(hash(&chart_with_notes), hash(&other));
    }

    #[rstest]
    fn test_notes_hash_ignores_timing_points(chart_with_notes: RoxChart) {
        let mut with_tp = chart_with_notes.clone();
        with_tp.timing_points.push(TimingPoint::bpm(0, 180.0));
        // notes_hash must be identical — timing change should not affect it
        assert_eq!(notes_hash(&chart_with_notes), notes_hash(&with_tp));
    }

    #[rstest]
    fn test_short_hash_is_16_chars(chart_with_notes: RoxChart) {
        let s = short_hash(&chart_with_notes);
        assert_eq!(s.len(), 16);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[rstest]
    fn test_short_hash_is_prefix_of_hash(chart_with_notes: RoxChart) {
        let h = hash(&chart_with_notes);
        let s = short_hash(&chart_with_notes);
        assert!(h.starts_with(&s));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p rox-analysis hash 2>&1 | head -30
```

Expected: compile error or `not yet implemented` panics.

- [ ] **Step 3: Implement the hash functions**

Replace the `todo!()` bodies:

```rust
use rkyv::rancor::Error as RkyvError;
use xxhash_rust::xxh3::xxh3_128;
use rox::model::RoxChart;

pub fn hash(chart: &RoxChart) -> String {
    let bytes = rkyv::to_bytes::<RkyvError>(chart).unwrap_or_default();
    format!("{:032x}", xxh3_128(&bytes))
}

pub fn notes_hash(chart: &RoxChart) -> String {
    let bytes = rkyv::to_bytes::<RkyvError>(&chart.notes).unwrap_or_default();
    format!("{:032x}", xxh3_128(&bytes))
}

pub fn timings_hash(chart: &RoxChart) -> String {
    let bytes = rkyv::to_bytes::<RkyvError>(&chart.timing_points).unwrap_or_default();
    format!("{:032x}", xxh3_128(&bytes))
}

pub fn short_hash(chart: &RoxChart) -> String {
    hash(chart)[..16].to_string()
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p rox-analysis hash
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/analysis/src/hash.rs
git commit -m "feat(rox-analysis): implement hash module with xxh3-128"
```

---

## Task 3: `rox-analysis` — bpm module

**Files:**
- Create: `crates/analysis/src/bpm.rs`

`TimingPoint` is now an enum. Use `tp.is_bpm()` and `tp.bpm_value()`. BPM field is `f32` — promote to `f64` for output.

- [ ] **Step 1: Write the failing tests in `crates/analysis/src/bpm.rs`**

```rust
use rox::model::RoxChart;

pub fn bpm_min(chart: &RoxChart) -> f64 {
    todo!()
}

pub fn bpm_max(chart: &RoxChart) -> f64 {
    todo!()
}

/// BPM active for the longest cumulative duration.
pub fn bpm_mode(chart: &RoxChart) -> f64 {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::model::{Note, TimingPoint};
    use rstest::{fixture, rstest};

    #[fixture]
    fn multi_bpm_chart() -> RoxChart {
        let mut chart = RoxChart::new(4);
        chart.timing_points.push(TimingPoint::bpm(0, 100.0));
        chart.timing_points.push(TimingPoint::bpm(10_000_000, 200.0));
        chart.timing_points.push(TimingPoint::bpm(20_000_000, 100.0));
        // note at 30s to define duration
        chart.notes.push(Note::tap(30_000_000, 0));
        chart
    }

    #[rstest]
    fn test_bpm_min(multi_bpm_chart: RoxChart) {
        assert_eq!(bpm_min(&multi_bpm_chart), 100.0);
    }

    #[rstest]
    fn test_bpm_max(multi_bpm_chart: RoxChart) {
        assert_eq!(bpm_max(&multi_bpm_chart), 200.0);
    }

    #[rstest]
    fn test_bpm_mode_returns_longest(multi_bpm_chart: RoxChart) {
        // 0-10s: 100bpm (10s), 10-20s: 200bpm (10s), 20-30s: 100bpm (10s)
        // 100bpm total: 20s, 200bpm total: 10s → mode = 100
        assert_eq!(bpm_mode(&multi_bpm_chart), 100.0);
    }

    #[test]
    fn test_bpm_empty_chart_returns_zero_or_infinity() {
        let chart = RoxChart::new(4);
        // empty → no BPM points
        assert_eq!(bpm_mode(&chart), 0.0);
        // min/max on empty should not panic; exact value is unspecified
        let _ = bpm_min(&chart);
        let _ = bpm_max(&chart);
    }

    #[test]
    fn test_sv_points_ignored() {
        let mut chart = RoxChart::new(4);
        chart.timing_points.push(TimingPoint::bpm(0, 180.0));
        chart.timing_points.push(TimingPoint::sv(5_000_000, 1.5)); // must be ignored
        chart.notes.push(Note::tap(10_000_000, 0));
        assert_eq!(bpm_min(&chart), 180.0);
        assert_eq!(bpm_max(&chart), 180.0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p rox-analysis bpm 2>&1 | head -20
```

Expected: `not yet implemented` panics.

- [ ] **Step 3: Implement**

```rust
use std::collections::HashMap;
use rox::model::RoxChart;

pub fn bpm_min(chart: &RoxChart) -> f64 {
    chart
        .timing_points
        .iter()
        .filter_map(|tp| tp.bpm_value())
        .map(f64::from)
        .fold(f64::INFINITY, f64::min)
}

pub fn bpm_max(chart: &RoxChart) -> f64 {
    chart
        .timing_points
        .iter()
        .filter_map(|tp| tp.bpm_value())
        .map(f64::from)
        .fold(f64::NEG_INFINITY, f64::max)
}

pub fn bpm_mode(chart: &RoxChart) -> f64 {
    let duration_us = chart.duration_us();
    if duration_us == 0 {
        return 0.0;
    }

    let mut bpm_points: Vec<_> = chart
        .timing_points
        .iter()
        .filter(|tp| tp.is_bpm())
        .collect();

    if bpm_points.is_empty() {
        return 0.0;
    }

    bpm_points.sort_by_key(|tp| tp.time_us());

    let mut durations: HashMap<String, f64> = HashMap::new();

    for (i, tp) in bpm_points.iter().enumerate() {
        let start = tp.time_us().max(0).min(duration_us);
        let end = bpm_points
            .get(i + 1)
            .map_or(duration_us, |next| next.time_us())
            .max(0)
            .min(duration_us);

        if end > start {
            let bpm_key = format!("{:.2}", tp.bpm_value().unwrap_or(0.0));
            *durations.entry(bpm_key).or_insert(0.0) += (end - start) as f64;
        }
    }

    durations
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .and_then(|(k, _)| k.parse::<f64>().ok())
        .unwrap_or(0.0)
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p rox-analysis bpm
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/analysis/src/bpm.rs
git commit -m "feat(rox-analysis): implement bpm module"
```

---

## Task 4: `rox-analysis` — nps module

**Files:**
- Create: `crates/analysis/src/nps.rs`

- [ ] **Step 1: Write the failing tests in `crates/analysis/src/nps.rs`**

```rust
use rox::model::RoxChart;

pub fn nps(chart: &RoxChart) -> f64 {
    todo!()
}

pub fn density(chart: &RoxChart, segments: usize) -> Vec<f64> {
    todo!()
}

pub fn highest_nps(chart: &RoxChart, window_s: f64) -> f64 {
    todo!()
}

pub fn lowest_nps(chart: &RoxChart, window_s: f64) -> f64 {
    todo!()
}

/// Duration in seconds of the longest continuous stretch where NPS stays
/// above the chart average, using a 1-second sliding window.
pub fn highest_drain_time(chart: &RoxChart) -> f64 {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::model::Note;
    use rstest::{fixture, rstest};

    #[fixture]
    fn three_note_chart() -> RoxChart {
        // 3 notes at 0s, 1s, 2s → duration 2s, avg NPS = 1.5
        let mut chart = RoxChart::new(4);
        chart.notes.push(Note::tap(0, 0));
        chart.notes.push(Note::tap(1_000_000, 0));
        chart.notes.push(Note::tap(2_000_000, 0));
        chart
    }

    #[rstest]
    fn test_nps_basic(three_note_chart: RoxChart) {
        assert_eq!(nps(&three_note_chart), 1.5);
    }

    #[test]
    fn test_nps_empty_chart() {
        assert_eq!(nps(&RoxChart::new(4)), 0.0);
    }

    #[test]
    fn test_density_two_segments() {
        let mut chart = RoxChart::new(4);
        // 10 notes in first 5s
        for i in 0..10u64 {
            chart.notes.push(Note::tap((i * 500_000) as i64, 0));
        }
        // 1 note at 9.999s to define ~10s duration
        chart.notes.push(Note::tap(9_999_999, 0));

        let dens = density(&chart, 2);
        assert_eq!(dens.len(), 2);
        // segment 0: ~10 notes / 5s = 2.0 NPS
        assert!((dens[0] - 2.0).abs() < 0.1, "got {}", dens[0]);
        // segment 1: ~1 note / 5s = 0.2 NPS
        assert!((dens[1] - 0.2).abs() < 0.1, "got {}", dens[1]);
    }

    #[test]
    fn test_density_zero_segments() {
        assert!(density(&RoxChart::new(4), 0).is_empty());
    }

    #[test]
    fn test_highest_nps_cluster() {
        let mut chart = RoxChart::new(4);
        // 10 notes within 0.5s at t=10s → peak NPS over 1s window = 10
        for i in 0..10i64 {
            chart.notes.push(Note::tap(10_000_000 + i * 50_000, 0));
        }
        chart.notes.push(Note::tap(0, 0));
        chart.notes.push(Note::tap(20_000_000, 0));

        assert_eq!(highest_nps(&chart, 1.0), 10.0);
    }

    #[test]
    fn test_lowest_nps_has_gap() {
        let mut chart = RoxChart::new(4);
        chart.notes.push(Note::tap(0, 0));
        chart.notes.push(Note::tap(1_000_000, 0));
        chart.notes.push(Note::tap(10_000_000, 0)); // big gap
        // 2s window: the gap has 0 notes
        assert_eq!(lowest_nps(&chart, 2.0), 0.0);
    }

    #[test]
    fn test_highest_drain_time_returns_positive() {
        let mut chart = RoxChart::new(4);
        // Dense section: 50 notes over 5s at 10 NPS
        for i in 0..50i64 {
            chart.notes.push(Note::tap(1_000_000 + i * 100_000, 0));
        }
        // Then another dense section: 100 notes over 10s
        for i in 0..100i64 {
            chart.notes.push(Note::tap(10_000_000 + i * 100_000, 0));
        }
        let drain = highest_drain_time(&chart);
        assert!(drain >= 9.0 && drain <= 10.5, "drain was {}", drain);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p rox-analysis nps 2>&1 | head -20
```

- [ ] **Step 3: Implement**

```rust
use rox::model::RoxChart;

pub fn nps(chart: &RoxChart) -> f64 {
    let duration_s = chart.duration_us() as f64 / 1_000_000.0;
    if duration_s <= 0.0 {
        return 0.0;
    }
    chart.note_count() as f64 / duration_s
}

pub fn density(chart: &RoxChart, segments: usize) -> Vec<f64> {
    if segments == 0 {
        return Vec::new();
    }
    let duration_us = chart.duration_us();
    if duration_us == 0 {
        return vec![0.0; segments];
    }
    let seg_us = duration_us as f64 / segments as f64;
    let mut counts = vec![0usize; segments];
    for note in &chart.notes {
        let idx = ((note.time_us as f64 / seg_us).floor() as usize).min(segments - 1);
        counts[idx] += 1;
    }
    let seg_s = seg_us / 1_000_000.0;
    counts.into_iter().map(|c| c as f64 / seg_s).collect()
}

pub fn highest_nps(chart: &RoxChart, window_s: f64) -> f64 {
    let window_us = (window_s * 1_000_000.0) as i64;
    if window_us <= 0 || chart.notes.is_empty() {
        return 0.0;
    }
    let mut times: Vec<i64> = chart.notes.iter().map(|n| n.time_us).collect();
    times.sort_unstable();

    let mut max_count = 0usize;
    let mut left = 0;
    for right in 0..times.len() {
        while times[right] - times[left] >= window_us {
            left += 1;
        }
        max_count = max_count.max(right - left + 1);
    }
    max_count as f64 / window_s
}

pub fn lowest_nps(chart: &RoxChart, window_s: f64) -> f64 {
    let window_us = (window_s * 1_000_000.0) as i64;
    let duration_us = chart.duration_us();
    if window_us <= 0 || duration_us <= 0 || chart.notes.is_empty() {
        return 0.0;
    }
    let mut times: Vec<i64> = chart.notes.iter().map(|n| n.time_us).collect();
    times.sort_unstable();

    let mut min_count = usize::MAX;
    let mut left = 0;
    for right in 0..times.len() {
        while times[right] - times[left] >= window_us {
            left += 1;
        }
        min_count = min_count.min(right - left + 1);
    }

    // Also check windows at the beginning/end with no notes
    let first = *times.first().unwrap_or(&0);
    let last = *times.last().unwrap_or(&0);
    if first > window_us || (duration_us - last) > window_us {
        return 0.0;
    }

    if min_count == usize::MAX { 0.0 } else { min_count as f64 / window_s }
}

pub fn highest_drain_time(chart: &RoxChart) -> f64 {
    let window_us = 1_000_000i64; // 1s window
    let threshold = nps(chart);
    let duration_us = chart.duration_us();

    if duration_us <= 0 || chart.notes.is_empty() {
        return 0.0;
    }

    let mut times: Vec<i64> = chart.notes.iter().map(|n| n.time_us).collect();
    times.sort_unstable();

    // Find windows above threshold, track longest streak
    let mut best_start: Option<i64> = None;
    let mut best_duration = 0i64;
    let mut streak_start: Option<i64> = None;
    let mut left = 0;

    for right in 0..times.len() {
        while times[right] - times[left] >= window_us {
            left += 1;
        }
        let count = right - left + 1;
        let local_nps = count as f64 / (window_us as f64 / 1_000_000.0);

        if local_nps >= threshold {
            if streak_start.is_none() {
                streak_start = Some(times[left]);
            }
            let streak_end = times[right];
            let dur = streak_end - streak_start.unwrap_or(streak_end);
            if dur > best_duration {
                best_duration = dur;
                best_start = streak_start;
            }
        } else {
            streak_start = None;
        }
    }
    let _ = best_start;
    best_duration as f64 / 1_000_000.0
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p rox-analysis nps
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/analysis/src/nps.rs
git commit -m "feat(rox-analysis): implement nps module"
```

---

## Task 5: `rox-analysis` — pattern module

**Files:**
- Create: `crates/analysis/src/pattern.rs`

`polyphony` counts how many notes share the same `time_us` (simultaneous notes). `lane_balance` counts notes per column.

- [ ] **Step 1: Write the failing tests in `crates/analysis/src/pattern.rs`**

```rust
use std::collections::HashMap;
use rox::model::RoxChart;

/// Count simultaneous notes: key = number of notes at same time, value = occurrence count.
pub fn polyphony(chart: &RoxChart) -> HashMap<u32, u32> {
    todo!()
}

/// Count notes per column (index 0..key_count-1).
pub fn lane_balance(chart: &RoxChart) -> Vec<u32> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::model::Note;
    use rstest::{fixture, rstest};

    #[fixture]
    fn pattern_chart() -> RoxChart {
        let mut chart = RoxChart::new(4);
        // t=0: 1 note (single)
        chart.notes.push(Note::tap(0, 0));
        // t=1s: 2 notes (jump)
        chart.notes.push(Note::tap(1_000_000, 0));
        chart.notes.push(Note::tap(1_000_000, 1));
        // t=2s: 3 notes (hand)
        chart.notes.push(Note::tap(2_000_000, 0));
        chart.notes.push(Note::tap(2_000_000, 1));
        chart.notes.push(Note::tap(2_000_000, 2));
        chart
    }

    #[rstest]
    fn test_polyphony_counts(pattern_chart: RoxChart) {
        let poly = polyphony(&pattern_chart);
        assert_eq!(poly.get(&1), Some(&1)); // one single
        assert_eq!(poly.get(&2), Some(&1)); // one jump
        assert_eq!(poly.get(&3), Some(&1)); // one hand
    }

    #[rstest]
    fn test_lane_balance_4k(pattern_chart: RoxChart) {
        let bal = lane_balance(&pattern_chart);
        assert_eq!(bal.len(), 4);
        // col 0: appears at t=0, t=1s, t=2s → 3
        assert_eq!(bal[0], 3);
        // col 1: appears at t=1s, t=2s → 2
        assert_eq!(bal[1], 2);
        // col 2: appears at t=2s → 1
        assert_eq!(bal[2], 1);
        // col 3: never → 0
        assert_eq!(bal[3], 0);
    }

    #[test]
    fn test_polyphony_empty() {
        assert!(polyphony(&RoxChart::new(4)).is_empty());
    }

    #[test]
    fn test_lane_balance_length_matches_key_count() {
        let chart = RoxChart::new(7);
        assert_eq!(lane_balance(&chart).len(), 7);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p rox-analysis pattern 2>&1 | head -20
```

- [ ] **Step 3: Implement**

```rust
use std::collections::HashMap;
use rox::model::RoxChart;

pub fn polyphony(chart: &RoxChart) -> HashMap<u32, u32> {
    let mut time_counts: HashMap<i64, u32> = HashMap::new();
    for note in &chart.notes {
        *time_counts.entry(note.time_us).or_insert(0) += 1;
    }
    let mut poly: HashMap<u32, u32> = HashMap::new();
    for count in time_counts.values() {
        *poly.entry(*count).or_insert(0) += 1;
    }
    poly
}

pub fn lane_balance(chart: &RoxChart) -> Vec<u32> {
    let mut counts = vec![0u32; chart.key_count as usize];
    for note in &chart.notes {
        let col = note.column as usize;
        if col < counts.len() {
            counts[col] += 1;
        }
    }
    counts
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p rox-analysis pattern
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/analysis/src/pattern.rs
git commit -m "feat(rox-analysis): implement pattern module (polyphony, lane_balance)"
```

---

## Task 6: `rox-analysis` — lib.rs with `RoxAnalysis` trait

**Files:**
- Modify: `crates/analysis/src/lib.rs`

- [ ] **Step 1: Write the failing integration test at the bottom of `lib.rs`**

```rust
pub mod bpm;
pub mod hash;
pub mod nps;
pub mod pattern;

use std::collections::HashMap;
use rox::model::RoxChart;

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

impl RoxAnalysis for RoxChart {
    fn bpm_min(&self) -> f64 { bpm::bpm_min(self) }
    fn bpm_max(&self) -> f64 { bpm::bpm_max(self) }
    fn bpm_mode(&self) -> f64 { bpm::bpm_mode(self) }
    fn nps(&self) -> f64 { nps::nps(self) }
    fn density(&self, segments: usize) -> Vec<f64> { nps::density(self, segments) }
    fn highest_nps(&self, window_s: f64) -> f64 { nps::highest_nps(self, window_s) }
    fn lowest_nps(&self, window_s: f64) -> f64 { nps::lowest_nps(self, window_s) }
    fn highest_drain_time(&self) -> f64 { nps::highest_drain_time(self) }
    fn polyphony(&self) -> HashMap<u32, u32> { pattern::polyphony(self) }
    fn lane_balance(&self) -> Vec<u32> { pattern::lane_balance(self) }
    fn hash(&self) -> String { hash::hash(self) }
    fn notes_hash(&self) -> String { hash::notes_hash(self) }
    fn timings_hash(&self) -> String { hash::timings_hash(self) }
    fn short_hash(&self) -> String { hash::short_hash(self) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::model::Note;

    #[test]
    fn test_trait_methods_callable_on_chart() {
        let mut chart = RoxChart::new(4);
        chart.notes.push(Note::tap(0, 0));
        chart.notes.push(Note::tap(1_000_000, 1));
        // just verify no panic and sensible values
        assert!(chart.nps() > 0.0);
        assert_eq!(chart.hash().len(), 32);
        assert_eq!(chart.short_hash().len(), 16);
        assert!(chart.short_hash().chars().all(|c| c.is_ascii_hexdigit()));
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p rox-analysis
```

Expected: all tests pass (including previous module tests).

- [ ] **Step 3: Commit**

```bash
git add crates/analysis/src/lib.rs
git commit -m "feat(rox-analysis): wire RoxAnalysis trait on RoxChart"
```

---

## Task 7: CLI — `apps/rox-cli/src/main.rs`

**Files:**
- Create: `apps/rox-cli/src/main.rs`

No external CLI framework — plain `std::env::args()` like the reference on main.

- [ ] **Step 1: Create `apps/rox-cli/src/main.rs`**

```rust
//! ROX CLI
//!
//! USAGE:
//!   rox convert <input> <output>
//!   rox info <file>
//!   rox validate <file>
//!   rox version
//!   rox help

mod cmd;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        cmd::help::run();
        return ExitCode::from(1);
    }

    match args[1].as_str() {
        "convert"           => cmd::convert::run(&args[2..]),
        "info"              => cmd::info::run(&args[2..]),
        "validate"          => cmd::validate::run(&args[2..]),
        "help" | "-h" | "--help" => { cmd::help::run(); ExitCode::SUCCESS }
        "version" | "-V" | "--version" => {
            println!("rox {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        unknown => {
            eprintln!("Unknown command: {unknown}");
            cmd::help::run();
            ExitCode::from(1)
        }
    }
}
```

- [ ] **Step 2: Create `apps/rox-cli/src/cmd/mod.rs`**

```rust
pub mod convert;
pub mod help;
pub mod info;
pub mod validate;
```

- [ ] **Step 3: Create `apps/rox-cli/src/cmd/help.rs`**

```rust
pub fn run() {
    println!(
        r#"ROX - Rhythm Open Exchange CLI

USAGE:
    rox <COMMAND> [OPTIONS]

COMMANDS:
    convert <input> <output>   Convert between chart formats
    info <file>                Display chart information and stats
    validate <file>            Validate a chart file
    version                    Show version
    help                       Show this help message

SUPPORTED FORMATS:
    .rox   - ROX binary format
    .jrox  - ROX JSON format
    .yrox  - ROX YAML format
    .osu   - osu!mania
    .sm    - StepMania
    .qua   - Quaver
    .json  - Friday Night Funkin'

EXAMPLES:
    rox convert song.osu song.qua
    rox info chart.rox
    rox validate song.osu
"#
    );
}
```

- [ ] **Step 4: Verify it compiles (stubs for other cmd modules not yet written)**

Create temporary stubs so it compiles:

`apps/rox-cli/src/cmd/convert.rs`:
```rust
use std::process::ExitCode;
pub fn run(_args: &[String]) -> ExitCode { todo!() }
```

`apps/rox-cli/src/cmd/info.rs`:
```rust
use std::process::ExitCode;
pub fn run(_args: &[String]) -> ExitCode { todo!() }
```

`apps/rox-cli/src/cmd/validate.rs`:
```rust
use std::process::ExitCode;
pub fn run(_args: &[String]) -> ExitCode { todo!() }
```

```bash
cargo check -p rox-cli
```

Expected: compiles cleanly.

- [ ] **Step 5: Commit**

```bash
git add apps/rox-cli/src/main.rs apps/rox-cli/src/cmd/
git commit -m "feat(rox-cli): scaffold CLI entry point and command modules"
```

---

## Task 8: CLI — `convert` command

**Files:**
- Modify: `apps/rox-cli/src/cmd/convert.rs`

- [ ] **Step 1: Implement `convert.rs`**

```rust
use std::path::PathBuf;
use std::process::ExitCode;
use rox_formats::auto::{auto_decode, auto_encode};

pub fn run(args: &[String]) -> ExitCode {
    if args.len() < 2 {
        eprintln!("Usage: rox convert <input> <output>");
        return ExitCode::from(1);
    }

    let input = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);

    println!("Converting: {} -> {}", input.display(), output.display());

    let chart = match auto_decode(&input) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error decoding {}: {e}", input.display());
            return ExitCode::from(1);
        }
    };

    println!(
        "  Loaded: {} - {} [{} notes, {}K]",
        chart.metadata.artist,
        chart.metadata.title,
        chart.notes.len(),
        chart.key_count,
    );

    if let Err(e) = auto_encode(&chart, &output) {
        eprintln!("Error encoding {}: {e}", output.display());
        return ExitCode::from(1);
    }

    println!("  Saved to: {}", output.display());
    ExitCode::SUCCESS
}
```

- [ ] **Step 2: Verify the import path is correct**

`auto_decode` and `auto_encode` are re-exported at the crate root in `crates/formats/src/lib.rs`:

```rust
use rox_formats::{auto_decode, auto_encode};
```

This is already what `convert.rs` uses — no changes needed.

- [ ] **Step 3: Compile check**

```bash
cargo check -p rox-cli
```

- [ ] **Step 4: Commit**

```bash
git add apps/rox-cli/src/cmd/convert.rs
git commit -m "feat(rox-cli): implement convert command"
```

---

## Task 9: CLI — `info` command

**Files:**
- Modify: `apps/rox-cli/src/cmd/info.rs`

- [ ] **Step 1: Implement `info.rs`**

```rust
use std::path::PathBuf;
use std::process::ExitCode;
use rox_formats::auto_decode;
use rox_analysis::RoxAnalysis; // trait must be in scope for chart.bpm_min() etc.

pub fn run(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("Usage: rox info <file>");
        return ExitCode::from(1);
    }

    let path = PathBuf::from(&args[0]);

    let chart = match auto_decode(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::from(1);
        }
    };

    println!("File: {}", path.display());

    println!();
    println!("=== Metadata ===");
    println!("  Title:      {}", chart.metadata.title);
    println!("  Artist:     {}", chart.metadata.artist);
    println!("  Creator:    {}", chart.metadata.creator);
    println!("  Difficulty: {}", chart.metadata.difficulty_name);
    if let Some(val) = chart.metadata.difficulty_value {
        println!("  Level:      {val:.2}");
    }
    println!("  Audio:      {}", chart.metadata.audio_file);
    if chart.metadata.is_coop {
        println!(
            "  Mode:       {}K Coop ({}K + {}K)",
            chart.key_count,
            chart.key_count / 2,
            chart.key_count / 2
        );
    } else {
        println!("  Mode:       {}K", chart.key_count);
    }

    println!();
    println!("=== Statistics ===");
    println!("  Notes:         {}", chart.notes.len());
    println!("  Timing Points: {}", chart.timing_points.len());
    let notes_with_hs = chart.notes.iter().filter(|n| n.hitsound_index.is_some()).count();
    if notes_with_hs > 0 {
        println!(
            "  Hitsounds:     {notes_with_hs} notes ({} samples)",
            chart.hitsounds.len()
        );
    }
    #[allow(clippy::cast_precision_loss)]
    let duration_s = chart.duration_us() as f64 / 1_000_000.0;
    println!("  Duration:      {duration_s:.1}s");

    println!();
    println!("=== Hashes ===");
    println!("  Hash:         {}", chart.hash());
    println!("  Notes Hash:   {}", chart.notes_hash());
    println!("  Timings Hash: {}", chart.timings_hash());

    println!();
    println!("=== Analysis ===");
    println!(
        "  BPM:    {:.1} - {:.1}  (Mode: {:.1})",
        chart.bpm_min(),
        chart.bpm_max(),
        chart.bpm_mode()
    );
    println!(
        "  NPS:    {:.2}  (Peak: {:.2})",
        chart.nps(),
        chart.highest_nps(1.0)
    );
    println!("  Drain:  {:.1}s", chart.highest_drain_time());

    println!();
    println!("  Polyphony:");
    let mut poly: Vec<_> = chart.polyphony().into_iter().collect();
    poly.sort_by_key(|&(k, _)| k);
    for (k, v) in poly {
        let label = match k {
            1 => "Single",
            2 => "Jump",
            3 => "Hand",
            4 => "Quad",
            _ => "Cluster",
        };
        println!("    {label}: {v}");
    }

    println!();
    println!("  Lane Balance:");
    let balance = chart.lane_balance();
    let total: u32 = balance.iter().sum();
    for (i, count) in balance.iter().enumerate() {
        let pct = if total > 0 { *count as f64 / total as f64 * 100.0 } else { 0.0 };
        println!("    Col {}: {count} ({pct:.1}%)", i + 1);
    }

    ExitCode::SUCCESS
}
```

- [ ] **Step 2: Compile check**

```bash
cargo check -p rox-cli
```

- [ ] **Step 3: Commit**

```bash
git add apps/rox-cli/src/cmd/info.rs
git commit -m "feat(rox-cli): implement info command"
```

---

## Task 10: CLI — `validate` command + final build check

**Files:**
- Modify: `apps/rox-cli/src/cmd/validate.rs`

- [ ] **Step 1: Implement `validate.rs`**

```rust
use std::path::PathBuf;
use std::process::ExitCode;
use rox_formats::auto_decode;

pub fn run(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("Usage: rox validate <file>");
        return ExitCode::from(1);
    }

    let path = PathBuf::from(&args[0]);

    let chart = match auto_decode(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error loading {}: {e}", path.display());
            return ExitCode::from(1);
        }
    };

    match chart.validate() {
        Ok(()) => {
            println!("{} is valid", path.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{} validation failed: {e}", path.display());
            ExitCode::from(1)
        }
    }
}
```

- [ ] **Step 2: Full workspace build and test**

```bash
cargo build --workspace
cargo test --workspace
```

Expected: all tests pass, binary builds at `target/debug/rox`.

- [ ] **Step 3: Smoke test the binary**

```bash
./target/debug/rox help
./target/debug/rox version
```

Expected: help text prints, version prints `rox 0.7.0`.

- [ ] **Step 4: Commit**

```bash
git add apps/rox-cli/src/cmd/validate.rs
git commit -m "feat(rox-cli): implement validate command"
```
