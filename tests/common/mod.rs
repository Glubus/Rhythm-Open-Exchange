//! Common test utilities and helpers.

#![allow(dead_code)]

use rhythm_open_exchange::{Metadata, Note, RoxChart, TimingPoint};

/// Create a minimal valid chart for testing.
pub fn minimal_chart(key_count: u8) -> RoxChart {
    let mut chart = RoxChart::new(key_count);
    chart.timing_points.push(TimingPoint::bpm(0, 120.0));
    chart
}

/// Create a chart with basic metadata.
pub fn chart_with_metadata() -> RoxChart {
    let mut chart = RoxChart::new(4);
    chart.metadata = Metadata {
        title: "Test Song".into(),
        artist: "Test Artist".into(),
        creator: "Tester".into(),
        difficulty_name: "Normal".into(),
        audio_file: "test.mp3".into(),
        ..Default::default()
    };
    chart.timing_points.push(TimingPoint::bpm(0, 120.0));
    chart
}

/// Create a chart with notes evenly spread across columns.
pub fn chart_with_spread_notes(key_count: u8, note_count: usize) -> RoxChart {
    let mut chart = RoxChart::new(key_count);
    chart.timing_points.push(TimingPoint::bpm(0, 120.0));

    for i in 0..note_count {
        let column = (i % key_count as usize) as u8;
        let time = (i as i64) * 100_000; // 100ms spacing
        chart.notes.push(Note::tap(time, column));
    }

    chart
}
