//! Tests of the DJ Hero reader, on files built in memory with the layout of the format notes.

use std::path::Path;

use rox::model::TimingPoint;

use super::{XmkDecoder, names_of};

/// A file: header, the entries `(measure, type, length, extra)`, then `strings`.
fn file(entries: &[(f32, u32, f32, u32)], strings: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for word in [2, 0, u32::try_from(entries.len()).unwrap(), u32::try_from(strings.len()).unwrap()] {
        out.extend(word.to_be_bytes());
    }
    for &(measure, kind, length, extra) in entries {
        out.extend(measure.to_bits().to_be_bytes());
        out.extend(kind.to_be_bytes());
        out.extend(length.to_bits().to_be_bytes());
        out.extend(extra.to_be_bytes());
    }
    out.extend(strings);
    out
}

const TAP: f32 = 1.0 / 32.0;

#[test]
fn the_turntable_is_five_columns_and_the_tempo_comes_from_the_beat_length() {
    // 500 000 µs per beat (120 BPM): a measure is 2 s, until the beat halves at measure 4. The charter's name is the first string, right
    // after the 12 entries (offsets count from the first entry).
    let data = file(
        &[
            (0.0, 0x0B00_0001, 0.0, 500_000),
            (0.0, 0x0AFF_FFFF, 0.0, 12 * 16),
            (0.0, 0, TAP, 0x5F),     // green tap
            (0.5, 2, TAP, 0x5F),     // red tap
            (0.5, 1, TAP, 0x5F),     // blue tap
            (1.0, 4, TAP, 0x5F),     // blue scratch up: a tap
            (1.5, 7, 0.25, 0x5F),    // green scratch, any direction: a hold
            (2.0, 11, 0.5, 0x5F),    // crossfader left: a hold on the left column
            (3.0, 28, 0.03, 0x5F),   // blue spike: a tap on the right column
            (3.0, 10, 1.0, 0x5F),    // centre: no note
            (3.5, 13, 0.5, 0x5F),    // effector: dropped
            (4.0, 0x0B00_0001, 0.0, 250_000),
        ],
        b"djname\0",
    );
    let (chart, report) = XmkDecoder::decode_with_report(&data).unwrap();
    assert_eq!(chart.key_count, 5);
    let notes: Vec<(i64, u8, i64)> = chart.notes.iter().map(|n| (n.time_us, n.column, n.duration_us())).collect();
    assert_eq!(
        notes,
        vec![
            (0, 1, 0),
            (1_000_000, 2, 0),
            (1_000_000, 3, 0),
            (2_000_000, 3, 0),
            (3_000_000, 1, 500_000),
            (4_000_000, 0, 1_000_000),
            (6_000_000, 4, 0),
        ]
    );
    assert_eq!(chart.timing_points, vec![TimingPoint::bpm(0, 120.0), TimingPoint::bpm(8_000_000, 240.0)]);
    assert_eq!(chart.metadata.creator.as_str(), "djname");
    assert_eq!((report.directed_scratches, report.effects), (1, 1));
}

#[test]
fn a_file_with_no_tempo_or_no_note_is_refused_and_the_name_gives_title_and_difficulty() {
    assert!(XmkDecoder::decode_with_report(&file(&[(0.0, 0, TAP, 0)], b"")).unwrap_err().to_string().contains("beat length"));
    let markup = file(&[(0.0, 0x0B00_0001, 0.0, 500_000), (0.0, 0x0300_000D, 1.0, 0)], b"");
    assert!(XmkDecoder::decode_with_report(&markup).unwrap_err().to_string().contains("no playable note"));
    assert_eq!(names_of(Path::new("x/CUS_BINTA_DJ_EXPERT.XMK")), ("CUS_BINTA".to_owned(), "Expert".to_owned()));
    assert_eq!(names_of(Path::new("Markup_Main_P1_4.fsgmub")), ("Markup_Main_P1_4".to_owned(), String::new()));
}
