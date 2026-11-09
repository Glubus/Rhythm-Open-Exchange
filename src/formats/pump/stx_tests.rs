//! Tests of the STX reader, on files written in the test.

use std::io::Write;

use rox::model::{NoteType, TimingPoint};

use super::StxDecoder;

/// A division: BPM, beats per measure, lines per beat, delay (cs), lines of cells (padded to 13 bytes), zlib-compressed with its length.
fn division(bpm: f32, beat: u32, split: u32, delay: i32, lines: &[&[u8]]) -> Vec<u8> {
    let mut raw = vec![0u8; 132];
    raw[..4].copy_from_slice(&bpm.to_le_bytes());
    raw[4..8].copy_from_slice(&beat.to_le_bytes());
    raw[8..12].copy_from_slice(&split.to_le_bytes());
    raw[12..16].copy_from_slice(&delay.to_le_bytes());
    raw[128..132].copy_from_slice(&u32::try_from(lines.len()).unwrap().to_le_bytes());
    for line in lines {
        let mut cells = line.to_vec();
        cells.resize(13, 0);
        raw.extend(cells);
    }
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&raw).unwrap();
    let stream = encoder.finish().unwrap();
    let mut out = u32::try_from(stream.len()).unwrap().to_le_bytes().to_vec();
    out.extend(stream);
    out
}

/// A chart block: meter, the division counts of the blocks, the divisions.
fn chart(meter: i32, blocks: &[Vec<Vec<u8>>]) -> Vec<u8> {
    let mut out = meter.to_le_bytes().to_vec();
    for index in 0..50 {
        out.extend(u32::try_from(blocks.get(index).map_or(0, Vec::len)).unwrap().to_le_bytes());
    }
    for block in blocks {
        for division in block {
            out.extend(division);
        }
    }
    out
}

/// A file of nine charts (the given ones, then empty charts).
fn file(title: &str, charts: &[Vec<u8>]) -> Vec<u8> {
    let mut out = b"STF4".to_vec();
    out.resize(288, 0);
    out[60..60 + title.len()].copy_from_slice(title.as_bytes());
    let empty = chart(0, &[]);
    let mut at = 288;
    let mut body: Vec<u8> = Vec::new();
    for index in 0..9 {
        let chart = charts.get(index).unwrap_or(&empty);
        out[252 + 4 * index..256 + 4 * index].copy_from_slice(&u32::try_from(at).unwrap().to_le_bytes());
        at += chart.len();
        body.extend(chart);
    }
    out.extend(body);
    out
}

#[test]
fn divisions_chain_and_holds_run_from_10_to_12() {
    // Practice is empty; Normal: 120 BPM, 2 lines per beat (0.25 s), delay -10 cs; then a 60 BPM division (its delay is not applied).
    // Its second block holds two alternative divisions (Division mode): only the first is read.
    let normal = chart(
        3,
        &[
            vec![division(120.0, 4, 2, -10, &[&[1, 0, 0, 0, 10], &[0, 0, 0, 0, 11], &[0, 2, 0, 0, 12], &[0, 0, 12, 0, 0]])],
            vec![division(60.0, 3, 1, 500, &[&[0, 0, 0, 1, 0]]), division(60.0, 3, 1, 0, &[&[1, 1, 1, 1, 1]])],
        ],
    );
    let data = file("Song", &[chart(0, &[]), normal]);
    let variants = StxDecoder::variants(&data).unwrap();
    assert_eq!(variants.iter().map(|v| (v.index, v.mode, v.meter)).collect::<Vec<_>>(), [(0, "Normal", 3)]);
    let (chart, report) = StxDecoder::decode_with_report(&data, 0).unwrap();
    let notes: Vec<(i64, u8, NoteType)> = chart.notes.iter().map(|n| (n.time_us, n.column, n.note_type)).collect();
    assert_eq!(
        notes,
        [(-100_000, 0, NoteType::Tap), (-100_000, 4, NoteType::Hold { duration_us: 500_000 }), (400_000, 1, NoteType::Tap), (900_000, 3, NoteType::Tap)]
    );
    assert_eq!((report.special_steps, report.invalid_notes, report.alternative_divisions), (1, 1, 1));
    assert_eq!(chart.timing_points, [TimingPoint::Bpm { time_us: -100_000, bpm: 120.0, signature: 4 }, TimingPoint::Bpm { time_us: 900_000, bpm: 60.0, signature: 3 }]);
    assert_eq!((chart.metadata.title.as_str(), chart.metadata.difficulty_name.as_str()), ("Song", "Normal"));
}

#[test]
fn a_file_that_is_not_stf4_is_refused() {
    assert!(StxDecoder::variants(b"STF3").is_err());
    assert!(StxDecoder::decode_variant(&file("x", &[]), 0).is_err(), "no playable chart");
}
