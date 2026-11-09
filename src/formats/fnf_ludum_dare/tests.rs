//! Tests of the Ludum Dare reader, on bitmaps written in the test.

use std::io::Write;

use rox::model::{NoteType, TimingPoint};

use super::{LudumDareDecoder, looks_like_ludum_dare};

/// An 8-pixel-wide RGB PNG with a black pixel at each `(row, column)` of `marks` and white elsewhere; rows use the Sub filter, so
/// unfiltering is exercised. The CRCs are not written (the reader does not check them).
fn bitmap(height: usize, marks: &[(usize, usize)]) -> Vec<u8> {
    let mut raw = Vec::new();
    for row in 0..height {
        let pixels: Vec<u8> = (0..8).flat_map(|column| if marks.contains(&(row, column)) { [0, 0, 0] } else { [0xFF; 3] }).collect();
        raw.push(1);
        raw.extend(pixels.iter().enumerate().map(|(at, &byte)| byte.wrapping_sub(if at >= 3 { pixels[at - 3] } else { 0 })));
    }
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&raw).unwrap();
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8], body: &[u8]| {
        png.extend(u32::try_from(body.len()).unwrap().to_be_bytes());
        png.extend(kind);
        png.extend(body);
        png.extend([0; 4]);
    };
    let mut header = 8u32.to_be_bytes().to_vec();
    header.extend(u32::try_from(height).unwrap().to_be_bytes());
    header.extend([8, 2, 0, 0, 0]);
    chunk(b"IHDR", &header);
    chunk(b"IDAT", &encoder.finish().unwrap());
    chunk(b"IEND", &[]);
    png
}

#[test]
fn the_player_repeats_each_section_after_the_opponent_on_the_games_lane_order() {
    let song = br#"{"song": "Test", "bpm": 120, "sections": 2}"#;
    // Section 1, 12 rows (4 beats): lane 1 at step 0, a lane-2 hold piece with no lane-2 note before it, lane 2 at step 2 held one
    // step, then a lane-1 piece after that lane-2 note. Section 2, 32 rows (8 beats): lane 4 at step 0.
    let first = bitmap(12, &[(0, 0), (1, 5), (2, 1), (3, 5), (4, 4)]);
    let second = bitmap(32, &[(0, 3)]);
    let (chart, report) = LudumDareDecoder::decode_with_report(song, &[first, second]).unwrap();
    let notes: Vec<(i64, u8, NoteType)> = chart.notes.iter().map(|n| (n.time_us, n.column, n.note_type)).collect();
    // 120 BPM: a beat is 500 ms, a step 125 ms. The player's half of section 1 starts 4 beats in; section 2 starts after 8 beats, its
    // player's half 8 beats later.
    assert_eq!(
        notes,
        [(2_000_000, 2, NoteType::Tap), (2_250_000, 3, NoteType::Hold { duration_us: 125_000 }), (8_000_000, 0, NoteType::Tap)]
    );
    assert_eq!(report.orphan_hold_pieces, 2);
    assert_eq!(chart.timing_points, [TimingPoint::bpm(0, 120.0)]);
    assert_eq!(chart.metadata.title, "Test");
}

#[test]
fn only_the_three_fields_make_a_song_file() {
    assert!(looks_like_ludum_dare(br#"{"song": "Bopeebo", "bpm": 100, "sections": 15}"#));
    assert!(!looks_like_ludum_dare(br#"{"song": {"song": "Bopeebo", "notes": []}}"#));
    assert!(!looks_like_ludum_dare(br#"{"song": "Bopeebo", "bpm": 100, "sections": 15, "speed": 1}"#));
    assert!(!looks_like_ludum_dare(br#"{"song": "Bopeebo", "bpm": 0, "sections": 15}"#));
    let blank = bitmap(4, &[]);
    assert!(LudumDareDecoder::decode_with_report(br#"{"song": "x", "bpm": 100, "sections": 1}"#, &[blank]).unwrap_err().to_string().contains("no note"));
}
