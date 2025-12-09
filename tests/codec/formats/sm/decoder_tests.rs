//! Tests for StepMania (.sm) decoder.

use rhythm_open_exchange::Decoder;
use rhythm_open_exchange::codec::formats::sm::SmDecoder;

/// Basic SM file content for testing.
const BASIC_SM: &str = r#"
#TITLE:Test Song;
#ARTIST:Test Artist;
#CREDIT:Test Mapper;
#MUSIC:song.ogg;
#OFFSET:0;
#BPMS:0=120;
#STOPS:;

#NOTES:
     dance-single:
     :
     Beginner:
     1:
     0,0,0,0,0:
0000
1000
0100
0010
,
0001
0000
0000
0000
;
"#;

#[test]
fn test_decode_basic_sm() {
    let chart = SmDecoder::decode(BASIC_SM.as_bytes()).expect("Failed to decode");

    assert_eq!(chart.key_count, 4);
    assert_eq!(chart.metadata.title, "Test Song");
    assert_eq!(chart.metadata.artist, "Test Artist");
    assert_eq!(chart.metadata.creator, "Test Mapper");
    assert_eq!(chart.metadata.difficulty_name, "Beginner");
    assert!(!chart.notes.is_empty());
}

#[test]
fn test_sm_note_count() {
    let chart = SmDecoder::decode(BASIC_SM.as_bytes()).expect("Failed to decode");

    // 4 tap notes: 1 per column across 2 measures
    assert_eq!(chart.notes.len(), 4);
}

#[test]
fn test_sm_timing_points() {
    let chart = SmDecoder::decode(BASIC_SM.as_bytes()).expect("Failed to decode");

    // Should have at least one BPM timing point
    assert!(!chart.timing_points.is_empty());
    assert_eq!(chart.timing_points[0].bpm, 120.0);
}
