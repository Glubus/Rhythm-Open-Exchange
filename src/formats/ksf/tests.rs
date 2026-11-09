//! Tests of the `.ksf` reader: the file-name classification, the rows and the hold rule, the
//! header and Direct Move timing, and the same chart written as `.ksf` and as `.sm` decoding to the
//! same notes and times.
use rox::codec::Decoder;
use rox::model::{NoteType, RoxChart, TimingPoint};

use super::parser::{KsfContext, parse_detailed};
use super::{KsfDecoder, KsfReport};
use crate::formats::sm::SmDecoder;

/// A row of 13 characters with `cells` first.
fn row(cells: &str) -> String {
    format!("{cells:0<13}")
}

/// A `.ksf` with `header` tags and `rows` (a row of thirteen `2`s ends it).
fn ksf(header: &str, rows: &[String]) -> Vec<u8> {
    format!("{header}\n#STEP:\n{}\n2222222222222\n", rows.join("\n")).into_bytes()
}

fn decode(header: &str, rows: &[String], file_name: &str) -> RoxChart {
    KsfDecoder::decode_named(&ksf(header, rows), file_name).expect("decodes and validates")
}

fn taps(chart: &RoxChart) -> Vec<(i64, u8)> {
    chart
        .notes
        .iter()
        .filter(|note| matches!(note.note_type, NoteType::Tap))
        .map(|note| (note.time_us, note.column))
        .collect()
}

fn times(chart: &RoxChart) -> Vec<i64> {
    taps(chart).iter().map(|&(time, _)| time).collect()
}

fn report_of(data: &[u8], file_name: &str) -> Result<KsfReport, String> {
    KsfDecoder::decode_with_report(
        data,
        &KsfContext {
            file_name,
            folder_name: None,
        },
    )
    .map(|(_, report)| report)
    .map_err(|error| error.to_string())
}

fn named(file_name: &str) -> KsfContext<'_> {
    KsfContext {
        file_name,
        folder_name: None,
    }
}

#[test]
fn the_same_chart_as_ksf_and_as_sm_has_the_same_notes_and_times() {
    // The sample of StepMania's `Docs/SimfileFormats/KSF/sample.ksf` (five taps, then a four-row
    // hold on the centre column), 4 rows per beat at 150 BPM, `#STARTTIME:50` (0.5 s).
    let rows: Vec<String> = [
        "1000000000000",
        "0100000000000",
        "0010000000000",
        "0001000000000",
        "0000100000000",
        "0040000000000",
        "0040000000000",
        "0040000000000",
        "0040000000000",
        "0000000000000",
    ]
    .map(str::to_string)
    .to_vec();
    let from_ksf = decode(
        "#TITLE:***;\n#BPM:150;\n#STARTTIME:50;\n#PLAYER:SINGLE;\n#TICKCOUNT:4;\n#SONGFILE:;",
        &rows,
        "sample.ksf",
    );
    // The hold is the head row 5 to the last `4` row 8: a `.sm` `2` and `3` three rows apart.
    let sm = "#TITLE:***;\n#OFFSET:-0.500;\n#BPMS:0=150;\n#NOTES:\n     pump-single:\n     :\n     Hard:\n     10:\n     0,0,0,0,0:\n\
10000\n01000\n00100\n00010\n00001\n00200\n00000\n00000\n00300\n00000\n00000\n00000\n00000\n00000\n00000\n00000\n;\n";
    let from_sm = SmDecoder::decode(sm.as_bytes()).expect("sm decodes");
    assert_eq!(from_ksf.notes, from_sm.notes);
    assert_eq!(from_ksf.timing_points, from_sm.timing_points);
    assert_eq!(from_ksf.key_count, 5);
    assert_eq!(from_ksf.notes.len(), 6);
    assert_eq!(from_ksf.notes[0].time_us, 500_000);
}

