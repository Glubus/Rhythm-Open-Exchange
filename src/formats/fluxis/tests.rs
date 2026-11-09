//! Tests of the fluXis reader, on maps written in the test.

use rox::model::{NoteType, TimingPoint};
use serde_json::json;

use super::FscDecoder;

#[test]
fn lanes_holds_ticks_mines_and_timing_are_read_as_the_game_writes_them() {
    let map = json!({
        "AudioFile": "audio.mp3",
        "BackgroundFile": "",
        "CoverFile": "cover.png",
        "metadata": {"title": "Life", "artist": "ARForest", "mapper": "m", "difficulty": "Beginner", "tags": "calm, melodic", "previewtime": 1500},
        "HitObjects": [
            {"time": 500.0, "lane": 4, "type": 0},
            {"time": 1000.0, "lane": 1, "holdtime": 250.0},
            {"time": 1100.0, "lane": 1, "type": 0},
            {"time": 1500.0, "lane": 2, "type": 1},
            {"time": 2000.0, "lane": 3, "type": 2},
            {"time": 2500.0, "lane": 0}
        ],
        "TimingPoints": [{"time": 0.0, "bpm": 150.0, "signature": 4}, {"time": 2000.0, "bpm": 150.0, "signature": 3}],
        "ScrollVelocities": [{"time": 1000.0, "multiplier": 0.5}]
    });
    let (chart, report) = FscDecoder::decode_with_report(&serde_json::to_vec(&map).unwrap()).unwrap();
    assert_eq!(chart.key_count, 4);
    let notes: Vec<(i64, u8, NoteType)> = chart.notes.iter().map(|n| (n.time_us, n.column, n.note_type)).collect();
    assert_eq!(
        notes,
        vec![
            (500_000, 3, NoteType::Tap),
            (1_000_000, 0, NoteType::Hold { duration_us: 250_000 }),
            (1_500_000, 1, NoteType::Tap),
            (2_000_000, 2, NoteType::Mine),
        ]
    );
    assert_eq!((report.ticks, report.invalid_notes, report.overlapping_notes_dropped), (1, 1, 1));
    assert_eq!(
        chart.timing_points,
        vec![
            TimingPoint::Bpm { time_us: 0, bpm: 150.0, signature: 4 },
            TimingPoint::sv(1_000_000, 0.5),
            TimingPoint::Bpm { time_us: 2_000_000, bpm: 150.0, signature: 3 },
        ]
    );
    assert_eq!((chart.metadata.title.as_str(), chart.metadata.creator.as_str()), ("Life", "m"));
    assert_eq!(chart.metadata.background_file.as_deref(), Some("cover.png"));
    assert_eq!(chart.metadata.preview_time_us, 1_500_000);
}

#[test]
fn a_map_without_a_note_or_that_is_not_json_is_refused() {
    assert!(FscDecoder::decode_with_report(br#"{"HitObjects": []}"#).unwrap_err().to_string().contains("no playable note"));
    assert!(FscDecoder::decode_with_report(b"not json").is_err());
}
