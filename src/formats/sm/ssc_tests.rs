//! Behaviour of the `.ssc` reader: charts, timing, note kinds, skips, malformed input and
//! agreement with `.sm`.
use rox::codec::{Decoder, Encoder};
use rox::model::{Note, NoteType, RoxChart, TimingPoint};

use super::decoder::{SmDecoder, from_chart};
use super::ssc_decoder::SscDecoder;
use super::ssc_encoder::SscEncoder;
use super::ssc_parser::parse;

const HEADER: &str = "#VERSION:0.83;\n#TITLE:Test Song;\n#ARTIST:Tester;\n#CREDIT:Song Author;\n#MUSIC:song.ogg;\n#BACKGROUND:bg.png;\n#OFFSET:0.000;\n#SAMPLESTART:10.5;\n#SAMPLELENGTH:12;\n#BPMS:0.000=120.000;\n#STOPS:;\n";

fn chart_block(stepstype: &str, difficulty: &str, extra: &str, notes: &str) -> String {
    format!(
        "#NOTEDATA:;\n#STEPSTYPE:{stepstype};\n#DESCRIPTION:;\n#DIFFICULTY:{difficulty};\n#METER:5;\n{extra}#NOTES:\n{notes}\n;\n"
    )
}

/// Four rows, one per beat: times 0, 0.5, 1.0 and 1.5 s at 120 BPM.
const FOUR: &str = "1000\n0100\n0010\n0001";

fn decode(text: &str) -> RoxChart {
    SscDecoder::decode(text.as_bytes()).expect("decodes and validates")
}

fn taps(chart: &RoxChart) -> Vec<(i64, u8)> {
    chart
        .notes
        .iter()
        .filter(|n| matches!(n.note_type, NoteType::Tap))
        .map(|n| (n.time_us, n.column))
        .collect()
}

#[test]
fn two_charts_keep_their_own_step_type_and_metadata() {
    let text = format!(
        "{HEADER}{}{}",
        chart_block("dance-single", "Hard", "#CREDIT:Chart Author;\n", FOUR),
        chart_block(
            "dance-double",
            "Challenge",
            "",
            "10000001\n00000000\n00000000\n00000000"
        )
    );
    let file = parse(text.as_bytes()).unwrap();
    assert_eq!(file.charts.len(), 2);
    assert!(file.skipped.is_empty());
    let (single, double) = (
        from_chart(&file, &file.charts[0]),
        from_chart(&file, &file.charts[1]),
    );
    assert_eq!((single.key_count, double.key_count), (4, 8));
    assert_eq!(single.metadata.difficulty_name, "Hard");
    assert_eq!(double.metadata.difficulty_name, "Challenge");
    assert_eq!(single.metadata.difficulty_value, Some(5.0));
    assert_eq!(
        (
            single.metadata.title.as_str(),
            single.metadata.artist.as_str()
        ),
        ("Test Song", "Tester")
    );
    assert_eq!(single.metadata.creator, "Chart Author");
    assert_eq!(
        double.metadata.creator, "Song Author",
        "a chart without #CREDIT takes the song's"
    );
    assert_eq!(single.metadata.audio_file, "song.ogg");
    assert_eq!(single.metadata.background_file.as_deref(), Some("bg.png"));
    assert_eq!(single.metadata.audio_offset_us, 0);
    assert_eq!(
        (
            single.metadata.preview_time_us,
            single.metadata.preview_duration_us
        ),
        (10_500_000, 12_000_000)
    );
    assert_eq!(
        taps(&single),
        [(0, 0), (500_000, 1), (1_000_000, 2), (1_500_000, 3)]
    );
    assert_eq!(taps(&double), [(0, 0), (0, 7)]);
}

