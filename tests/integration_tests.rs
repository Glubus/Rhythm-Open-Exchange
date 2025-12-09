//! Integration tests for full workflows.

use rhythm_open_exchange::{
    Decoder, Encoder, Hitsound, Metadata, Note, RoxChart, RoxCodec, TimingPoint,
};

/// Test creating a complex, realistic chart.
#[test]
fn test_complex_chart_workflow() {
    // Create a realistic 7K chart
    let mut chart = RoxChart::new(7);

    // Set up metadata
    chart.metadata = Metadata {
        title: "Galaxy Collapse".into(),
        artist: "Kurokotei".into(),
        creator: "Shoegazer".into(),
        difficulty_name: "Cataclysmic Hypernova".into(),
        difficulty_value: Some(9.99),
        audio_file: "audio.ogg".into(),
        background_file: Some("bg.jpg".into()),
        audio_offset_us: -5000,
        preview_time_us: 60_000_000,
        preview_duration_us: 20_000_000,
        source: Some("BMS".into()),
        genre: Some("Speedcore".into()),
        language: Some("JP".into()),
        tags: vec!["marathon".into(), "stream".into(), "dump".into()],
    };

    // Add timing points with BPM changes
    chart.timing_points.push(TimingPoint::bpm(0, 270.0));
    chart.timing_points.push(TimingPoint::sv(30_000_000, 0.8));
    chart
        .timing_points
        .push(TimingPoint::bpm(60_000_000, 300.0));
    chart.timing_points.push(TimingPoint::sv(90_000_000, 1.2));

    // Add various notes across all columns
    for col in 0..7u8 {
        chart
            .notes
            .push(Note::tap(1_000_000 + (col as i64 * 50_000), col));
    }

    // Add some holds
    chart.notes.push(Note::hold(5_000_000, 500_000, 0));
    chart.notes.push(Note::hold(5_000_000, 500_000, 6));

    // Add a burst
    chart.notes.push(Note::burst(10_000_000, 200_000, 3));

    // Add mines to avoid
    chart.notes.push(Note::mine(15_000_000, 1));
    chart.notes.push(Note::mine(15_000_000, 5));

    // Validate
    assert!(chart.validate().is_ok());

    // Encode
    let encoded = RoxCodec::encode(&chart).expect("Encoding failed");

    // Verify reasonable size (should be compact)
    assert!(
        encoded.len() < 2000,
        "Encoded size unexpectedly large: {}",
        encoded.len()
    );

    // Decode
    let decoded = RoxCodec::decode(&encoded).expect("Decoding failed");

    // Verify full equality
    assert_eq!(chart, decoded);

    // Verify specific fields
    assert_eq!(decoded.key_count, 7);
    assert_eq!(decoded.metadata.title, "Galaxy Collapse");
    assert_eq!(decoded.timing_points.len(), 4);
    assert_eq!(decoded.notes.len(), 12); // 7 taps + 2 holds + 1 burst + 2 mines
}

/// Test keysounded chart workflow.
#[test]
fn test_keysounded_chart() {
    let mut chart = RoxChart::new(7);
    chart.metadata.title = "BMS Song".into();

    // Add hitsound samples
    chart.hitsounds.push(Hitsound::new("sounds/kick.wav"));
    chart.hitsounds.push(Hitsound::new("sounds/snare.wav"));
    chart
        .hitsounds
        .push(Hitsound::with_volume("sounds/hihat.wav", 60));
    chart.hitsounds.push(Hitsound::new("sounds/piano_c4.wav"));

    chart.timing_points.push(TimingPoint::bpm(0, 140.0));

    // Create notes with keysounds
    let mut kick = Note::tap(0, 0);
    kick.hitsound_index = Some(0);
    chart.notes.push(kick);

    let mut snare = Note::tap(500_000, 3);
    snare.hitsound_index = Some(1);
    chart.notes.push(snare);

    let mut hihat = Note::tap(250_000, 6);
    hihat.hitsound_index = Some(2);
    chart.notes.push(hihat);

    // Note without keysound
    chart.notes.push(Note::tap(750_000, 1));

    // Encode and decode
    let encoded = RoxCodec::encode(&chart).expect("Encoding failed");
    let decoded = RoxCodec::decode(&encoded).expect("Decoding failed");

    // Verify hitsounds preserved
    assert_eq!(decoded.hitsounds.len(), 4);
    assert_eq!(decoded.hitsounds[2].volume, Some(60));

    // Verify note-hitsound links
    assert_eq!(decoded.notes[0].hitsound_index, Some(0));
    assert_eq!(decoded.notes[1].hitsound_index, Some(1));
    assert!(decoded.notes[3].hitsound_index.is_none());
}

/// Test hash is deterministic and unique.
#[test]
fn test_hash_determinism() {
    let mut chart = RoxChart::new(4);
    chart.metadata.title = "Test".into();
    chart.notes.push(Note::tap(0, 0));

    let hash1 = chart.hash();

    // Same chart should produce same hash
    let mut chart2 = RoxChart::new(4);
    chart2.metadata.title = "Test".into();
    chart2.notes.push(Note::tap(0, 0));

    let hash2 = chart2.hash();
    assert_eq!(hash1, hash2);

    // Different chart should produce different hash
    chart2.notes.push(Note::tap(1_000_000, 1));
    let hash3 = chart2.hash();
    assert_ne!(hash1, hash3);
}

/// Test edge case: many notes.
#[test]
fn test_many_notes() {
    let mut chart = RoxChart::new(4);
    chart.timing_points.push(TimingPoint::bpm(0, 200.0));

    // Add 1000 notes
    for i in 0..1000 {
        chart.notes.push(Note::tap(i * 100_000, (i % 4) as u8));
    }

    assert_eq!(chart.note_count(), 1000);
    assert_eq!(chart.duration_us(), 999 * 100_000);

    // Should still encode/decode correctly
    let encoded = RoxCodec::encode(&chart).expect("Encoding failed");
    let decoded = RoxCodec::decode(&encoded).expect("Decoding failed");

    assert_eq!(decoded.notes.len(), 1000);
}

/// Test various key counts.
#[test]
fn test_various_key_counts() {
    for key_count in [1, 4, 5, 6, 7, 8, 9, 10, 18] {
        let mut chart = RoxChart::new(key_count);

        // Add one note per column
        for col in 0..key_count {
            chart.notes.push(Note::tap(col as i64 * 100_000, col));
        }

        assert!(chart.validate().is_ok());

        let encoded = RoxCodec::encode(&chart).expect("Encoding failed");
        let decoded = RoxCodec::decode(&encoded).expect("Decoding failed");

        assert_eq!(decoded.key_count, key_count);
        assert_eq!(decoded.notes.len(), key_count as usize);
    }
}

/// Test negative timing values.
#[test]
fn test_negative_timing() {
    let mut chart = RoxChart::new(4);
    chart.metadata.audio_offset_us = -50_000; // -50ms offset
    chart.timing_points.push(TimingPoint::bpm(-500_000, 120.0)); // BPM before audio start
    chart.notes.push(Note::tap(-100_000, 0)); // Note before audio start

    let encoded = RoxCodec::encode(&chart).expect("Encoding failed");
    let decoded = RoxCodec::decode(&encoded).expect("Decoding failed");

    assert_eq!(decoded.metadata.audio_offset_us, -50_000);
    assert_eq!(decoded.timing_points[0].time_us, -500_000);
    assert_eq!(decoded.notes[0].time_us, -100_000);
}
