//! Full roundtrip integration test for osu!mania format.
//!
//! Tests: .osu -> RoxChart -> .osu and verifies data integrity.

use rhythm_open_exchange::RoxCodec;
use rhythm_open_exchange::codec::formats::osu::{OsuDecoder, OsuEncoder};
use rhythm_open_exchange::codec::{Decoder, Encoder};

fn roundtrip_test(path: &str) {
    // Load original .osu
    let original = std::fs::read(path).expect("Failed to read .osu");

    // Decode to RoxChart
    let chart1 = OsuDecoder::decode(&original).expect("Failed to decode .osu");

    // Encode to ROX
    let rox_data = RoxCodec::encode(&chart1).expect("Failed to encode to ROX");

    // Decode ROX back
    let chart2 = RoxCodec::decode(&rox_data).expect("Failed to decode ROX");

    // Verify ROX roundtrip
    assert_eq!(chart1.key_count(), chart2.key_count());
    assert_eq!(chart1.notes.len(), chart2.notes.len());
    assert_eq!(chart1.timing_points.len(), chart2.timing_points.len());

    // Encode to .osu
    let osu_data = OsuEncoder::encode(&chart2).expect("Failed to encode to .osu");

    // Decode .osu again
    let chart3 = OsuDecoder::decode(&osu_data).expect("Failed to decode re-encoded .osu");

    // Verify full roundtrip
    assert_eq!(chart1.key_count(), chart3.key_count(), "Key count mismatch");
    assert_eq!(
        chart1.notes.len(),
        chart3.notes.len(),
        "Note count mismatch"
    );
    assert_eq!(
        chart1.timing_points.len(),
        chart3.timing_points.len(),
        "Timing points mismatch"
    );

    // Verify note timestamps
    for (i, (n1, n3)) in chart1.notes.iter().zip(chart3.notes.iter()).enumerate() {
        assert!(
            (n1.time_us - n3.time_us).abs() <= 1000,
            "Note {} time mismatch: {} vs {}",
            i,
            n1.time_us,
            n3.time_us
        );
        assert_eq!(n1.column, n3.column, "Note {} column mismatch", i);
    }
}

#[test]
fn test_roundtrip_4k() {
    roundtrip_test("assets/osu/mania_4k.osu");
}

#[test]
fn test_roundtrip_7k() {
    roundtrip_test("assets/osu/mania_7k.osu");
}

#[test]
fn test_rox_compression() {
    let data = std::fs::read("assets/osu/mania_7k.osu").unwrap();
    let chart = OsuDecoder::decode(&data).unwrap();
    let rox_data = RoxCodec::encode(&chart).unwrap();

    // ROX should be much smaller than .osu
    assert!(
        rox_data.len() < data.len() / 10,
        "ROX {} bytes should be much smaller than .osu {} bytes",
        rox_data.len(),
        data.len()
    );
}