#[test]
fn an_edit_chart_is_named_by_its_description() {
    let text = format!(
        "{HEADER}#NOTEDATA:;\n#STEPSTYPE:dance-single;\n#DESCRIPTION:Made by Me;\n#DIFFICULTY:Edit;\n#METER:7;\n#NOTES:\n{FOUR}\n;\n"
    );
    assert_eq!(decode(&text).metadata.difficulty_name, "Made by Me");
}

#[test]
fn a_chart_s_own_bpms_and_offset_replace_the_songs_others_inherit_them() {
    let text = format!(
        "{}{}{}",
        HEADER.replace("#OFFSET:0.000;", "#OFFSET:-0.050;"),
        chart_block(
            "dance-single",
            "Easy",
            "#BPMS:0.000=60.000;\n#OFFSET:0.100;\n",
            FOUR
        ),
        chart_block("dance-single", "Hard", "", FOUR)
    );
    let file = parse(text.as_bytes()).unwrap();
    let (slow, inherited) = (
        from_chart(&file, &file.charts[0]),
        from_chart(&file, &file.charts[1]),
    );
    // `#OFFSET:0.100` puts beat 0 at -100 ms of the audio, `#OFFSET:-0.050` at +50 ms.
    assert_eq!(taps(&slow)[1].0, 1_000_000 - 100_000);
    assert_eq!(slow.timing_points, [TimingPoint::bpm(-100_000, 60.0)]);
    assert_eq!(taps(&inherited)[1].0, 500_000 + 50_000);
    assert_eq!(inherited.timing_points, [TimingPoint::bpm(50_000, 120.0)]);
    assert_eq!(
        (
            slow.metadata.audio_offset_us,
            inherited.metadata.audio_offset_us
        ),
        (0, 0)
    );
}

#[test]
fn bpm_changes_move_the_notes_after_them() {
    let text = HEADER.replace("#BPMS:0.000=120.000;", "#BPMS:0.000=120.000,2.000=240.000;")
        + &chart_block("dance-single", "Easy", "", FOUR);
    let chart = decode(&text);
    assert_eq!(
        chart.timing_points,
        [
            TimingPoint::bpm(0, 120.0),
            TimingPoint::bpm(1_000_000, 240.0)
        ]
    );
    assert_eq!(taps(&chart)[3], (1_250_000, 3));
}

#[test]
fn a_stop_pushes_back_what_follows_and_lengthens_a_hold_across_it() {
    let text = HEADER
        .replace("#STOPS:;", "#STOPS:1.000=1.000;")
        .replace("0.000=120.000;", "0.000=120.000,2.000=240.000;")
        + &chart_block("dance-single", "Easy", "", "2100\n0000\n3000\n0001");
    let chart = decode(&text);
    // The hold's tail (beat 2) comes 1 s of stop later than its beat alone; the note after it
    // (beat 3) is a quarter second past the stop and the tempo change.
    assert!(chart.notes.contains(&Note::hold(0, 2_000_000, 0)));
    assert!(chart.notes.contains(&Note::tap(0, 1)));
    assert!(chart.notes.contains(&Note::tap(2_250_000, 3)));
    assert_eq!(chart.notes.len(), 3);
    // A BPM change is reached after the stop it follows.
    assert_eq!(chart.timing_points[1], TimingPoint::bpm(2_000_000, 240.0));
}

#[test]
fn a_delay_holds_the_note_of_its_beat() {
    let text = HEADER.to_owned()
        + &chart_block(
            "dance-single",
            "Easy",
            "#DELAYS:1.000=0.500;\n#BPMS:0.000=120.000;\n",
            FOUR,
        );
    assert_eq!(taps(&decode(&text))[1], (1_000_000, 1));
}

#[test]
fn warped_notes_are_dropped_and_time_skips_the_warp() {
    let text = HEADER.to_owned()
        + &chart_block(
            "dance-single",
            "Easy",
            "#WARPS:1.000=1.000;\n#BPMS:0.000=120.000;\n",
            FOUR,
        );
    assert_eq!(taps(&decode(&text)), [(0, 0), (500_000, 2), (1_000_000, 3)]);
}