#[test]
fn bunki_changes_the_bpm_at_a_time_of_the_audio() {
    // 120 BPM; #BPM2 240 from 4 s (#BUNKI is centiseconds) = beat 8 = (4 + 0) * 120 / 60.
    let rows: Vec<String> = (0..12).map(|_| row("1")).collect();
    let header = "#BPM:120;\n#BPM2:240;\n#BUNKI:400;\n#STARTTIME:0;\n#TICKCOUNT:1;";
    let from_ksf = decode(header, &rows, "x.ksf");
    let measure = "10000\n10000\n10000\n10000";
    let sm = format!(
        "#OFFSET:0;\n#BPMS:0=120,8=240;\n#NOTES:\n     pump-single:\n     :\n     Hard:\n     10:\n     0,0,0,0,0:\n{measure},\n{measure},\n{measure}\n;\n"
    );
    let from_sm = SmDecoder::decode(sm.as_bytes()).expect("sm decodes");
    assert_eq!(from_ksf.notes, from_sm.notes);
    assert_eq!(from_ksf.timing_points, from_sm.timing_points);
    assert_eq!(times(&from_ksf)[7..10], [3_500_000, 4_000_000, 4_250_000]);
    // The BUNKI beat counts the start time: beat 0 is 1 s into the audio, so the change comes at
    // beat (4 - 1) * 2 = 6, 3 s after beat 0.
    let late = decode(
        "#BPM:120;\n#BPM2:240;\n#BUNKI:400;\n#STARTTIME:100;\n#TICKCOUNT:1;",
        &rows,
        "x.ksf",
    );
    assert_eq!(times(&late)[6..8], [4_000_000, 4_250_000]);
}

#[test]
fn a_third_bpm_uses_the_second_gap_and_the_second_bpm() {
    // Loader quirk kept: beat = (time + gap2) * BPM2 / 60. 120, 240 at 2 s (beat 4), 60 at 6 s
    // (beat (6 + 0) * 240 / 60 = 24).
    let rows: Vec<String> = (0..26).map(|_| row("1")).collect();
    let chart = decode(
        "#BPM:120;\n#BPM2:240;\n#BPM3:60;\n#BUNKI:200;\n#BUNKI2:600;\n#STARTTIME:0;\n#STARTTIME2:0;\n#TICKCOUNT:1;",
        &rows,
        "x.ksf",
    );
    assert_eq!(
        chart.timing_points,
        [
            TimingPoint::bpm(0, 120.0),
            TimingPoint::bpm(2_000_000, 240.0),
            TimingPoint::bpm(7_000_000, 60.0)
        ]
    );
    // Beat 4 at 2 s, beat 24 at 2 + 20 * 0.25 = 7 s.
    assert_eq!(times(&chart)[4], 2_000_000);
    assert_eq!(times(&chart)[24], 7_000_000);
}

#[test]
fn direct_move_rows_change_the_tempo_and_delay_the_song() {
    // |B240| at beat 1 (tick 1), |E2| at beat 2: 60 / 240 * 2 / 1 = 0.5 s before its notes.
    let rows = [
        row("1"),
        "|B240|".to_string(),
        row("1"),
        "|E2|".to_string(),
        row("1"),
        row("1"),
    ];
    let chart = decode("#BPM:120;\n#TICKCOUNT:1;", &rows, "x.ksf");
    assert_eq!(times(&chart), [0, 500_000, 1_250_000, 1_500_000]);
    let sm = "#OFFSET:0;\n#BPMS:0=120,1=240;\n#DELAYS:2=0.5;\n#NOTES:\n     pump-single:\n     :\n     Hard:\n     10:\n     0,0,0,0,0:\n10000\n10000\n10000\n10000\n;\n";
    let from_sm = SmDecoder::decode(sm.as_bytes()).expect("sm decodes");
    assert_eq!(chart.notes, from_sm.notes);
}

#[test]
fn direct_move_ticks_and_millisecond_delays() {
    // |T2| from the second row on (half-beat rows); |D500| half a second before beat 1.
    let rows = [
        row("1"),
        "|T2|".to_string(),
        row("1"),
        "|D500|".to_string(),
        row("1"),
        "|M3|".to_string(),
        row("1"),
    ];
    let (chart, report) =
        KsfDecoder::decode_with_report(&ksf("#BPM:120;\n#TICKCOUNT:1;", &rows), &named("x.ksf"))
            .expect("decodes");
    // Row 0 at beat 0; |T2| applies from the next row: beat 1 (the step after row 0 used tick 1),
    // then rows every half beat: beat 1, 1.5 (after a 0.5 s delay), 2.
    assert_eq!(times(&chart), [0, 500_000, 1_250_000, 1_500_000]);
    assert!(report.warnings.iter().any(|w| w.contains("Direct Move")));
    assert!(
        !report.kiu_compliant,
        "a `|` row makes the file Direct Move"
    );
}

