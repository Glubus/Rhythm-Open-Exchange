//! Tests of the Codename Engine reader, on charts written in the test.

use rox::model::{NoteType, TimingPoint};
use serde_json::json;

use super::{CodenameDecoder, decode_file, looks_like_codename};

fn chart() -> serde_json::Value {
    json!({
        "events": [
            {"name": "BPM Change", "time": 4000, "params": [150]},
            {"name": "Continuous BPM Change", "time": 6000, "params": [180, 8]},
            {"type": 2, "time": 8000, "params": [120]}
        ],
        "strumLines": [
            {"type": 0, "position": "dad", "notes": [{"time": 0, "id": 0, "type": 0, "sLen": 0}]},
            {"type": 1, "position": "boyfriend", "keyCount": 6, "notes": [
                {"time": 500, "id": 5, "type": 0, "sLen": 0},
                {"time": 1000, "id": 0, "type": 0, "sLen": 250},
                {"time": 1100, "id": 0, "type": 0, "sLen": 0},
                {"time": 1500, "id": 2, "type": 1, "sLen": 0},
                {"time": 2000, "id": 6, "type": 0, "sLen": 0}
            ]},
            {"type": 2, "position": "girlfriend", "notes": [{"time": 0, "id": 1}]}
        ],
        "noteTypes": ["Hurt Note"],
        "codenameChart": true
    })
}

#[test]
fn the_player_lines_are_the_columns_and_events_set_the_tempo() {
    let bytes = serde_json::to_vec(&chart()).unwrap();
    let meta = br#"{"displayName": "Synthetic", "bpm": 120, "beatsPerMeasure": 3}"#;
    let events = br#"{"events": [{"name": "Time Signature Change", "time": 4000, "params": [4, 4]}]}"#;
    let (chart, report) = CodenameDecoder::decode_with_report(&bytes, Some(meta), Some(events)).unwrap();
    assert_eq!(chart.key_count, 6);
    let notes: Vec<(i64, u8, NoteType)> = chart.notes.iter().map(|n| (n.time_us, n.column, n.note_type)).collect();
    assert_eq!(
        notes,
        [(500_000, 5, NoteType::Tap), (1_000_000, 0, NoteType::Hold { duration_us: 250_000 }), (1_500_000, 2, NoteType::Tap)]
    );
    assert_eq!((report.other_strumline_notes, report.invalid_notes, report.overlapping_notes_dropped, report.tempo_ramps_ignored), (2, 1, 1, 1));
    assert_eq!(report.note_types.get("Hurt Note"), Some(&1));
    // The meta's 120 BPM in 3, then at 4 s the BPM change and the signature change of events.json in one point; the old numbered event at 8 s.
    assert_eq!(
        chart.timing_points,
        [
            TimingPoint::Bpm { time_us: 0, bpm: 120.0, signature: 3 },
            TimingPoint::Bpm { time_us: 4_000_000, bpm: 150.0, signature: 4 },
            TimingPoint::Bpm { time_us: 8_000_000, bpm: 120.0, signature: 4 },
        ]
    );
    assert_eq!(chart.metadata.title, "Synthetic");
}

#[test]
fn a_chart_in_its_song_folder_finds_its_meta_and_instrumental() {
    let song = std::env::temp_dir().join(format!("rox-codename-{}", std::process::id())).join("bopeebo");
    let _ = std::fs::remove_dir_all(&song);
    std::fs::create_dir_all(song.join("charts/erect")).unwrap();
    std::fs::create_dir_all(song.join("song")).unwrap();
    let bytes = serde_json::to_vec(&chart()).unwrap();
    std::fs::write(song.join("charts/hard.json"), &bytes).unwrap();
    std::fs::write(song.join("charts/erect/hard.json"), &bytes).unwrap();
    std::fs::write(song.join("meta.json"), br#"{"bpm": 90}"#).unwrap();
    std::fs::write(song.join("meta-erect.json"), br#"{"bpm": 95, "displayName": "Bopeebo Erect", "instSuffix": "-erect"}"#).unwrap();
    std::fs::write(song.join("song/Inst.ogg"), b"").unwrap();
    std::fs::write(song.join("song/Inst-erect-hard.ogg"), b"").unwrap();

    let plain = decode_file(&song.join("charts/hard.json"), &bytes).unwrap();
    assert_eq!((plain.metadata.title.as_str(), plain.metadata.difficulty_name.as_str(), plain.metadata.audio_file.as_str()), ("bopeebo", "hard", "../song/Inst.ogg"));
    assert_eq!(plain.timing_points[0].bpm_value(), Some(90.0));
    let erect = decode_file(&song.join("charts/erect/hard.json"), &bytes).unwrap();
    assert_eq!((erect.metadata.title.as_str(), erect.metadata.audio_file.as_str()), ("Bopeebo Erect", "../../song/Inst-erect-hard.ogg"));
    assert_eq!(erect.timing_points[0].bpm_value(), Some(95.0));
    // Read alone, a chart has the engine's default tempo and no music.
    let alone = decode_file(&std::env::temp_dir().join("x/hard.json"), &bytes).unwrap();
    assert_eq!((alone.timing_points[0].bpm_value(), alone.metadata.audio_file.as_str()), (Some(100.0), ""));
    std::fs::remove_dir_all(song.parent().unwrap()).unwrap();
}

#[test]
fn only_a_json_object_that_sets_codename_chart_is_one() {
    assert!(looks_like_codename(br#"{"strumLines": [], "codenameChart" : true}"#));
    assert!(looks_like_codename(br#" {"codenameChart":"true"}"#));
    assert!(!looks_like_codename(br#"{"codenameChart": false}"#));
    assert!(!looks_like_codename(br#"{"song": {"notes": []}}"#));
    assert!(!looks_like_codename(br#"["codenameChart", true]"#));
    assert!(CodenameDecoder::decode_with_report(br#"{"strumLines": [{"type": 0, "notes": [{"time": 0, "id": 0}]}]}"#, None, None).is_err());
}
