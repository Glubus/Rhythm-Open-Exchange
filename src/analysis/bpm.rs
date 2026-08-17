#![warn(clippy::pedantic)]

use std::collections::HashMap;

use rox::model::{RoxChart, TimingPoint};

#[must_use]
pub fn bpm_min(chart: &RoxChart) -> f64 {
    chart
        .timing_points
        .iter()
        .filter_map(TimingPoint::bpm_value)
        .map(f64::from)
        .reduce(f64::min)
        .unwrap_or(0.0)
}

#[must_use]
pub fn bpm_max(chart: &RoxChart) -> f64 {
    chart
        .timing_points
        .iter()
        .filter_map(TimingPoint::bpm_value)
        .map(f64::from)
        .reduce(f64::max)
        .unwrap_or(0.0)
}

/// BPM active for the longest cumulative duration.
#[must_use]
pub fn bpm_mode(chart: &RoxChart) -> f64 {
    let duration_us = chart.duration_us();
    if duration_us == 0 {
        return 0.0;
    }

    let mut bpm_points: Vec<(i64, f32)> = chart
        .timing_points
        .iter()
        .filter_map(|tp| match tp {
            TimingPoint::Bpm { time_us, bpm, .. } => Some((*time_us, *bpm)),
            TimingPoint::Sv { .. } => None,
        })
        .collect();

    if bpm_points.is_empty() {
        return 0.0;
    }

    bpm_points.sort_by_key(|&(time_us, _)| time_us);

    let mut durations: HashMap<u32, f64> = HashMap::new();

    for (i, &(time_us, bpm)) in bpm_points.iter().enumerate() {
        let start = time_us.max(0).min(duration_us);
        let end = bpm_points
            .get(i + 1)
            .map_or(duration_us, |&(t, _)| t)
            .max(0)
            .min(duration_us);

        if end > start {
            #[allow(clippy::cast_precision_loss)]
            let dur = (end - start) as f64;
            *durations.entry(bpm.to_bits()).or_insert(0.0) += dur;
        }
    }

    durations
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map_or(0.0, |(bits, _)| f64::from(f32::from_bits(bits)))
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
        chart
            .timing_points
            .push(TimingPoint::bpm(10_000_000, 200.0));
        chart
            .timing_points
            .push(TimingPoint::bpm(20_000_000, 100.0));
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
    fn test_bpm_empty_chart_returns_zero() {
        let chart = RoxChart::new(4);
        assert_eq!(bpm_mode(&chart), 0.0);
        assert_eq!(bpm_min(&chart), 0.0);
        assert_eq!(bpm_max(&chart), 0.0);
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
