use crate::model::RoxChart;
use std::collections::BTreeMap;

/// Info for a specific point in time.
#[derive(Debug, Clone)]
pub struct TimingInfo {
    pub timestamp: i64,
    pub delta_to_next: i64,
    pub bpm: f64,
    pub note_count: usize,
}

/// Analyzes timing information and delta times between notes.
///
/// # Why
/// Calculates BPM using: BPM = 15000 / delta_time (for 1/4 notes)
/// This is needed to detect density-based difficulty spikes.
pub struct TimingAnalyzer {
    pub timing_info: Vec<TimingInfo>,
}

impl TimingAnalyzer {
    /// Creates a new TimingAnalyzer.
    ///
    /// # Why
    /// We sort and group notes by time to calculate accurate deltas.
    pub fn new(chart: &RoxChart, ignore_holds: bool) -> Self {
        let notes = Self::get_sorted_notes(chart, ignore_holds);
        let time_groups = Self::group_notes_by_time(&notes);
        let sorted_times: Vec<i64> = time_groups.keys().cloned().collect();

        let info_list = Self::calculate_timing_info(&sorted_times, &time_groups);

        Self {
            timing_info: info_list,
        }
    }

    /// Filters and sorts notes.
    fn get_sorted_notes(chart: &RoxChart, ignore_holds: bool) -> Vec<&crate::model::Note> {
        let mut notes: Vec<_> = chart
            .notes
            .iter()
            .filter(|n| !ignore_holds || !n.is_hold())
            .collect();
        notes.sort_by_key(|n| n.time_us);
        notes
    }

    /// Groups notes by timestamp.
    fn group_notes_by_time(notes: &[&crate::model::Note]) -> BTreeMap<i64, usize> {
        let mut time_groups = BTreeMap::new();
        for note in notes {
            *time_groups.entry(note.time_us).or_default() += 1;
        }
        time_groups
    }

    /// Calculates timing info for each time point.
    fn calculate_timing_info(times: &[i64], groups: &BTreeMap<i64, usize>) -> Vec<TimingInfo> {
        let mut list = Vec::with_capacity(times.len());

        for (i, &time) in times.iter().enumerate() {
            let delta = if i < times.len() - 1 {
                times[i + 1] - time
            } else {
                0
            };

            let bpm = if delta > 0 {
                15_000_000.0 / delta as f64
            } else {
                0.0
            };

            list.push(TimingInfo {
                timestamp: time,
                delta_to_next: delta,
                bpm,
                note_count: groups[&time],
            });
        }
        list
    }
}
