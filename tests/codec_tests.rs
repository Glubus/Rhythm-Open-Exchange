//! Codec tests for ROX format encoding/decoding.

use rhythm_open_exchange::{
    Decoder, Encoder, Hitsound, Metadata, Note, RoxChart, RoxCodec, TimingPoint,
};

mod common;

/// Test basic roundtrip: encode then decode should produce identical chart.
#[test]
fn test_roundtrip_basic() {
    let mut chart = RoxChart::new(4);
    chart.metadata = Metadata {
        title: "Test Song".into(),
        artist: "Test Artist".into(),
        creator: "Mapper".into(),
        difficulty_name: "Hard".into(),
        difficulty_value: Some(5.5),
        audio_file: "audio.mp3".into(),
        background_file: Some("bg.png".into()),
        preview_time_us: 30_000_000,
        ..Default::default()
    };
    chart.timing_points.push(TimingPoint::bpm(0, 180.0));
    chart.timing_points.push(TimingPoint::sv(60_000_000, 1.5));
    chart.notes.push(Note::tap(1_000_000, 0));
    chart.notes.push(Note::tap(1_500_000, 1));
    chart.notes.push(Note::hold(2_000_000, 1_000_000, 2));

    let encoded = RoxCodec::encode(&chart).expect("Failed to encode");
    let decoded = RoxCodec::decode(&encoded).expect("Failed to decode");

    assert_eq!(chart, decoded);
}

/// Test roundtrip with all note types.
#[test]
fn test_roundtrip_all_note_types() {
    let mut chart = RoxChart::new(4);
    chart.timing_points.push(TimingPoint::bpm(0, 120.0));
    chart.notes.push(Note::tap(0, 0));
    chart.notes.push(Note::hold(1_000_000, 500_000, 1));
    chart.notes.push(Note::burst(2_000_000, 300_000, 2));
    chart.notes.push(Note::mine(3_000_000, 3));

    let encoded = RoxCodec::encode(&chart).expect("Failed to encode");
    let decoded = RoxCodec::decode(&encoded).expect("Failed to decode");

    assert_eq!(chart, decoded);
}

/// Test roundtrip with hitsounds.
#[test]
fn test_roundtrip_with_hitsounds() {
    let mut chart = RoxChart::new(4);
    chart.timing_points.push(TimingPoint::bpm(0, 140.0));

    chart.hitsounds.push(Hitsound::new("kick.wav"));
    chart.hitsounds.push(Hitsound::with_volume("snare.wav", 80));

    let mut note = Note::tap(0, 0);
    note.hitsound_index = Some(0);
    chart.notes.push(note);

    let mut note2 = Note::tap(500_000, 1);
    note2.hitsound_index = Some(1);
    chart.notes.push(note2);

    let encoded = RoxCodec::encode(&chart).expect("Failed to encode");
    let decoded = RoxCodec::decode(&encoded).expect("Failed to decode");

    assert_eq!(chart, decoded);
}

/// Test that magic bytes are correctly written.
#[test]
fn test_magic_bytes() {
    let chart = RoxChart::new(4);
    let encoded = RoxCodec::encode(&chart).expect("Failed to encode");

    assert!(encoded.len() >= 4);
    assert_eq!(&encoded[..4], &[0x52, 0x4F, 0x58, 0x00]); // "ROX\0"
}

/// Test decoding with invalid magic bytes fails.
#[test]
fn test_invalid_magic_bytes() {
    let bad_data = [0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03];
    let result = RoxCodec::decode(&bad_data);

    assert!(result.is_err());
}

/// Test decoding empty data fails.
#[test]
fn test_decode_empty_data() {
    let result = RoxCodec::decode(&[]);
    assert!(result.is_err());
}

/// Test decoding data too short for magic fails.
#[test]
fn test_decode_short_data() {
    let result = RoxCodec::decode(&[0x52, 0x4F]);
    assert!(result.is_err());
}

/// Test encoding empty chart succeeds.
#[test]
fn test_encode_empty_chart() {
    let chart = RoxChart::new(7);
    let encoded = RoxCodec::encode(&chart);

    assert!(encoded.is_ok());
    let data = encoded.unwrap();
    assert!(data.len() >= 4);
}

/// Test encoding chart with invalid column fails validation.
#[test]
fn test_encode_invalid_column() {
    let mut chart = RoxChart::new(4);
    chart.notes.push(Note::tap(0, 5)); // Column 5 is invalid for 4K

    let result = RoxCodec::encode(&chart);
    assert!(result.is_err());
}

/// Test roundtrip with full metadata.
#[test]
fn test_roundtrip_full_metadata() {
    let mut chart = RoxChart::new(7);
    chart.metadata = Metadata {
        title: "Complex Song Title (Extended Mix)".into(),
        artist: "Famous Artist feat. Another Artist".into(),
        creator: "Pro Mapper".into(),
        difficulty_name: "GRAVITY".into(),
        difficulty_value: Some(9.87),
        audio_file: "audio/song.ogg".into(),
        background_file: Some("images/bg.jpg".into()),
        audio_offset_us: -15000, // -15ms offset
        preview_time_us: 45_000_000,
        preview_duration_us: 20_000_000,
        source: Some("Game OST".into()),
        genre: Some("Electronic".into()),
        language: Some("JP".into()),
        tags: vec!["stream".into(), "technical".into(), "marathon".into()],
    };

    let encoded = RoxCodec::encode(&chart).expect("Failed to encode");
    let decoded = RoxCodec::decode(&encoded).expect("Failed to decode");

    assert_eq!(chart.metadata, decoded.metadata);
}

/// Test roundtrip preserves timing point details.
#[test]
fn test_roundtrip_timing_points() {
    let mut chart = RoxChart::new(4);
    chart.timing_points.push(TimingPoint::bpm(0, 175.0));
    chart.timing_points.push(TimingPoint::sv(10_000_000, 0.5));
    chart
        .timing_points
        .push(TimingPoint::bpm(30_000_000, 200.0));
    chart.timing_points.push(TimingPoint::sv(60_000_000, 2.0));

    let encoded = RoxCodec::encode(&chart).expect("Failed to encode");
    let decoded = RoxCodec::decode(&encoded).expect("Failed to decode");

    assert_eq!(chart.timing_points.len(), decoded.timing_points.len());
    for (orig, dec) in chart.timing_points.iter().zip(decoded.timing_points.iter()) {
        assert_eq!(orig, dec);
    }
}
