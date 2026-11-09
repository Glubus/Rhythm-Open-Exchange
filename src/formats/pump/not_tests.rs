//! Tests of the NOT reader, on files written in the test.

use rox::model::{NoteType, TimingPoint};

use super::NotDecoder;

/// A `NOT4` file: BPM 120 from 50 cs, 2 lines per beat, then the obfuscated records `(line, mask)` (mask bit 15 = 1P left).
fn not4(records: &[(u16, u16)]) -> Vec<u8> {
    let mut out = vec![0u8; 0x88];
    out[8..12].copy_from_slice(&120f32.to_le_bytes());
    out[24..28].copy_from_slice(&50u32.to_le_bytes());
    out[56..60].copy_from_slice(&2u32.to_le_bytes());
    let mut body = Vec::new();
    for (line, mask) in records {
        body.extend(line.to_le_bytes());
        body.extend(mask.to_le_bytes());
    }
    #[allow(clippy::cast_possible_truncation)]
    out.extend(body.iter().enumerate().map(|(index, byte)| byte ^ (0xD2u32 + 0x95 * (index as u32 % 256)) as u8));
    out
}

#[test]
fn a_not4_reads_its_obfuscated_records_by_line() {
    // 120 BPM, 2 lines per beat: a line is 25 cs. Line 2 holds 1P left (bit 15) and the stage lights (low bits): one step at 50 + 50 cs.
    // Line 4: 1P right (bit 11). A record of stage lights only is no step.
    let data = not4(&[(0, 0b11_1000), (2, 0x8000 | 0b111), (4, 1 << 11)]);
    let (chart, report) = NotDecoder::decode_named(&data, "04_1_.NOT").unwrap();
    assert!(!report.not5);
    assert_eq!(chart.key_count, 5);
    assert_eq!(chart.notes.iter().map(|n| (n.time_us, n.column)).collect::<Vec<_>>(), [(1_000_000, 0), (1_500_000, 4)]);
    assert_eq!(chart.timing_points, [TimingPoint::Bpm { time_us: 1_000_000, bpm: 120.0, signature: 4 }]);
    // A 2P panel (bit 6) or a double name makes it a 10-column chart.
    assert_eq!(NotDecoder::decode_named(&not4(&[(2, 1 << 6)]), "x.NOT").unwrap().0.notes[0].column, 9);
    assert_eq!(NotDecoder::decode_named(&data, "04_XD.NOT").unwrap().0.key_count, 10);
}

#[test]
fn a_not5_reads_steps_and_holds_from_its_three_arrays() {
    let lines = 4usize;
    let mut data = b"pump 5.0".to_vec();
    data.resize(0xD8, 0);
    data[10..14].copy_from_slice(&u32::try_from(lines).unwrap().to_le_bytes());
    data[16..20].copy_from_slice(&60f32.to_le_bytes());
    data[136..140].copy_from_slice(&1u32.to_le_bytes());
    // 60 BPM, 1 line per beat: a line is 100 cs. Steps: line 0 the 1P left (bit 9); a hold on bit 4 (2P left: column 5) from line 1 to
    // line 3.
    let steps: [u16; 4] = [1 << 9, 1 << 4, 1 << 4, 1 << 4];
    let starts: [u16; 4] = [0, 1 << 4, 0, 0];
    let ends: [u16; 4] = [0, 0, 0, 1 << 4];
    for array in [steps, starts, ends] {
        for mask in array {
            data.extend(mask.to_le_bytes());
        }
    }
    let (chart, report) = NotDecoder::decode_named(&data, "song.not").unwrap();
    assert!(report.not5);
    assert_eq!(chart.key_count, 10);
    let notes: Vec<(i64, u8, NoteType)> = chart.notes.iter().map(|n| (n.time_us, n.column, n.note_type)).collect();
    assert_eq!(notes, [(0, 0, NoteType::Tap), (1_000_000, 5, NoteType::Hold { duration_us: 2_000_000 })]);
    assert_eq!(report.broken_holds, 0);
}

#[test]
fn a_file_without_a_step_is_refused() {
    assert!(NotDecoder::decode_named(&not4(&[(0, 0b111)]), "a.NOT").is_err());
    assert!(NotDecoder::decode_named(b"pump 5.0", "a.not").is_err());
}
