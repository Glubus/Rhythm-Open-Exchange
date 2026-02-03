use super::RoxCodec;
use super::MAX_FILE_SIZE;
use crate::codec::{Decoder, Encoder};
use crate::error::RoxError;
use crate::model::{Hitsound, Metadata, Note, RoxChart, TimingPoint, ROX_MAGIC};

#[test]
fn test_roundtrip() {
    let mut chart = RoxChart::new(4);
    chart.metadata = Metadata {
        title: "Test Song".into(),
        artist: "Test Artist".into(),
        creator: "Mapper".into(),
        difficulty_name: "Hard".into(),
        difficulty_value: Some(5.5),
        audio_file: "audio.mp3".into(),
        background_file: Some("bg.png".into()),
        preview_time_us: 30_000_000, // 30 seconds
        ..Default::default()
    };
    chart.timing_points.push(TimingPoint::bpm(0, 180.0));
    chart.timing_points.push(TimingPoint::sv(60_000_000, 1.5));
    chart.notes.push(Note::tap(1_000_000, 0));
    chart.notes.push(Note::tap(1_500_000, 1));
    chart.notes.push(Note::hold(2_000_000, 1_000_000, 2)); // 1s duration

    // Encode
    let encoded = RoxCodec::encode(&chart).expect("Failed to encode");

    // Check magic bytes
    assert_eq!(&encoded[..4], &ROX_MAGIC);

    // Decode
    let decoded = RoxCodec::decode(&encoded).expect("Failed to decode");

    // Verify equality
    assert_eq!(chart, decoded);
}

#[test]
fn test_invalid_magic() {
    let bad_data = [0x00, 0x00, 0x00, 0x00, 0x01];
    let result = RoxCodec::decode(&bad_data);
    assert!(result.is_err());
}

#[test]
fn test_compression_reduces_size() {
    let mut chart = RoxChart::new(4);
    chart.timing_points.push(TimingPoint::bpm(0, 120.0));

    // Add many similar notes (compresses well)
    for i in 0i64..1000 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let column = (i % 4) as u8;
        chart.notes.push(Note::tap(i * 100_000, column));
    }

    let encoded = RoxCodec::encode(&chart).expect("Failed to encode");

    // With delta encoding, compression should be excellent
    assert!(
        encoded.len() < 2000,
        "Compressed size {} is larger than expected",
        encoded.len()
    );
}

#[test]
fn test_delta_encoding() {
    let mut chart = RoxChart::new(4);
    chart.timing_points.push(TimingPoint::bpm(0, 120.0));
    chart.notes.push(Note::tap(1_000_000, 0));
    chart.notes.push(Note::tap(1_100_000, 1));
    chart.notes.push(Note::tap(1_200_000, 2));
    chart.notes.push(Note::tap(1_300_000, 3));

    let encoded = RoxCodec::encode(&chart).expect("Failed to encode");
    let decoded = RoxCodec::decode(&encoded).expect("Failed to decode");

    // Verify timestamps are correctly restored
    assert_eq!(decoded.notes[0].time_us, 1_000_000);
    assert_eq!(decoded.notes[1].time_us, 1_100_000);
    assert_eq!(decoded.notes[2].time_us, 1_200_000);
    assert_eq!(decoded.notes[3].time_us, 1_300_000);
}

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

#[test]
fn test_magic_bytes() {
    let chart = RoxChart::new(4);
    let encoded = RoxCodec::encode(&chart).expect("Failed to encode");

    assert!(encoded.len() >= 4);
    assert_eq!(&encoded[..4], &[0x52, 0x4F, 0x58, 0x00]); // "ROX\0"
}

#[test]
fn test_invalid_magic_bytes() {
    let bad_data = [0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03];
    let result = RoxCodec::decode(&bad_data);

    assert!(result.is_err());
}

#[test]
fn test_decode_empty_data() {
    let result = RoxCodec::decode(&[]);
    assert!(result.is_err());
}

#[test]
fn test_decode_short_data() {
    let result = RoxCodec::decode(&[0x52, 0x4F]);
    assert!(result.is_err());
}

#[test]
fn test_encode_empty_chart() {
    let chart = RoxChart::new(7);
    let encoded = RoxCodec::encode(&chart);

    assert!(encoded.is_ok());
    let data = encoded.unwrap();
    assert!(data.len() >= 4);
}

