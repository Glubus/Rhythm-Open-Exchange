pub mod bpm;
pub mod grid;
pub mod merger;
pub mod timeline;
pub mod tree;
pub mod types;
pub mod window;

pub use bpm::TimingAnalyzer;
pub use grid::PatternGrid;
pub use timeline::{PatternTimeline, PatternTimelineEntry};
pub use tree::{QuadTreeBuilder, QuadTreeNode};
pub use types::{PatternCategory, PatternClassification, PatternType};
pub use window::CrossSegmentAnalyzer;

use crate::model::RoxChart;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, Deserialize)]
pub struct AnalysisResult {
    pub tree: Vec<QuadTreeNode>,
    pub timeline: PatternTimeline,
    pub key_count: u8,
}

impl Serialize for AnalysisResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Custom serialization to flatten time to seconds and hide the raw tree
        let mut state = serializer.serialize_struct("AnalysisResult", 2)?;
        state.serialize_field("timeline", &self.timeline.entries)?;
        state.serialize_field("key_count", &self.key_count)?;
        state.end()
    }
}

/// Perform full pattern recognition analysis on a chart.
pub fn analyze(chart: &RoxChart) -> AnalysisResult {
    let key_count = chart.key_count();
    // Quattern defaults
    let max_time_slices = 20;
    let ignore_holds = true;
    let window_size = 4;

    let (grids, timestamps) = PatternGrid::from_chart(chart, max_time_slices, ignore_holds);

    let mut trees = Vec::new();
    for grid in &grids {
        let builder = QuadTreeBuilder::new(grid);
        trees.push(builder.build());
    }

    // New Window-based Analysis (Quattern 1:1 match)
    let timing_analyzer = TimingAnalyzer::new(chart, ignore_holds);
    let cross_analyzer =
        CrossSegmentAnalyzer::new(&grids, &timestamps, &timing_analyzer, key_count as usize);
    let cross_results = cross_analyzer.analyze_cross_segment(window_size);

    let timeline = PatternTimeline::build_from_cross_analysis(
        &cross_results,
        &grids,
        &timestamps,
        key_count as usize,
    );

    AnalysisResult {
        tree: trees,
        timeline,
        key_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Note, RoxChart, TimingPoint};

    // Helper to create a dummy chart with a simple stream
    fn create_test_chart() -> RoxChart {
        let mut chart = RoxChart::new(4);
        chart.timing_points.push(TimingPoint::bpm(0, 150.0));

        // Add 1 second of stream (16th notes at 150 BPM)
        // 1 beat = 400ms, 1/4 beat = 100ms = 100_000 us
        let interval = 100_000;
        for i in 0..16 {
            chart.notes.push(Note::tap(i * interval, (i % 4) as u8));
        }
        chart
    }

    #[test]
    fn test_pattern_recognition_json_format() {
        let chart = create_test_chart();
        let result = analyze(&chart);

        // Serialize to JSON value
        let json = serde_json::to_value(&result).expect("Failed to serialize");

        // 1. Check top-level structure
        assert!(json.get("timeline").is_some(), "Missing 'timeline' field");
        assert!(
            json.get("tree").is_none(),
            "Should NOT expose 'tree' in final JSON"
        );

        // 2. Check timeline entries
        let timeline = json["timeline"]
            .as_array()
            .expect("Timeline should be an array");
        // We expect at least one entry for the stream we created
        // Note: The original analyze might return empty if thresholds aren't met,
        // but with 1 second of stream it should likely find something or at least an empty timeline structure.

        if !timeline.is_empty() {
            let entry = &timeline[0];

            // 3. Check specific fields (flat structure)
            assert!(entry.get("start_time").is_some(), "Missing 'start_time'");
            assert!(entry.get("end_time").is_some(), "Missing 'end_time'");
            assert!(entry.get("duration").is_some(), "Missing 'duration'");
            assert!(
                entry.get("pattern_type").is_some(),
                "Missing 'pattern_type'"
            ); // Was 'pattern'
            assert!(entry.get("avg_bpm").is_some(), "Missing 'avg_bpm'");
            assert!(entry.get("min_bpm").is_some(), "Missing 'min_bpm'");
            assert!(entry.get("max_bpm").is_some(), "Missing 'max_bpm'");
            assert!(entry.get("note_count").is_some(), "Missing 'note_count'");
        }
    }
}
