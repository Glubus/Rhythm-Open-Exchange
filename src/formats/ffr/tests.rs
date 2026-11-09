//! Tests of the FFR level reader, on movies written in the test the way the FFR levels are (one push and one init-array per entry).

use std::io::Write;

use rox::model::{NoteType, TimingPoint};

use super::SwfDecoder;

enum Item<'a> {
    Int(i32),
    /// A double, written high word first as the format wants.
    Double(f64),
    Pool(u8),
    Text(&'a str),
}

fn push(items: &[Item]) -> Vec<u8> {
    let mut payload = Vec::new();
    for item in items {
        match item {
            Item::Int(value) => {
                payload.push(7);
                payload.extend(value.to_le_bytes());
            }
            Item::Double(value) => {
                let bits = value.to_bits();
                payload.push(6);
                #[allow(clippy::cast_possible_truncation)]
                payload.extend(((bits >> 32) as u32).to_le_bytes());
                #[allow(clippy::cast_possible_truncation)]
                payload.extend((bits as u32).to_le_bytes());
            }
            Item::Pool(index) => payload.extend([8, *index]),
            Item::Text(text) => {
                payload.push(0);
                payload.extend(text.as_bytes());
                payload.push(0);
            }
        }
    }
    action(0x96, &payload)
}

fn action(code: u8, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![code, 0, 0];
    bytes[1..3].copy_from_slice(&u16::try_from(payload.len()).unwrap().to_le_bytes());
    bytes.extend(payload);
    bytes
}

/// `_root.<name> = [entries]`, each entry `[items…]`, as the levels write it: entries pushed last first, each one's items last first.
fn assign(name: &str, entries: &[Vec<Item>]) -> Vec<u8> {
    let mut code = action(0x88, &[2, 0, b'_', b'r', b'o', b'o', b't', 0, b'r', b'e', b'd', 0]);
    code.extend(push(&[Item::Pool(0)]));
    code.push(0x1C);
    code.extend(push(&[Item::Text(name)]));
    for entry in entries.iter().rev() {
        let count = i32::try_from(entry.len()).unwrap();
        for item in entry.iter().rev() {
            code.extend(push(std::slice::from_ref(item)));
        }
        code.extend(push(&[Item::Int(count)]));
        code.push(0x42);
    }
    code.extend(push(&[Item::Int(i32::try_from(entries.len()).unwrap())]));
    code.push(0x42);
    code.push(0x4F);
    code.push(0);
    code
}

fn tag(code: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = ((code << 6) | 0x3F).to_le_bytes().to_vec();
    bytes.extend(u32::try_from(payload.len()).unwrap().to_le_bytes());
    bytes.extend(payload);
    bytes
}

/// A movie of the given `DoAction` payloads; `compressed` writes a `CWS`.
fn movie(actions: &[Vec<u8>], compressed: bool) -> Vec<u8> {
    // A RECT of 5-bit fields all zero (5 + 4 × 0 bits: 1 byte), frame rate 30, one frame.
    let mut body = vec![0, 0, 30, 1, 0];
    body.extend(tag(1, &[]));
    for payload in actions {
        body.extend(tag(12, payload));
    }
    body.extend([0, 0]);
    let mut file = if compressed { b"CWS\x06".to_vec() } else { b"FWS\x06".to_vec() };
    file.extend(u32::try_from(body.len() + 8).unwrap().to_le_bytes());
    if compressed {
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&body).unwrap();
        file.extend(encoder.finish().unwrap());
    } else {
        file.extend(body);
    }
    file
}

fn beats() -> Vec<Vec<Item<'static>>> {
    vec![
        vec![Item::Int(30), Item::Text("L"), Item::Pool(1)],
        vec![Item::Double(45.0), Item::Text("R"), Item::Text("blue"), Item::Int(1600)],
        vec![Item::Int(30), Item::Text("L"), Item::Text("blue")],
        vec![Item::Int(60), Item::Text("X"), Item::Text("blue")],
        vec![Item::Int(90), Item::Text("U"), Item::Text("red")],
    ]
}

#[test]
fn a_level_reads_its_beatbox_by_frames_or_milliseconds_on_four_columns() {
    let file = movie(&[vec![0x07, 0x00], assign("beatBox", &beats())], false);
    let (chart, report) = SwfDecoder::decode_with_report(&file).unwrap();
    assert_eq!(chart.key_count, 4);
    let notes: Vec<(i64, u8)> = chart.notes.iter().map(|note| (note.time_us, note.column)).collect();
    // Frame 30 = 1 s; the second entry's 1600 ms win over its frame; the duplicate tap and the unknown direction are dropped.
    assert_eq!(notes, [(1_000_000, 0), (1_600_000, 3), (3_000_000, 2)]);
    assert!(chart.notes.iter().all(|note| note.note_type == NoteType::Tap));
    assert_eq!((report.invalid_notes, report.overlapping_notes_dropped, report.tempo_in_file), (1, 1, false));
    assert_eq!(chart.timing_points, [TimingPoint::bpm(0, 120.0)]);
}

#[test]
fn a_compressed_level_with_a_bpmbox_takes_its_first_tempo() {
    let tempo = vec![vec![Item::Int(0), Item::Int(137_000)], vec![Item::Int(1152), Item::Int(274_000)]];
    let file = movie(&[assign("beatBox", &beats()), assign("bpmBox", &tempo)], true);
    let (chart, report) = SwfDecoder::decode_with_report(&file).unwrap();
    assert!(report.tempo_in_file);
    assert_eq!(chart.timing_points, [TimingPoint::bpm(0, 137.0)]);
    assert_eq!(chart.notes.len(), 3);
}

#[test]
fn a_movie_that_is_no_level_or_lzma_is_refused() {
    let other = movie(&[assign("menu", &beats())], false);
    assert!(SwfDecoder::decode_with_report(&other).unwrap_err().to_string().contains("beatBox"));
    let mut lzma = movie(&[assign("beatBox", &beats())], false);
    lzma[0] = b'Z';
    assert!(SwfDecoder::decode_with_report(&lzma).unwrap_err().to_string().contains("LZMA"));
    assert!(SwfDecoder::decode_with_report(b"FW").is_err());
}