#[test]
fn fake_notes_and_fake_regions_are_not_played_and_a_hold_follows_its_head() {
    let text = HEADER.to_owned()
        + &chart_block(
            "dance-single",
            "Easy",
            "#FAKES:1.000=1.000;\n#BPMS:0.000=120.000;\n",
            "1F00\n2000\n0100\n3000\n,\n2000\n0000\n3000\n0000",
        );
    let chart = decode(&text);
    // Beat 1 (the head at 0.5 s) is inside the region: head and tail go; the `F` cell never plays.
    // Beat 0's tap stays; the second measure's hold is outside the region.
    assert_eq!(chart.notes[0], Note::tap(0, 0));
    assert_eq!(chart.notes[1], Note::tap(1_000_000, 1));
    assert_eq!(chart.notes[2], Note::hold(2_000_000, 1_000_000, 0));
    assert_eq!(chart.notes.len(), 3);
}

#[test]
fn holds_rolls_mines_and_lifts_become_the_matching_notes() {
    let text = HEADER.to_owned()
        + &chart_block(
            "dance-single",
            "Easy",
            "",
            "2400\n0000\n3300\nM0L0\n,\n0000\n0000\n0000\n0000",
        );
    let chart = decode(&text);
    assert!(chart.notes.contains(&Note::hold(0, 1_000_000, 0)));
    assert!(chart.notes.contains(&Note::burst(0, 1_000_000, 1)));
    assert!(chart.notes.contains(&Note::mine(1_500_000, 0)));
    assert!(
        chart.notes.contains(&Note::tap(1_500_000, 2)),
        "a lift is a tap"
    );
    assert_eq!(chart.notes.len(), 4);
}

#[test]
fn rows_of_a_measure_are_spread_over_its_four_beats_and_empty_measures_count() {
    let text = HEADER.to_owned()
        + &chart_block(
            "dance-single",
            "Easy",
            "",
            "1000\n0000\n0000\n0000\n0000\n0000\n0000\n0000\n,\n,\n0100\n0000",
        );
    let chart = decode(&text);
    // Eight rows: eighth notes (0.25 s); measure 2 is empty; measure 3 starts at 4 s.
    assert_eq!(taps(&chart), [(0, 0), (4_000_000, 1)]);
}

#[test]
fn notes2_and_inline_comments_are_read() {
    let text = HEADER.to_owned()
        + "#NOTEDATA:;\n#STEPSTYPE:dance-single;\n#DIFFICULTY:Easy;\n#METER:2;\n#NOTES2:\n1000 // first\n// a whole comment line\n0100\n;\n";
    assert_eq!(taps(&decode(&text)), [(0, 0), (1_000_000, 1)]);
}

#[test]
fn step_types_map_to_key_counts_and_the_others_are_skipped_with_a_reason() {
    let row = |n: usize| format!("{}1", "0".repeat(n - 1));
    let block = |stepstype: &str, n: usize| chart_block(stepstype, "Easy", "", &row(n));
    let text = format!(
        "{HEADER}{}{}{}{}{}{}{}{}",
        block("dance-single", 4),
        block("pump-single", 5),
        block("dance-solo", 6),
        block("kb7-single", 7),
        block("dance-double", 8),
        block("pump-double", 10),
        block("dance-routine", 8),
        block("dance-single", 5)
    );
    let file = parse(text.as_bytes()).unwrap();
    let keys: Vec<u8> = file
        .charts
        .iter()
        .map(|c| from_chart(&file, c).key_count)
        .collect();
    assert_eq!(keys, [4, 5, 6, 7, 8]);
    assert_eq!(file.skipped.len(), 3);
    assert!(
        file.skipped[0].starts_with("chart 6")
            && file.skipped[0].contains("unsupported step type `pump-double`")
    );
    assert!(file.skipped[1].contains("dance-routine"));
    assert!(
        file.skipped[2].contains("5 columns, expected 4"),
        "{:?}",
        file.skipped
    );
}

