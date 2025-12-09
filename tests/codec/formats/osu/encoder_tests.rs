//! Tests for osu encoder (RoxChart -> .osu).

use rhythm_open_exchange::codec::formats::osu::{OsuDecoder, OsuEncoder, column_to_x};
use rhythm_open_exchange::codec::{Decoder, Encoder};
use rhythm_open_exchange::model::{Note, RoxChart, TimingPoint};

/// Helper to verify all columns for a key count
fn verify_columns(key_count: u8, expected: &[i32]) {
    assert_eq!(
        expected.len(),
        key_count as usize,
        "Wrong number of expected values for {}K",
        key_count
    );
    for (col, &expected_x) in expected.iter().enumerate() {
        let actual = column_to_x(col as u8, key_count);
        assert_eq!(
            actual, expected_x,
            "{}K column {} failed: expected {}, got {}",
            key_count, col, expected_x, actual
        );
    }
}

#[test]
fn test_column_to_x_4k() {
    verify_columns(4, &[64, 192, 320, 448]);
}

#[test]
fn test_column_to_x_5k() {
    verify_columns(5, &[51, 153, 256, 358, 460]);
}

#[test]
fn test_column_to_x_6k() {
    verify_columns(6, &[42, 128, 213, 298, 384, 469]);
}

#[test]
fn test_column_to_x_7k() {
    verify_columns(7, &[36, 109, 182, 256, 329, 402, 475]);
}

#[test]
fn test_column_to_x_8k() {
    verify_columns(8, &[32, 96, 160, 224, 288, 352, 416, 480]);
}

#[test]
fn test_column_to_x_9k() {
    verify_columns(9, &[28, 85, 142, 199, 256, 312, 369, 426, 483]);
}

#[test]
fn test_column_to_x_10k() {
    verify_columns(10, &[25, 76, 128, 179, 230, 281, 332, 384, 435, 486]);
}

#[test]
fn test_column_to_x_12k() {
    verify_columns(
        12,
        &[21, 64, 106, 149, 192, 234, 277, 320, 362, 405, 448, 490],
    );
}

#[test]
fn test_column_to_x_14k() {
    verify_columns(
        14,
        &[
            18, 54, 91, 128, 164, 201, 237, 274, 310, 347, 384, 420, 457, 493,
        ],
    );
}

#[test]
fn test_column_to_x_16k() {
    verify_columns(
        16,
        &[
            16, 48, 80, 112, 144, 176, 208, 240, 272, 304, 336, 368, 400, 432, 464, 496,
        ],
    );
}

#[test]
fn test_column_to_x_18k() {
    verify_columns(
        18,
        &[
            14, 42, 71, 99, 128, 156, 184, 213, 241, 270, 298, 327, 355, 384, 412, 440, 469, 497,
        ],
    );
}

#[test]
fn test_column_roundtrip() {
    for key_count in [4, 5, 6, 7, 8, 9, 10] {
        for col in 0..key_count {
            let x = column_to_x(col, key_count);
            let decoded_col = ((x * key_count as i32) / 512) as u8;
            assert_eq!(
                decoded_col, col,
                "Roundtrip failed for {}K column {}",
                key_count, col
            );
        }
    }
}

#[test]
fn test_encode_basic() {
    let mut chart = RoxChart::new(7);
    chart.metadata.title = "Test".into();
    chart.metadata.artist = "Artist".into();
    chart.metadata.creator = "Mapper".into();
    chart.metadata.difficulty_name = "Hard".into();
    chart.metadata.audio_file = "audio.mp3".into();
    chart.timing_points.push(TimingPoint::bpm(0, 180.0));
    chart.notes.push(Note::tap(1_000_000, 0));
    chart.notes.push(Note::tap(1_500_000, 3));
    chart.notes.push(Note::hold(2_000_000, 500_000, 6));

    let encoded = OsuEncoder::encode(&chart).unwrap();
    let output = String::from_utf8_lossy(&encoded);

    assert!(output.contains("osu file format v14"));
    assert!(output.contains("Mode: 3"));
    assert!(output.contains("CircleSize:7"));
}

#[test]
fn test_roundtrip() {
    let data = include_bytes!("../../../../assets/osu/mania_7k.osu");
    let chart1 = OsuDecoder::decode(data).unwrap();
    let encoded = OsuEncoder::encode(&chart1).unwrap();
    let chart2 = OsuDecoder::decode(&encoded).unwrap();

    assert_eq!(chart1.key_count, chart2.key_count);
    assert_eq!(chart1.notes.len(), chart2.notes.len());
}
