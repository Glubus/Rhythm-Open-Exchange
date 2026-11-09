//! Tests of the Interlude reader, on charts written in the test.

use rox::model::{NoteType, TimingPoint};

use super::YavDecoder;

/// A headless chart: rows `(ms, [(column, type)])`, BPM points `(ms, meter, ms per beat)`, scroll velocities `(ms, multiplier)`.
fn headless(rows: &[(f32, &[(u8, u8)])], bpms: &[(f32, i32, f32)], svs: &[(f32, f32)]) -> Vec<u8> {
    let mut out = i32::try_from(rows.len()).unwrap().to_le_bytes().to_vec();
    for (time, notes) in rows {
        out.extend(time.to_le_bytes());
        out.extend(notes.iter().fold(0u16, |mask, (column, _)| mask | 1 << column).to_le_bytes());
        let mut sorted = notes.to_vec();
        sorted.sort_unstable();
        out.extend(sorted.iter().map(|(_, kind)| kind));
    }
    out.extend(i32::try_from(bpms.len()).unwrap().to_le_bytes());
    for (time, meter, ms_per_beat) in bpms {
        out.extend(time.to_le_bytes());
        out.extend(meter.to_le_bytes());
        out.extend(ms_per_beat.to_le_bytes());
    }
    out.extend(i32::try_from(svs.len()).unwrap().to_le_bytes());
    for (time, multiplier) in svs {
        out.extend(time.to_le_bytes());
        out.extend(multiplier.to_le_bytes());
    }
    out
}

#[test]
fn rows_holds_bpms_and_velocities_are_read() {
    // The fiche's example layout: a tap, then a tap and a hold head, the hold's body and tail; a stray tail on column 2.
    let rows: [(f32, &[(u8, u8)]); 4] = [(0.0, &[(0, 1)]), (500.0, &[(1, 1), (3, 2)]), (750.0, &[(3, 3), (2, 4)]), (1000.0, &[(3, 4)])];
    let data = headless(&rows, &[(0.0, 4, 500.0), (1000.0, 3, 400.0)], &[(0.0, 1.0), (500.0, 1.5)]);
    let (chart, report) = YavDecoder::decode_headless(&data, 4).unwrap();
    let notes: Vec<(i64, u8, NoteType)> = chart.notes.iter().map(|n| (n.time_us, n.column, n.note_type)).collect();
    assert_eq!(notes, [(0, 0, NoteType::Tap), (500_000, 1, NoteType::Tap), (500_000, 3, NoteType::Hold { duration_us: 500_000 })]);
    assert_eq!(report.broken_hold_parts, 1);
    assert_eq!(
        chart.timing_points,
        [
            TimingPoint::Bpm { time_us: 0, bpm: 120.0, signature: 4 },
            TimingPoint::sv(0, 1.0),
            TimingPoint::sv(500_000, 1.5),
            TimingPoint::Bpm { time_us: 1_000_000, bpm: 150.0, signature: 3 },
        ]
    );
    // The key count is the caller's: a note beyond it refuses the chart, as the game's reader fails.
    assert!(YavDecoder::decode_headless(&data, 3).is_err());
}

#[test]
fn a_yav_file_is_the_key_count_a_json_header_and_the_chart() {
    let header = r#"{"Title":"Song","Artist":"Someone","Creator":"Me","DiffName":"Hard","PreviewTime":1500,"Tags":["a"],"AudioFile":"audio.mp3"}"#;
    let mut file = vec![4, u8::try_from(header.len()).unwrap()];
    file.extend(header.as_bytes());
    file.extend(headless(&[(250.0, &[(2, 1)])], &[(0.0, 4, 500.0)], &[]));
    let (chart, _) = YavDecoder::decode_with_report(&file).unwrap();
    assert_eq!((chart.key_count, chart.notes.len()), (4, 1));
    assert_eq!((chart.metadata.title.as_str(), chart.metadata.difficulty_name.as_str(), chart.metadata.audio_file.as_str()), ("Song", "Hard", "audio.mp3"));
    assert_eq!(chart.metadata.preview_time_us, 1_500_000);
    assert!(YavDecoder::decode_with_report(&file[..file.len() - 3]).is_err(), "cut short");
}
