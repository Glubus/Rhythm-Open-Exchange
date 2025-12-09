//! Tests for osu decoder (.osu -> RoxChart).

use rhythm_open_exchange::codec::Decoder;
use rhythm_open_exchange::codec::formats::osu::OsuDecoder;

#[test]
fn test_decode_sample_7k() {
    let data = include_bytes!("../../../../assets/osu/mania_7k.osu");
    let chart = OsuDecoder::decode(data).expect("Failed to decode");

    assert_eq!(chart.key_count, 7);
    assert!(!chart.notes.is_empty());
    assert!(!chart.timing_points.is_empty());
    assert_eq!(chart.metadata.difficulty_name, "7K Awakened");
    assert_eq!(chart.metadata.creator, "arcwinolivirus");
}

#[test]
fn test_decode_metadata() {
    let data = include_bytes!("../../../../assets/osu/mania_7k.osu");
    let chart = OsuDecoder::decode(data).unwrap();

    // Check unicode title is used
    assert!(chart.metadata.title.contains("宙の旋律") || chart.metadata.title.contains("Sora"));
    assert!(!chart.metadata.audio_file.is_empty());
    assert!(chart.metadata.background_file.is_some());
}

#[test]
fn test_decode_timing_points() {
    let data = include_bytes!("../../../../assets/osu/mania_7k.osu");
    let chart = OsuDecoder::decode(data).unwrap();

    // Should have at least one BPM point
    let bpm_points: Vec<_> = chart
        .timing_points
        .iter()
        .filter(|tp| !tp.is_inherited)
        .collect();
    assert!(!bpm_points.is_empty());

    // First timing point should be around 186 BPM
    let first_bpm = &bpm_points[0];
    assert!((first_bpm.bpm - 186.0).abs() < 1.0);
}

#[test]
fn test_decode_notes_sorted() {
    let data = include_bytes!("../../../../assets/osu/mania_7k.osu");
    let chart = OsuDecoder::decode(data).unwrap();

    // Notes should be sorted by time
    for window in chart.notes.windows(2) {
        assert!(window[0].time_us <= window[1].time_us);
    }
}
