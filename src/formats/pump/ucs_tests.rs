//! Tests of the UCS reader, on charts written in the test.

use rox::model::{NoteType, TimingPoint};

use super::UcsDecoder;

#[test]
fn blocks_set_the_row_length_and_holds_run_from_m_to_w() {
    // 120 BPM split 2: a row is 0.25 s; the first block's delay puts row 0 at -0.5 s. The second block (60 BPM, split 1, a delay the
    // editors ignore) makes a row a second.
    let ucs = ":Format=1\r\n:Mode=Single\r\n:BPM=120\r\n:Delay=-500\r\n:Beat=4\r\n:Split=2\r\nX...M\r\n....H\r\n.X..W\r\nW....\r\n:Split=1\r\n:BPM=60\r\n:Delay=9999\r\n:Beat=3\r\n..X..\r\n...X.\r\n";
    let (chart, report) = UcsDecoder::decode_with_report(ucs.as_bytes()).unwrap();
    assert_eq!(chart.key_count, 5);
    let notes: Vec<(i64, u8, NoteType)> = chart.notes.iter().map(|n| (n.time_us, n.column, n.note_type)).collect();
    assert_eq!(
        notes,
        [
            (-500_000, 0, NoteType::Tap),
            (-500_000, 4, NoteType::Hold { duration_us: 500_000 }),
            (0, 1, NoteType::Tap),
            (500_000, 2, NoteType::Tap),
            (1_500_000, 3, NoteType::Tap),
        ]
    );
    assert_eq!(report.invalid_notes, 1, "a W without its M");
    assert_eq!(chart.timing_points, [TimingPoint::Bpm { time_us: -500_000, bpm: 120.0, signature: 4 }, TimingPoint::Bpm { time_us: 500_000, bpm: 60.0, signature: 3 }]);
}

#[test]
fn a_file_that_is_not_ucs_is_refused() {
    assert!(UcsDecoder::decode_with_report(b":Format=2\r\n:Mode=Single\r\n").is_err());
    assert!(UcsDecoder::decode_with_report(b":Format=1\r\n:Mode=Triple\r\n").is_err());
    assert!(UcsDecoder::decode_with_report(b":Format=1\r\n:Mode=Double\r\nX.........\r\n").is_err(), "a row before BPM and Split");
}
