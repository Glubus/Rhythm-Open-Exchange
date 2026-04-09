#![warn(clippy::pedantic)]

use rox::model::RoxChart;

/// Average notes per second over the full chart duration.
#[must_use]
pub fn nps(chart: &RoxChart) -> f64 {
    #[allow(clippy::cast_precision_loss)] // duration_us and note_count fit comfortably in f64
    let duration_s = chart.duration_us() as f64 / 1_000_000.0;
    if duration_s <= 0.0 {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)] // note_count fits comfortably in f64
    let count = chart.note_count() as f64;
    count / duration_s
}

/// NPS split into `segments` equal-duration buckets.
#[must_use]
pub fn density(chart: &RoxChart, segments: usize) -> Vec<f64> {
    if segments == 0 {
        return Vec::new();
    }
    let duration_us = chart.duration_us();
    if duration_us == 0 {
        return vec![0.0; segments];
    }
    #[allow(clippy::cast_precision_loss)] // duration_us fits in f64 for typical chart lengths
    let bucket_micros = duration_us as f64 / segments as f64;
    let mut counts = vec![0usize; segments];
    for note in &chart.notes {
        #[allow(clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let idx = ((note.time_us as f64 / bucket_micros).floor() as usize).min(segments - 1);
        counts[idx] += 1;
    }
    // Convert bucket width from microseconds to seconds for NPS calculation
    let bucket_seconds = bucket_micros / 1_000_000.0;
    #[allow(clippy::cast_precision_loss)] // segment counts fit in f64
    counts.into_iter().map(|c| c as f64 / bucket_seconds).collect()
}

/// Peak NPS in any sliding window of `window_s` seconds.
#[must_use]
pub fn highest_nps(chart: &RoxChart, window_s: f64) -> f64 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // window_s is positive
    let win_micros = (window_s * 1_000_000.0) as i64;
    if win_micros <= 0 || chart.notes.is_empty() {
        return 0.0;
    }
    let mut times: Vec<i64> = chart.notes.iter().map(|n| n.time_us).collect();
    times.sort_unstable();

    let mut max_count = 0usize;
    let mut left = 0;
    for right in 0..times.len() {
        while times[right] - times[left] >= win_micros {
            left += 1;
        }
        max_count = max_count.max(right - left + 1);
    }
    #[allow(clippy::cast_precision_loss)] // max_count fits in f64
    let count = max_count as f64;
    count / window_s
}

/// Minimum NPS in any sliding window of `window_s` seconds.
#[must_use]
pub fn lowest_nps(chart: &RoxChart, window_s: f64) -> f64 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // window_s is positive
    let win_micros = (window_s * 1_000_000.0) as i64;
    let duration_us = chart.duration_us();
    if win_micros <= 0 || duration_us <= 0 || chart.notes.is_empty() {
        return 0.0;
    }
    let mut times: Vec<i64> = chart.notes.iter().map(|n| n.time_us).collect();
    times.sort_unstable();

    // Check if there is any window-sized gap with no notes: before first, after last, or between consecutive notes
    let first = *times.first().unwrap_or(&0);
    let last = *times.last().unwrap_or(&0);
    if first > win_micros || (duration_us - last) > win_micros {
        return 0.0;
    }
    // Check gaps between consecutive notes
    for pair in times.windows(2) {
        if pair[1] - pair[0] > win_micros {
            return 0.0;
        }
    }

    let mut min_count = usize::MAX;
    let mut left = 0;
    for right in 0..times.len() {
        while times[right] - times[left] >= win_micros {
            left += 1;
        }
        min_count = min_count.min(right - left + 1);
    }

    if min_count == usize::MAX {
        0.0
    } else {
        #[allow(clippy::cast_precision_loss)] // min_count fits in f64
        let count = min_count as f64;
        count / window_s
    }
}

/// Duration in seconds of the longest continuous stretch where the
/// 1-second sliding window NPS stays at or above the chart average NPS.
#[must_use]
pub fn highest_drain_time(chart: &RoxChart) -> f64 {
    let window_us = 1_000_000i64; // 1s window
    let threshold = nps(chart);
    let duration_us = chart.duration_us();

    if duration_us <= 0 || chart.notes.is_empty() {
        return 0.0;
    }

    let mut times: Vec<i64> = chart.notes.iter().map(|n| n.time_us).collect();
    times.sort_unstable();

    let mut best_duration = 0i64;
    let mut streak_start: Option<i64> = None;
    let mut left = 0;

    for right in 0..times.len() {
        while times[right] - times[left] >= window_us {
            left += 1;
        }
        let count = right - left + 1;
        #[allow(clippy::cast_precision_loss)] // count fits in f64
        let local_nps = count as f64 / (window_us as f64 / 1_000_000.0);

        if local_nps >= threshold {
            if streak_start.is_none() {
                streak_start = Some(times[left]);
            }
            let streak_end = times[right];
            let dur = streak_end - streak_start.unwrap_or(streak_end);
            if dur > best_duration {
                best_duration = dur;
            }
        } else {
            streak_start = None;
        }
    }
    #[allow(clippy::cast_precision_loss)] // best_duration fits in f64
    let result = best_duration as f64 / 1_000_000.0;
    result
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