#[test]
fn a_file_with_nothing_playable_names_what_was_skipped() {
    let text = HEADER.to_owned() + &chart_block("pump-double", "Easy", "", &"0".repeat(10));
    let error = SscDecoder::decode(text.as_bytes()).unwrap_err().to_string();
    assert!(
        error.contains("No playable chart") && error.contains("pump-double"),
        "{error}"
    );
}

#[test]
fn malformed_files_are_errors_or_skipped_charts_never_panics() {
    let no_notedata = "#TITLE:x;\n#BPMS:0=120;\n#NOTES:\n    dance-single:\n    :\n    Easy:\n    1:\n    0,0,0,0,0:\n1000\n;\n";
    let error = SscDecoder::decode(no_notedata.as_bytes())
        .unwrap_err()
        .to_string();
    assert!(error.contains("#NOTEDATA"), "{error}");
    for bytes in [
        &b""[..],
        b"#",
        b"#NOTEDATA",
        b"#NOTEDATA:;#NOTES:1000",
        b"\xff\xfe\x00#NOTEDATA:;",
        b"#NOTEDATA:;;;;;:::",
    ] {
        let _ = SscDecoder::decode(bytes);
    }
    let missing = |extra: &str, notes: &str| {
        parse(
            (HEADER.replace("#BPMS:0.000=120.000;", "")
                + &chart_block("dance-single", "Easy", extra, notes))
                .as_bytes(),
        )
        .unwrap()
    };
    assert!(missing("", FOUR).skipped[0].contains("no valid #BPMS"));
    let no_notes = parse(
        (HEADER.to_owned() + "#NOTEDATA:;\n#STEPSTYPE:dance-single;\n#DIFFICULTY:Easy;\n")
            .as_bytes(),
    )
    .unwrap();
    assert!(no_notes.skipped[0].contains("no #NOTES"));
    let ragged = parse(
        (HEADER.to_owned() + &chart_block("dance-single", "Easy", "", "1000\n01")).as_bytes(),
    )
    .unwrap();
    assert!(ragged.skipped[0].contains("measure 1") && ragged.skipped[0].contains("expected 4"));
    let no_type =
        parse((HEADER.to_owned() + "#NOTEDATA:;\n#NOTES:\n1000\n;\n").as_bytes()).unwrap();
    assert!(no_type.skipped[0].contains("no #STEPSTYPE"));
}

#[test]
fn a_bom_crlf_and_undecodable_text_do_not_disturb_the_notes() {
    let text = HEADER.to_owned() + &chart_block("dance-single", "Easy", "", FOUR);
    let plain = decode(&text);
    let mut bytes = b"\xef\xbb\xbf".to_vec();
    bytes.extend(text.replace('\n', "\r\n").bytes());
    assert_eq!(SscDecoder::decode(&bytes).unwrap(), plain);
    let latin1 = text
        .replace("Tester", "T\u{e9}ster")
        .into_bytes()
        .into_iter()
        .filter(|&b| b != 0xc3)
        .collect::<Vec<u8>>();
    let chart = SscDecoder::decode(&latin1).expect("bad bytes in a title are not fatal");
    assert_eq!(chart.notes, plain.notes);
}