#[test]
fn the_file_name_gives_the_step_type_the_difficulty_and_the_default_meter() {
    let data = ksf("#BPM:120;\n#TICKCOUNT:4;", &[row("1")]);
    for (name, stepstype, difficulty, meter) in [
        ("Crazy_1.ksf", "pump-single", "Hard", 14),
        ("easy_1.ksf", "pump-single", "Easy", 4),
        ("normal.ksf", "pump-single", "Easy", 4),
        ("hard_1.ksf", "pump-single", "Medium", 8),
        ("wild_1.ksf", "pump-single", "Challenge", 20),
        ("Crazy+_1.ksf", "pump-single", "Challenge", 20),
        ("another_1.ksf", "pump-single", "Edit", 25),
        ("beginner_1.ksf", "pump-single", "Beginner", 4),
        ("song.ksf", "pump-single", "Hard", 10),
        ("Halfdouble.ksf", "pump-halfdouble", "Medium", 8),
        ("half-double.ksf", "pump-halfdouble", "Medium", 8),
        ("C:\\songs\\HDB.KSF", "pump-halfdouble", "Medium", 8),
    ] {
        let parsed = parse_detailed(&data, &named(name)).expect("parses");
        let chart = parsed
            .file
            .charts
            .first()
            .unwrap_or_else(|| panic!("{name}: {:?}", parsed.file.skipped));
        assert_eq!(
            (
                chart.stepstype.as_str(),
                chart.difficulty.as_str(),
                chart.meter
            ),
            (stepstype, difficulty, meter),
            "{name}"
        );
        assert_eq!(
            chart.description,
            name.rsplit('\\')
                .next()
                .unwrap()
                .to_lowercase()
                .trim_end_matches(".ksf")
        );
    }
}

#[test]
fn doubles_play_on_ten_columns_and_couples_are_skipped_with_a_message() {
    // A row of 13 cells: a double reads the ten pads, a tap on the first and on the last.
    let data = ksf("#BPM:120;\n#TICKCOUNT:4;", &[row("1000000001")]);
    for name in ["Double.ksf", "nightmare.ksf", "freestyle.ksf"] {
        let chart = KsfDecoder::decode_named(&data, name).expect(name);
        assert_eq!(chart.key_count, 10, "{name}");
        assert_eq!(chart.notes.iter().map(|note| note.column).collect::<Vec<_>>(), [0, 9], "{name}");
    }
    let parsed = parse_detailed(&data, &named("crazy_2.ksf")).expect("parses");
    assert!(parsed.file.charts.is_empty());
    assert!(parsed.file.skipped[0].contains("pump-couple"), "{:?}", parsed.file.skipped);
    assert!(KsfDecoder::decode_named(&data, "crazy_2.ksf").is_err());
    // `#PLAYER` containing `double` makes a double of any name.
    let player = ksf("#BPM:120;\n#PLAYER:DOUBLE;", &[row("1")]);
    assert_eq!(KsfDecoder::decode_named(&player, "x.ksf").expect("double").key_count, 10);
}

#[test]
fn a_half_double_is_the_middle_six_columns() {
    let rows = [row("0010000000000"), row("0000000100000")];
    let chart = decode("#BPM:120;\n#TICKCOUNT:2;", &rows, "Halfdouble.ksf");
    assert_eq!(chart.key_count, 6);
    assert_eq!(taps(&chart), [(0, 0), (250_000, 5)]);
}

