//! Tests of the pydance reader, on files written in the test.

use rox::model::{NoteType, TimingPoint};

use super::DanceDecoder;

const SONG: &str = "filename song.ogg
title A Song
subtitle Crazy Mix
artist Someone
bpm 120
gap -500
end
DESCRIPTION
 This is an example song.
end
SINGLE
BASIC 3
# a comment
q 1010
e 0300
e 0000
B 60
q 0100
S 1.5
q 0001
q 0001
W 2
5PANEL
end
6PANEL
MANIAC 9
D 2
q 100001
q 5x0000
end
";

#[test]
fn sections_are_variants_and_time_follows_letters_commands_and_gap() {
    let variants = DanceDecoder::variants(SONG.as_bytes()).unwrap();
    assert_eq!(variants.iter().map(|v| (v.mode.as_str(), v.difficulty.as_str(), v.rating)).collect::<Vec<_>>(), [("SINGLE", "BASIC", 3), ("6PANEL", "MANIAC", 9)]);
    let (chart, report) = DanceDecoder::decode_with_report(SONG.as_bytes(), 0).unwrap();
    assert_eq!(chart.key_count, 4);
    let notes: Vec<(i64, u8, NoteType)> = chart.notes.iter().map(|n| (n.time_us, n.column, n.note_type)).collect();
    // gap -500: beat 0 is 0.5 s in. 120 BPM: q 1010 at 0.5 s; the hold at 1.0 s ends on its column's next press (1.5 s, after the
    // empty eighth): that press is no note; B 60 then makes a beat a second; the stop after it: the next note plays when the stop
    // starts (2.5 s), the one after it 1.5 s later plus its beat (5.0 s).
    assert_eq!(
        notes,
        [
            (500_000, 0, NoteType::Tap),
            (500_000, 2, NoteType::Tap),
            (1_000_000, 1, NoteType::Hold { duration_us: 500_000 }),
            (2_500_000, 3, NoteType::Tap),
            (5_000_000, 3, NoteType::Tap),
        ]
    );
    assert_eq!(chart.timing_points, [TimingPoint::bpm(500_000, 120.0), TimingPoint::bpm(1_500_000, 60.0)]);
    assert_eq!(report.skipped_lines, 1, "the stray 5PANEL line");
    assert_eq!((chart.metadata.title.as_str(), chart.metadata.audio_file.as_str(), chart.metadata.difficulty_name.as_str()), ("A Song Crazy Mix", "song.ogg", "SINGLE BASIC"));

    let (six, report) = DanceDecoder::decode_with_report(SONG.as_bytes(), 1).unwrap();
    assert_eq!(six.key_count, 6);
    assert_eq!(six.notes.iter().map(|n| (n.time_us, n.column)).collect::<Vec<_>>(), [(1_500_000, 0), (1_500_000, 5)]);
    assert_eq!(report.secret_or_unknown, 2, "a secret note and an unknown character");
}

#[test]
fn a_file_without_bpm_or_section_is_refused() {
    assert!(DanceDecoder::variants(b"title x\nend\nSINGLE\nBASIC 1\nq 1000\nend\n").is_err());
    assert!(DanceDecoder::decode_variant(b"bpm 120\nend\n", 0).is_err());
}

#[test]
fn a_pyddr_step_reads_like_the_dance_it_became() {
    let step = "version 0.3\nfile song.ogg\nsong A Song\ngroup Someone\nbpm 120\noffset -500\nSINGLE \nBASIC 3\ndelay 1\nready\nqurtr 08 00 08 00\nlyric Hello there\neight 00 00 00 08\neight 00 07 00 00\nend\n";
    assert!(super::looks_like_pyddr_step(step.as_bytes()));
    assert!(!super::looks_like_pyddr_step(b"\x00\x01binary step"));
    let (chart, report) = super::PyddrStepDecoder::decode_with_report(step.as_bytes(), 0).unwrap();
    // offset -500 is the `.dance` gap: beat 0 at 0.5 s; one beat of delay at 120 BPM, then the notes.
    assert_eq!(chart.notes.iter().map(|n| (n.time_us, n.column)).collect::<Vec<_>>(), [(1_000_000, 0), (1_000_000, 2), (1_500_000, 3)]);
    assert_eq!(report.secret_or_unknown, 1, "a code other than 08 or 00");
    assert_eq!((chart.metadata.title.as_str(), chart.metadata.artist.as_str(), chart.metadata.audio_file.as_str()), ("A Song", "Someone", "song.ogg"));
}