#[test]
fn test_encode_invalid_column() {
    let mut chart = RoxChart::new(4);
    chart.notes.push(Note::tap(0, 5)); // Column 5 is invalid for 4K

    let result = RoxCodec::encode(&chart);
    assert!(result.is_err());
}

#[test]
fn test_roundtrip_full_metadata() {
    let mut chart = RoxChart::new(7);
    chart.metadata = Metadata {
        key_count: 7,
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
        is_coop: false,
        ..Default::default()
    };

    let encoded = RoxCodec::encode(&chart).expect("Failed to encode");
    let decoded = RoxCodec::decode(&encoded).expect("Failed to decode");

    assert_eq!(chart.metadata, decoded.metadata);
}

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

// Helper for building complex charts
fn create_complex_chart() -> RoxChart {
    // Create a realistic 7K chart
    let mut chart = RoxChart::new(7);

    // Set up metadata
    chart.metadata = Metadata {
        key_count: 7,
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
        is_coop: false,
        ..Default::default()
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

    chart
}

#[test]
fn test_complex_chart_creation() {
    let chart = create_complex_chart();
    // Validate
    assert!(chart.validate().is_ok());

    // Verify specific fields
    assert_eq!(chart.key_count(), 7);
    assert_eq!(chart.metadata.title, "Galaxy Collapse");
    assert_eq!(chart.timing_points.len(), 4);
    assert_eq!(chart.notes.len(), 12); // 7 taps + 2 holds + 1 burst + 2 mines
}

#[test]
fn test_complex_chart_encoding_size() {
    let chart = create_complex_chart();
    // Encode
    let encoded = RoxCodec::encode(&chart).expect("Encoding failed");

    // Verify reasonable size (should be compact)
    assert!(
        encoded.len() < 2000,
        "Encoded size unexpectedly large: {}",
        encoded.len()
    );
}

#[test]
fn test_complex_chart_roundtrip() {
    let chart = create_complex_chart();
    let encoded = RoxCodec::encode(&chart).expect("Encoding failed");

    // Decode
    let decoded = RoxCodec::decode(&encoded).expect("Decoding failed");

    // Verify full equality
    assert_eq!(chart, decoded);
}

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

    // Sort notes
    chart.notes.sort_by_key(|n| n.time_us);

    // Encode and decode
    let encoded = RoxCodec::encode(&chart).expect("Encoding failed");
    let decoded = RoxCodec::decode(&encoded).expect("Decoding failed");

    // Verify hitsounds preserved
    assert_eq!(decoded.hitsounds.len(), 4);
    assert_eq!(decoded.hitsounds[2].volume, Some(60));

    // Verify note-hitsound links
    // Sorted order: Kick (0), Hihat (1), Snare (2), Tap (3)
    assert_eq!(decoded.notes[0].hitsound_index, Some(0)); // Kick (t=0)
    assert_eq!(decoded.notes[1].hitsound_index, Some(2)); // Hihat (t=250k)
    assert_eq!(decoded.notes[2].hitsound_index, Some(1)); // Snare (t=500k)
    assert!(decoded.notes[3].hitsound_index.is_none()); // Tap (t=750k)
}

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

#[test]
fn test_various_key_counts() {
    for key_count in [1, 4, 5, 6, 7, 8, 9, 10, 18] {
        let mut chart = RoxChart::new(key_count);

        // Add one note per column
        for col in 0..key_count {
            chart.notes.push(Note::tap(col as i64 * 100_000, col));
        }

        chart.timing_points.push(TimingPoint::bpm(0, 120.0));

        assert!(chart.validate().is_ok());

        let encoded = RoxCodec::encode(&chart).expect("Encoding failed");
        let decoded = RoxCodec::decode(&encoded).expect("Decoding failed");

        assert_eq!(decoded.key_count(), key_count);
        assert_eq!(decoded.notes.len(), key_count as usize);
    }
}

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

#[test]
fn test_file_too_large() {
    // Create a header that looks valid until the size check hits
    let mut big_data = Vec::with_capacity(MAX_FILE_SIZE + 1);
    big_data.extend_from_slice(&ROX_MAGIC);
    big_data.resize(MAX_FILE_SIZE + 1, 0);

    let result = RoxCodec::decode(&big_data);
    assert!(matches!(result, Err(RoxError::InvalidFormat(msg)) if msg.contains("File too large")));
}
