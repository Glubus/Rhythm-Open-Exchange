//! Tests of the R2Beat reader, on charts written in the test.

use rox::model::{NoteType, TimingPoint};

use super::{R2BeatDecoder, looks_like_r2beat};

const CHART: &str = r#"<?xml version="1.0" encoding="euc-kr"?>
<TITLE Name="Test">
<BPM Frame="0" BPM="120" OriginalBPM="120"></BPM>
<BPM Frame="240" BPM="240"></BPM>
<BGM Name="test\test.ogg" Volume="1.00"></BGM>
<DELAY Value="60"></DELAY>
<LENGTH Value="192"></LENGTH>
<AREA Coord="12" Kind="18" Level="2" FxSndIndex="0"></AREA>
<AREA Coord="24" Kind="21" Level="5" FxSndIndex="0"></AREA>
<AREA Coord="36" Kind="26" Level="1" FxSndIndex="0"></AREA>
<AREA Coord="48" Kind="137" Level="5" FxSndIndex="0"></AREA>
<AREA Coord="54" Kind="138" Level="5" FxSndIndex="0"></AREA>
<AREA Coord="60" Kind="139" Level="5" FxSndIndex="0"></AREA>
<AREA Coord="132" Kind="19" Level="2" FxSndIndex="0"></AREA>
</TITLE>"#;

#[test]
fn coords_follow_the_editors_player_and_kinds_are_arrow_keys() {
    assert!(looks_like_r2beat(CHART.as_bytes()));
    let (chart, report) = R2BeatDecoder::decode_with_report(CHART.as_bytes()).unwrap();
    // 120 BPM from 0: 24 coords a second (12 a beat). The second BPM takes over at (240 + 60) / 60 = 5 s, coord 120: from there
    // 48 coords a second.
    let notes: Vec<(i64, u8, NoteType)> = chart.notes.iter().map(|n| (n.time_us, n.column, n.note_type)).collect();
    assert_eq!(
        notes,
        [
            (500_000, 0, NoteType::Tap),
            (1_000_000, 2, NoteType::Tap),
            (1_000_000, 3, NoteType::Tap),
            (2_000_000, 1, NoteType::Hold { duration_us: 500_000 }),
            (5_250_000, 3, NoteType::Tap),
        ]
    );
    assert_eq!(report.no_key_obstacles, 1, "a star");
    assert_eq!(chart.timing_points, [TimingPoint::bpm(0, 120.0), TimingPoint::bpm(1_000_000, 120.0), TimingPoint::bpm(5_000_000, 240.0)]);
    assert_eq!((chart.metadata.title.as_str(), chart.metadata.audio_file.as_str()), ("Test", "test.ogg"));
}

#[test]
fn other_xml_is_not_a_chart() {
    assert!(!looks_like_r2beat(b"<?xml version=\"1.0\"?><project><name>x</name></project>"));
    assert!(R2BeatDecoder::decode_with_report(b"<?xml version=\"1.0\"?><TITLE Name=\"x\"></TITLE>").is_err());
}