#[test]
fn a_hold_is_its_first_and_its_last_four() {
    // Rows 0-2 are `4`: head on row 0, tail on row 2 (not on the first `0`); 4 rows per beat.
    let rows = [row("4"), row("4"), row("4"), row("0"), row("1")];
    let chart = decode("#BPM:120;\n#TICKCOUNT:4;", &rows, "x.ksf");
    let hold = chart.notes.iter().find(|n| n.is_hold()).expect("hold");
    assert_eq!(
        (hold.time_us, hold.column, hold.duration_us()),
        (0, 0, 250_000)
    );
    assert_eq!(taps(&chart), [(500_000, 0)]);
    // A single `4` row is a tap.
    let chart = decode("#BPM:120;\n#TICKCOUNT:4;", &[row("4"), row("0")], "x.ksf");
    assert_eq!(taps(&chart), [(0, 0)]);
    // A hold still open at the end row closes on the row before it.
    let chart = decode("#BPM:120;\n#TICKCOUNT:4;", &[row("04"), row("04")], "x.ksf");
    let hold = &chart.notes[0];
    assert_eq!((hold.column, hold.duration_us()), (1, 125_000));
}

#[test]
fn a_hold_with_no_end_row_is_removed_and_reported() {
    let data = format!(
        "#BPM:120;\n#TICKCOUNT:4;\n#STEP:\n{}\n{}\n{}\n",
        row("1"),
        row("4"),
        row("4")
    );
    let parsed = parse_detailed(data.as_bytes(), &named("x.ksf")).expect("parses");
    assert_eq!(parsed.file.charts[0].notes.len(), 1);
    assert!(parsed.warnings.iter().any(|w| w.contains("never closed")));
    assert!(!parsed.kiu_compliant, "no end row, no header BPM change");
}

#[test]
fn mines_lifts_fakes_and_invalid_cells() {
    let (chart, report) = KsfDecoder::decode_with_report(
        &ksf("#BPM:120;\n#TICKCOUNT:4;", &[row("1MLFx")]),
        &named("x.ksf"),
    )
    .expect("decodes");
    let kinds: Vec<(u8, bool)> = chart
        .notes
        .iter()
        .map(|n| (n.column, n.is_mine()))
        .collect();
    // Tap 0, mine 1, lift 2 (a tap); the fake is not played, `x` is counted.
    assert_eq!(kinds, [(0, false), (1, true), (2, false)]);
    assert!(report.warnings.iter().any(|w| w.contains("invalid")));
}

#[test]
fn a_missing_tick_count_is_4_and_a_bad_one_refuses_the_chart() {
    let rows = [row("1"), row("1")];
    let (chart, report) =
        KsfDecoder::decode_with_report(&ksf("#BPM:120;", &rows), &named("x.ksf")).expect("decodes");
    assert_eq!(times(&chart), [0, 125_000]);
    assert!(report.warnings.iter().any(|w| w.contains("TICKCOUNT")));
    let error = report_of(&ksf("#BPM:120;\n#TICKCOUNT:0;", &rows), "x.ksf").unwrap_err();
    assert!(error.contains("tick count"), "{error}");
}

#[test]
fn a_chart_that_cannot_be_timed_or_read_is_refused() {
    let rows = [row("1")];
    let error = report_of(&ksf("#TICKCOUNT:4;", &rows), "x.ksf").unwrap_err();
    assert!(error.contains("BPM"), "{error}");
    let error = report_of(&ksf("#BPM:0;", &rows), "x.ksf").unwrap_err();
    assert!(error.contains("BPMS"), "{error}");
    let error = report_of(&ksf("#BPM:120;", &["1000".to_string()]), "x.ksf").unwrap_err();
    assert!(error.contains("columns"), "{error}");
}

#[test]
fn the_report_names_the_chart() {
    let data = ksf(
        "#BPM:120;\n#DIFFICULTY:7;\n#BPM2:150;\n#BUNKI:100;",
        &[row("1")],
    );
    let report = report_of(&data, "Crazy_1.ksf").expect("decodes");
    assert_eq!(
        (
            report.stepstype.as_str(),
            report.difficulty.as_str(),
            report.meter
        ),
        ("pump-single", "Hard", 7)
    );
    assert!(report.kiu_compliant);
    // `#DIFFICULTY:;` is 1, an absent tag the default of the difficulty.
    let data = ksf("#BPM:120;\n#DIFFICULTY:;", &[row("1")]);
    assert_eq!(
        super::KsfDecoder::decode_with_report(&data, &named("x.ksf"))
            .unwrap()
            .1
            .meter,
        1
    );
}

