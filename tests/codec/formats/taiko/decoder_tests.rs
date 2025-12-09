//! Integration tests for Taiko decoder.

use rhythm_open_exchange::codec::Decoder;
use rhythm_open_exchange::codec::formats::taiko::TaikoDecoder;
use rhythm_open_exchange::codec::formats::taiko::types::ColumnLayout;

#[test]
fn test_decode_basic_taiko_map() {
    let input = b"osu file format v14

[General]
AudioFilename: audio.mp3
Mode: 1

[Metadata]
Title:Test Title
Artist:Test Artist
Creator:Test Creator
Version:Test Diff

[TimingPoints]
0,500,4,1,0,100,1,0

[HitObjects]
// Don (0) at 1000ms
256,192,1000,1,0,0:0:0:0:
// Kat (2) at 2000ms
256,192,2000,1,2,0:0:0:0:
// Big Don (4) at 3000ms
256,192,3000,1,4,0:0:0:0:
// Big Kat (2+4=6) at 4000ms
256,192,4000,1,6,0:0:0:0:
";

    // Defaults to DKKD (Dons=0,3 / Kats=1,2)
    // 1000ms: Don -> Col 0 (Smallest Don)
    // 2000ms: Kat -> Col 1 (Smallest Kat)
    // 3000ms: Big Don -> Col 0 + 3
    // 4000ms: Big Kat -> Col 1 + 2

    let chart = TaikoDecoder::decode(input).expect("Failed to decode");

    assert_eq!(chart.notes.len(), 6);

    // Note 1: Don -> Col 0, 1000ms
    assert_eq!(chart.notes[0].time_us, 1_000_000);
    assert_eq!(chart.notes[0].column, 0);

    // Note 2: Kat -> Col 1, 2000ms
    assert_eq!(chart.notes[1].time_us, 2_000_000);
    assert_eq!(chart.notes[1].column, 1);

    // Note 3: Big Don -> Col 0 + 3, 3000ms
    // Sorted by time, notes at same time are not guaranteed order, but typically sorted by col?
    // Current decoder sorts by time. Stable sort might preserve insertion order.
    // Insert order: Col 0 then Col 3 (from next_don_columns which returns [0, 3])
    let note3a = &chart.notes[2];
    let note3b = &chart.notes[3];
    assert_eq!(note3a.time_us, 3_000_000);
    assert_eq!(note3b.time_us, 3_000_000);
    // 0 and 3
    let mut cols3 = vec![note3a.column, note3b.column];
    cols3.sort();
    assert_eq!(cols3, vec![0, 3]);

    // Note 4: Big Kat -> Col 1 + 2, 4000ms
    let note4a = &chart.notes[4];
    let note4b = &chart.notes[5];
    assert_eq!(note4a.time_us, 4_000_000);
    assert_eq!(note4b.time_us, 4_000_000);
    // 1 and 2
    let mut cols4 = vec![note4a.column, note4b.column];
    cols4.sort();
    assert_eq!(cols4, vec![1, 2]);
}

#[test]
fn test_decode_with_layout_kddk() {
    let input = b"osu file format v14
[General]
Mode: 1
[HitObjects]
256,192,1000,1,0,0:0:0:0:
";

    // KDDK: Kats=0,3 / Dons=1,2
    // Don at 1000ms -> Smallest Don Col -> 1

    let chart =
        TaikoDecoder::decode_with_layout(input, ColumnLayout::Kddk).expect("Failed to decode");

    assert_eq!(chart.notes.len(), 1);
    assert_eq!(chart.notes[0].column, 1);
}

#[test]
fn test_decode_with_layout_dkdk() {
    let input = b"osu file format v14
[General]
Mode: 1
[HitObjects]
256,192,1000,1,0,0:0:0:0:
";

    // DKDK: Dons=0,2 / Kats=1,3
    // Don at 1000ms -> Smallest Don Col -> 0

    let chart =
        TaikoDecoder::decode_with_layout(input, ColumnLayout::Dkdk).expect("Failed to decode");

    assert_eq!(chart.notes.len(), 1);
    assert_eq!(chart.notes[0].column, 0);
}
