use crate::model::Note;
use crate::model::RoxChart;

/// Calculate the average Notes Per Second (NPS).
pub fn nps(chart: &RoxChart) -> f64 {
    let duration_s = chart.duration_us() as f64 / 1_000_000.0;
    if duration_s <= 0.0 {
        return 0.0;
    }
    chart.note_count() as f64 / duration_s
}

/// Calculate NPS density divided into `segments`.
/// Returns a vector of NPS values for each segment.
pub fn density(chart: &RoxChart, segments: usize) -> Vec<f64> {
    if segments == 0 {
        return Vec::new();
    }

    let duration_us = chart.duration_us();
    if duration_us == 0 {
        return vec![0.0; segments];
    }

    let segment_duration_us = duration_us as f64 / segments as f64;
    let mut segment_counts = vec![0; segments];

    for note in &chart.notes {
        // Find which segment this note belongs to
        // We use note.time_us.
        // Clamping to ensure it falls within range 0..segments-1
        let idx = ((note.time_us as f64 / segment_duration_us).floor() as usize).min(segments - 1);
        segment_counts[idx] += 1;
    }

    let segment_duration_s = segment_duration_us / 1_000_000.0;

    segment_counts
        .into_iter()
        .map(|count| {
            if segment_duration_s > 0.0 {
                count as f64 / segment_duration_s
            } else {
                0.0
            }
        })
        .collect()
}

/// Calculate the highest peak NPS using a sliding window.
/// `window_size_s` is in seconds (e.g. 1.0).
pub fn highest_nps(chart: &RoxChart, window_size_s: f64) -> f64 {
    let window_us = (window_size_s * 1_000_000.0) as i64;
    if window_us <= 0 || chart.notes.is_empty() {
        return 0.0;
    }

    let mut notes: Vec<&Note> = chart.notes.iter().collect();
    notes.sort_by_key(|n| n.time_us);

    let mut max_notes_in_window = 0;
    let mut left = 0;

    for right in 0..notes.len() {
        let window_end = notes[right].time_us;
        let window_start = window_end - window_us;

        // Advance left pointer to be within window [window_end - window, window_end]
        while left < right && notes[left].time_us <= window_start {
            left += 1;
        }

        // Count notes in window
        let count = right - left + 1;
        if count > max_notes_in_window {
            max_notes_in_window = count;
        }
    }

    max_notes_in_window as f64 / window_size_s
}

/// Calculate the lowest NPS using a sliding window, starting from the first note.
/// `window_size_s` is in seconds (e.g. 1.0).
pub fn lowest_nps(chart: &RoxChart, window_size_s: f64) -> f64 {
    let window_us = (window_size_s * 1_000_000.0) as i64;
    if window_us <= 0 || chart.notes.is_empty() {
        return 0.0;
    }

    let mut notes: Vec<&Note> = chart.notes.iter().collect();
    notes.sort_by_key(|n| n.time_us);

    let first_note_time = notes[0].time_us;

    // Check for gaps larger than window
    for window in notes.windows(2) {
        let gap = window[1].time_us - window[0].time_us;
        if gap > window_us {
            return 0.0;
        }
    }

    let mut min_notes_in_window = usize::MAX;
    let mut left = 0;

    for right in 0..notes.len() {
        let window_end = notes[right].time_us;
        let window_start = window_end - window_us;

        // We only care if window starts >= first_note_time
        if window_start < first_note_time {
            continue;
        }

        while left < right && notes[left].time_us <= window_start {
            left += 1;
        }

        let count = right - left + 1;
        if count < min_notes_in_window {
            min_notes_in_window = count;
        }
    }

    if min_notes_in_window == usize::MAX {
        return 0.0;
    }

    min_notes_in_window as f64 / window_size_s
}

/// Calculate the longest duration where NPS is maintained above 90% of the max NPS.
/// Returns duration in seconds.
/// This uses a 1.0s sliding window sampled every 100ms.
pub fn highest_drain_time(chart: &RoxChart) -> f64 {
    // 1. Calculate Peak NPS (using 1s window)
    let peak_nps = highest_nps(chart, 1.0);
    if peak_nps <= 0.0 {
        return 0.0;
    }

    let threshold = peak_nps * 0.9;
    let duration_us = chart.duration_us();
    if duration_us <= 0 {
        return 0.0;
    }

    // 2. Scan chart every 100ms
    let scan_step_us = 100_000; // 0.1s
    let window_size_us = 1_000_000; // 1s

    let mut max_contiguous_ticks = 0;
    let mut current_contiguous_ticks = 0;

    // Sort notes for efficient window checks
    let mut notes: Vec<_> = chart.notes.iter().map(|n| n.time_us).collect();
    notes.sort_unstable();

    if notes.is_empty() {
        return 0.0;
    }

    // Optimize: sliding window count
    let mut left = 0;
    let mut right = 0;

    // Scan time t from 0 to duration
    let mut t = 0;
    while t <= duration_us {
        let window_end = t;
        let window_start = t - window_size_us;

        // Ensure `right` includes all notes <= window_end
        while right < notes.len() && notes[right] <= window_end {
            right += 1;
        }

        // Ensure `left` excludes notes < window_start
        while left < right && notes[left] < window_start.max(0) {
            left += 1;
        }

        let count = right - left;
        let current_nps = count as f64; // Since window is 1.0s, count = NPS

        if current_nps >= threshold {
            current_contiguous_ticks += 1;
        } else {
            if current_contiguous_ticks > max_contiguous_ticks {
                max_contiguous_ticks = current_contiguous_ticks;
            }
            current_contiguous_ticks = 0;
        }

        t += scan_step_us;
    }

    if current_contiguous_ticks > max_contiguous_ticks {
        max_contiguous_ticks = current_contiguous_ticks;
    }

    (max_contiguous_ticks as f64 * scan_step_us as f64) / 1_000_000.0
}