#[test]
fn metadata_comes_from_the_tags_then_the_folder_name() {
    let data = ksf(
        "#TITLE:The Artist - The Title - Crazy;\n#BPM:120;\n#SONGFILE:song.mp3;\n#TITLEFILE:back.bmp;\n#DISCFILE:disc.bmp;\n#INTRO:1:05;",
        &[row("1")],
    );
    let (chart, _) = KsfDecoder::decode_with_report(&data, &named("x.ksf")).expect("decodes");
    assert_eq!(chart.metadata.title.as_str(), "The Title");
    assert_eq!(chart.metadata.artist.as_str(), "The Artist");
    assert_eq!(chart.metadata.audio_file.as_str(), "song.mp3");
    assert_eq!(chart.metadata.background_file.as_deref(), Some("back.bmp"));
    assert_eq!(chart.metadata.preview_time_us, 65_000_000);
    assert_eq!(chart.metadata.preview_duration_us, 7_000_000);
    assert_eq!(chart.metadata.difficulty_name.as_str(), "Hard");
    // No tag for the title: the folder name, `Artist - Title`; `Title` alone gives no artist.
    let data = ksf("#BPM:120;", &[row("1")]);
    let context = KsfContext {
        file_name: "x.ksf",
        folder_name: Some("Foo - Bar"),
    };
    let (chart, _) = KsfDecoder::decode_with_report(&data, &context).expect("decodes");
    assert_eq!(chart.metadata.title.as_str(), "Bar");
    assert_eq!(chart.metadata.artist.as_str(), "Foo");
    assert_eq!(
        chart.metadata.audio_file.as_str(),
        "",
        "the caller finds the audio"
    );
}

#[test]
fn korean_titles_and_crlf_files() {
    let (korean, _, _) = encoding_rs::EUC_KR.encode("가수 - 노래");
    let mut data = b"#TITLE:".to_vec();
    data.extend_from_slice(&korean);
    data.extend_from_slice(b";\r\n#BPM:120;\r\n#TICKCOUNT:2;\r\n#STEP:\r\n");
    data.extend_from_slice(
        format!("{}\r\n{}\r\n2222222222222\r\n", row("1"), row("01")).as_bytes(),
    );
    let chart = KsfDecoder::decode_named(&data, "x.ksf").expect("decodes");
    assert_eq!(chart.metadata.title.as_str(), "노래");
    assert_eq!(chart.metadata.artist.as_str(), "가수");
    assert_eq!(taps(&chart), [(0, 0), (250_000, 1)]);
}

#[test]
fn decode_without_a_name_reads_a_single() {
    let chart = KsfDecoder::decode(&ksf("#BPM:120;\n#TICKCOUNT:4;", &[row("1")])).expect("decodes");
    assert_eq!(chart.key_count, 5);
}

#[test]
fn the_auto_decoders_route_ksf_files_by_their_names() {
    use crate::formats::{auto_decode, auto_decode_all, auto_decode_bytes, auto_decode_variant};
    let dir = std::env::temp_dir().join(format!("rox-ksf-auto-{}", std::process::id()));
    let folder = dir.join("Some Artist - Some Song");
    std::fs::create_dir_all(&folder).expect("temp dir");
    let data = ksf("#BPM:120;\n#TICKCOUNT:4;", &[row("1"), row("01")]);
    let crazy = folder.join("Crazy_1.ksf");
    let double = folder.join("Double.ksf");
    std::fs::write(&crazy, &data).expect("write");
    std::fs::write(&double, &data).expect("write");
    let chart = auto_decode(&crazy).expect("decodes");
    assert_eq!(chart.key_count, 5);
    assert_eq!(chart.metadata.difficulty_name.as_str(), "Hard");
    assert_eq!(chart.metadata.title.as_str(), "Some Song");
    assert_eq!(chart.metadata.artist.as_str(), "Some Artist");
    assert_eq!(
        auto_decode_variant(&crazy, 0).expect("variant").notes.len(),
        2
    );
    assert!(auto_decode_variant(&crazy, 1).is_err());
    assert_eq!(
        auto_decode_bytes(&crazy, &data).expect("bytes").key_count,
        5
    );
    assert_eq!(auto_decode(&double).expect("double").key_count, 10, "pump-double has 10 columns");
    assert!(auto_decode_all(&crazy).expect("all").is_none());
    std::fs::remove_dir_all(&dir).ok();
}
