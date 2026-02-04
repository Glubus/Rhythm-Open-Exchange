use crate::model::RoxChart;
use std::collections::HashMap;

/// Calculate the minimum BPM in the chart.
pub fn bpm_min(chart: &RoxChart) -> f64 {
    chart
        .timing_points
        .iter()
        .filter(|tp| !tp.is_inherited)
        .map(|tp| tp.bpm as f64)
        .fold(f64::INFINITY, f64::min)
}

/// Calculate the maximum BPM in the chart.
pub fn bpm_max(chart: &RoxChart) -> f64 {
    chart
        .timing_points
        .iter()
        .filter(|tp| !tp.is_inherited)
        .map(|tp| tp.bpm as f64)
        .fold(f64::NEG_INFINITY, f64::max)
}

/// Calculate the mode BPM (weighted by duration).
///
/// Returns the BPM that is active for the longest total duration in the chart.
pub fn bpm_mode(chart: &RoxChart) -> f64 {
    let duration_us = chart.duration_us();
    if duration_us == 0 {
        return 0.0;
    }

    let mut bpm_durations: HashMap<String, f64> = HashMap::new(); // Use String for key to avoid float NaNs issues, or just i64 bits

    // Sort timing points by time just in case (though they should be sorted)
    let mut timing_points = chart.timing_points.clone();
    timing_points.sort_by_key(|tp| tp.time_us);

    // Filter only BPM points
    let bpm_points: Vec<_> = timing_points
        .into_iter()
        .filter(|tp| !tp.is_inherited)
        .collect();

    if bpm_points.is_empty() {
        return 0.0;
    }

    for i in 0..bpm_points.len() {
        let current_tp = &bpm_points[i];
        let next_time = if i + 1 < bpm_points.len() {
            bpm_points[i + 1].time_us
        } else {
            duration_us
        };

        // If the BPM point is after the song end (rare but possible), clamp it
        let start_time = current_tp.time_us.max(0).min(duration_us);
        let end_time = next_time.max(0).min(duration_us);

        if end_time > start_time {
            let dur = (end_time - start_time) as f64;
            // Round BPM to 2 decimal places to group similar BPMs
            let bpm_key = format!("{:.2}", current_tp.bpm);
            *bpm_durations.entry(bpm_key).or_insert(0.0) += dur;
        }
    }

    // Find max duration
    bpm_durations
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(k, _)| k.parse::<f64>().unwrap_or(0.0))
        .unwrap_or(0.0)
}