#[test]
fn an_ssc_gives_the_same_chart_as_the_equivalent_sm() {
    let measures = "1000\n0100\n0010\n0001\n,\n2000\n0000\n3000\n0M00\n,\n0010\n0010\n1001\n0000";
    let sm = format!(
        "#TITLE:Same;\n#MUSIC:song.ogg;\n#OFFSET:-0.020;\n#BPMS:0.000=150.000,4.000=90.000;\n#STOPS:;\n#NOTES:\n     dance-single:\n     :\n     Hard:\n     5:\n     0,0,0,0,0:\n{measures}\n;\n"
    );
    let ssc = format!(
        "#TITLE:Same;\n#MUSIC:song.ogg;\n#OFFSET:-0.020;\n#BPMS:0.000=150.000,4.000=90.000;\n#STOPS:;\n{}",
        chart_block("dance-single", "Hard", "", measures)
    );
    let (a, b) = (
        SmDecoder::decode(sm.as_bytes()).unwrap(),
        SscDecoder::decode(ssc.as_bytes()).unwrap(),
    );
    assert_eq!(a, b, "the two readers share one pipeline");
    assert!(!b.notes.is_empty());
}

#[test]
fn a_chart_survives_the_ssc_encoder() {
    let text = HEADER.to_owned()
        + &chart_block(
            "dance-single",
            "Hard",
            "",
            "2000\n0100\n3000\n0001\n,\n0010\n0000\nM000\n0000",
        );
    let chart = decode(&text);
    let written = SscEncoder::encode(&chart).unwrap();
    let again = SscDecoder::decode(&written).expect("the encoder's output decodes");
    assert_eq!(again.notes, chart.notes);
    assert_eq!(again.key_count, 4);
    assert_eq!(again.metadata.title, chart.metadata.title);
    let text = String::from_utf8(written).unwrap();
    assert!(
        text.starts_with("#VERSION:")
            && text.contains("#NOTEDATA:;")
            && text.contains("#STEPSTYPE:dance-single;")
    );
}

#[test]
fn the_encoder_refuses_a_column_count_without_a_step_type() {
    let mut chart = RoxChart::new(9);
    chart.timing_points.push(TimingPoint::bpm(0, 120.0));
    chart.notes.push(Note::tap(0, 0));
    assert!(
        SscEncoder::encode(&chart)
            .unwrap_err()
            .to_string()
            .contains("9 columns")
    );
}

#[test]
fn files_are_read_by_extension_and_variant() {
    let dir = std::env::temp_dir().join(format!("rox-ssc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let text = HEADER.to_owned()
        + &chart_block("pump-double", "Easy", "", &"0".repeat(10))
        + &chart_block("dance-single", "Easy", "", FOUR)
        + &chart_block("dance-double", "Hard", "", "10000001");
    let path = dir.join("song.SSC");
    std::fs::write(&path, text).unwrap();
    assert!(super::is_stepmania(&path));
    let file = super::parse_file(&path).unwrap();
    assert_eq!(
        file.charts.len(),
        2,
        "the skipped chart takes no variant number"
    );
    assert_eq!(
        crate::formats::auto_decode_variant(&path, 0)
            .unwrap()
            .key_count,
        4
    );
    assert_eq!(
        crate::formats::auto_decode_variant(&path, 1)
            .unwrap()
            .key_count,
        8
    );
    assert!(crate::formats::auto_decode_variant(&path, 2).is_err());
    assert_eq!(crate::formats::auto_decode(&path).unwrap().key_count, 4);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_missing_or_empty_music_plays_the_first_audio_file_of_the_folder() {
    let dir = std::env::temp_dir().join(format!("rox-ssc-music-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["b.ogg", "A.MP3", "cover.png"] {
        std::fs::write(dir.join(name), b"x").unwrap();
    }
    let chart = chart_block("dance-single", "Easy", "", FOUR);
    let write = |header: &str| {
        let path = dir.join("song.ssc");
        std::fs::write(&path, format!("{header}#BPMS:0=120;\n{chart}")).unwrap();
        super::parse_file(&path).unwrap().metadata.music
    };
    assert_eq!(write("#MUSIC:;\n"), "A.MP3");
    assert_eq!(write(""), "A.MP3");
    assert_eq!(write("#MUSIC:gone.ogg;\n"), "A.MP3");
    assert_eq!(
        write("#MUSIC:b.ogg;\n"),
        "b.ogg",
        "an existing #MUSIC is kept"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
