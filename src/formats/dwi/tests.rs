//! Tests of the `.dwi` reader: the note grammar, the tags and units of the loader, the skipped
//! charts, and the same chart written as `.dwi` and as `.sm` decoding to the same notes and times.
use rox::codec::Decoder;
use rox::model::{NoteType, RoxChart};

use super::DwiDecoder;
use crate::formats::sm::SmDecoder;

/// A `.dwi` file with the header `tags` and one chart tag `chart` (`SINGLE:BASIC:4:steps`).
fn dwi(tags: &str, chart: &str) -> Vec<u8> {
    format!("#TITLE:T;\n#ARTIST:A;\n#BPM:120;\n{tags}\n#{chart};\n").into_bytes()
}

fn decode(data: &[u8]) -> RoxChart {
    DwiDecoder::decode(data).expect("decodes and validates")
}

fn taps(chart: &RoxChart) -> Vec<(i64, u8)> {
    chart
        .notes
        .iter()
        .filter(|note| matches!(note.note_type, NoteType::Tap))
        .map(|note| (note.time_us, note.column))
        .collect()
}

/// The columns of the notes at `time_us`, sorted.
fn columns_at(chart: &RoxChart, time_us: i64) -> Vec<u8> {
    let mut columns: Vec<u8> = chart
        .notes
        .iter()
        .filter(|note| note.time_us == time_us)
        .map(|note| note.column)
        .collect();
    columns.sort_unstable();
    columns
}

/// A `.sm` file with one `dance-single` chart.
fn sm(header: &str, measures: &str) -> Vec<u8> {
    format!(
        "#TITLE:T;\n{header}\n#NOTES:\n     dance-single:\n     :\n     Easy:\n     4:\n     0,0,0,0,0:\n{measures}\n;\n"
    )
    .into_bytes()
}

#[test]
fn the_same_chart_as_dwi_and_as_sm_has_the_same_notes_and_times() {
    // Three measures of "L U D R" on the first four 8ths; the BPM doubles at SM beat 4 (DWI beat
    // 16: the loader divides by 4) and a 500 ms freeze sits at SM beat 8 (DWI beat 32); GAP -300.
    let from_dwi = decode(&dwi(
        "#GAP:-300;\n#CHANGEBPM:16=240;\n#FREEZE:32=500;",
        "SINGLE:BASIC:4:482600004826000048260000",
    ));
    let measure = "1000\n0010\n0100\n0001\n0000\n0000\n0000\n0000\n";
    let measures = [measure, measure, measure].join(",\n");
    let from_sm = SmDecoder::decode(&sm(
        "#OFFSET:0.300;\n#BPMS:0=120,4=240;\n#STOPS:8=0.5;",
        &measures,
    ))
    .expect("sm decodes");
    assert_eq!(from_dwi.notes, from_sm.notes);
    assert_eq!(from_dwi.timing_points, from_sm.timing_points);
    assert_eq!(from_dwi.key_count, 4);
    // A GAP of -300 ms puts beat 0 0.3 s before the start of the audio.
    assert_eq!(from_dwi.notes[0].time_us, -300_000);
}

#[test]
fn holds_and_jumps_match_their_sm_spelling() {
    // Up held from slot 0 to slot 4 (beat 2), then a Left+Right jump on slot 6 (beat 3).
    let from_dwi = decode(&dwi("", "SINGLE:BASIC:4:8!800080B"));
    let measure = "0020\n0000\n0000\n0000\n0030\n0000\n1001\n0000\n";
    let from_sm = SmDecoder::decode(&sm("#OFFSET:0;\n#BPMS:0=120;", measure)).expect("sm decodes");
    assert_eq!(from_dwi.notes.len(), 3);
    assert_eq!(from_dwi.notes, from_sm.notes);
}

#[test]
fn a_positive_gap_delays_beat_zero() {
    let chart = decode(&dwi("#GAP:100;", "SINGLE:BASIC:4:20"));
    assert_eq!(taps(&chart), [(100_000, 1)]);
    // The loader reads an integer: `12.9` is 12 ms.
    let chart = decode(&dwi("#GAP:12.9;", "SINGLE:BASIC:4:20"));
    assert_eq!(taps(&chart), [(12_000, 1)]);
}

#[test]
fn brackets_change_the_slot_length() {
    // 120 BPM: a beat is 500 ms. `(..)` is 1/4 beat per slot, `[..]` 1/6, `` `..' `` 1/48.
    let chart = decode(&dwi("", "SINGLE:BASIC:4:2(24)8"));
    assert_eq!(
        taps(&chart),
        [(0, 1), (250_000, 1), (375_000, 0), (500_000, 2)]
    );
    let chart = decode(&dwi("", "SINGLE:BASIC:4:[222]"));
    assert_eq!(taps(&chart), [(0, 1), (83_333, 1), (166_667, 1)]);
    let chart = decode(&dwi("", "SINGLE:BASIC:4:`2222'8"));
    assert_eq!(
        taps(&chart),
        [(0, 1), (10_417, 1), (20_833, 1), (31_250, 1), (41_667, 2)]
    );
    let chart = decode(&dwi("", "SINGLE:BASIC:4:{22}2"));
    assert_eq!(taps(&chart), [(0, 1), (31_250, 1), (62_500, 1)]);
}

#[test]
fn angle_brackets_are_a_jump_or_the_old_192_marker() {
    // A `0` before the next `>`: the old 1/192 marker (StepMania's `Is192`).
    let chart = decode(&dwi("", "SINGLE:BASIC:4:<202>2"));
    assert_eq!(taps(&chart), [(0, 1), (20_833, 1), (31_250, 1)]);
    // No `0`: a jump, all on one slot.
    let chart = decode(&dwi("", "SINGLE:BASIC:4:<246>8"));
    assert_eq!(columns_at(&chart, 0), [0, 1, 3]);
    assert_eq!(taps(&chart).last(), Some(&(250_000, 2)));
}

#[test]
fn solo_has_six_panels_and_single_ignores_the_two_corners() {
    let solo = decode(&dwi("", "SOLO:ANOTHER:7:<46M>"));
    assert_eq!(solo.key_count, 6);
    assert_eq!(columns_at(&solo, 0), [0, 1, 4, 5]);
    let (single, report) =
        DwiDecoder::decode_with_report(&dwi("", "SINGLE:BASIC:4:<46M>"), 0).expect("decodes");
    assert_eq!(columns_at(&single, 0), [0, 3]);
    assert!(report.warnings.iter().any(|w| w.contains("panels")));
}

#[test]
fn every_panel_code_of_the_four_panel_styles() {
    // 1 D+L, 2 D, 3 D+R, 4 L, 6 R, 7 U+L, 8 U, 9 U+R, A U+D, B L+R (columns L D U R = 0 1 2 3).
    let chart = decode(&dwi("", "SINGLE:BASIC:4:12346789AB"));
    let expected = [
        vec![0, 1],
        vec![1],
        vec![1, 3],
        vec![0],
        vec![3],
        vec![0, 2],
        vec![2],
        vec![2, 3],
        vec![1, 2],
        vec![0, 3],
    ];
    for (slot, columns) in expected.iter().enumerate() {
        let time = i64::try_from(slot).unwrap() * 250_000;
        assert_eq!(&columns_at(&chart, time), columns, "slot {slot}");
    }
}

#[test]
fn a_double_has_two_pads_timed_from_beat_zero() {
    let chart = decode(&dwi("", "DOUBLE:MANIAC:9:80:20"));
    assert_eq!(chart.key_count, 8);
    // Up of pad 1 and Down of pad 2 (columns 4..8), both at beat 0.
    assert_eq!(columns_at(&chart, 0), [2, 5]);
}

#[test]
fn a_hold_ends_on_the_next_arrow_of_its_column_whatever_it_is() {
    // `8!8` holds Up; the `7` (Up+Left) two slots later releases it and is a Left tap.
    let chart = decode(&dwi("", "SINGLE:BASIC:4:8!807"));
    let hold = chart
        .notes
        .iter()
        .find(|note| note.is_hold())
        .expect("hold");
    assert_eq!((hold.time_us, hold.column), (0, 2));
    assert_eq!(hold.duration_us(), 500_000);
    assert_eq!(taps(&chart), [(500_000, 0)]);
    // `7!4`: Up and Left are shown, only Left is held.
    let chart = decode(&dwi("", "SINGLE:BASIC:4:7!404"));
    assert_eq!(taps(&chart), [(0, 2)]);
    let hold = chart
        .notes
        .iter()
        .find(|note| note.is_hold())
        .expect("hold");
    assert_eq!(
        (hold.time_us, hold.column, hold.duration_us()),
        (0, 0, 500_000)
    );
}

#[test]
fn a_hold_nothing_releases_is_removed_and_reported() {
    let (chart, report) =
        DwiDecoder::decode_with_report(&dwi("", "SINGLE:BASIC:4:2008!80"), 0).expect("decodes");
    assert_eq!(chart.notes.len(), 1);
    assert!(report.warnings.iter().any(|w| w.contains("never closed")));
}

#[test]
fn freezes_and_bpm_changes_are_read_like_the_loader() {
    // BPM 120, 240 from DWI beat 4 (SM beat 1), a 1 s freeze at DWI beat 8 (SM beat 2).
    let chart = decode(&dwi(
        "#CHANGEBPM:4=240;\n#FREEZE:8=1000;",
        "SINGLE:BASIC:4:2222222222",
    ));
    let times: Vec<i64> = taps(&chart).iter().map(|&(time, _)| time).collect();
    // Beats 0, 0.5, 1 (500 ms); then 240 BPM: 1.5 -> 625 ms, 2 -> 750 ms (before the freeze),
    // 2.5 -> 750 + 1000 + 125.
    assert_eq!(
        &times[..6],
        [0, 250_000, 500_000, 625_000, 750_000, 1_875_000]
    );
}

#[test]
fn bad_entries_are_ignored_and_counted() {
    let (chart, report) = DwiDecoder::decode_with_report(
        &dwi(
            "#CHANGEBPM:16=0,bad,8=;\n#FREEZE:1=2=3;",
            "SINGLE:BASIC:4:22",
        ),
        0,
    )
    .expect("decodes");
    assert_eq!(taps(&chart), [(0, 1), (250_000, 1)]);
    assert_eq!(report.warnings.len(), 4, "{:?}", report.warnings);
}

#[test]
fn couple_and_charts_without_steps_are_skipped_with_a_message() {
    let data = dwi(
        "",
        "COUPLE:BASIC:4:20:80;\n#SINGLE:BASIC:4:;\n#SINGLE:MANIAC:9:20",
    );
    let variants = DwiDecoder::variants(&data).expect("variants");
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0].difficulty, "Hard");
    assert_eq!(variants[0].name, "MANIAC");
    assert_eq!(variants[0].meter, 9);
    let (_, report) = DwiDecoder::decode_with_report(&data, 0).expect("decodes");
    assert_eq!(report.skipped.len(), 2, "{:?}", report.skipped);
    assert!(report.skipped[0].contains("dance-couple"));
    assert!(DwiDecoder::decode_variant(&data, 1).is_err());
}

#[test]
fn a_file_without_a_bpm_or_with_a_negative_freeze_plays_nothing() {
    let no_bpm = b"#TITLE:T;\n#SINGLE:BASIC:4:22;\n";
    let error = DwiDecoder::decode(no_bpm).expect_err("no BPM");
    assert!(error.to_string().contains("BPM"), "{error}");
    let negative = dwi("#FREEZE:8=-100;", "SINGLE:BASIC:4:22");
    assert!(DwiDecoder::decode(&negative).is_err());
}

#[test]
fn the_difficulty_names_of_the_loader() {
    let data = dwi(
        "",
        "SINGLE:BASIC:1:20;\n#SINGLE:ANOTHER:;\n#SINGLE:SMANIAC:12.5:20;\n#SINGLE:LIGHT:3:20;\n#SINGLE:WEIRD:3:20",
    );
    let variants = DwiDecoder::variants(&data).expect("variants");
    // `#SINGLE:ANOTHER:;` has no steps and is skipped.
    let listed: Vec<(&str, u32)> = variants
        .iter()
        .map(|v| (v.difficulty.as_str(), v.meter))
        .collect();
    assert_eq!(
        listed,
        [("Easy", 1), ("Challenge", 12), ("Easy", 3), ("WEIRD", 3)]
    );
    // An empty rating is 1.
    let chart = dwi("", "SINGLE:BASIC::20");
    assert_eq!(DwiDecoder::variants(&chart).unwrap()[0].meter, 1);
}

#[test]
fn metadata_text_and_audio() {
    let mut data = b"#TITLE:Caf\xe9 - demo;\n#ARTIST:A: B;\n#FILE:C:\\music\\song.mp3;\n".to_vec();
    data.extend_from_slice(
        b"#BPM:120;\n#SAMPLESTART:1:05.5;\n#SAMPLELENGTH:0.015;\n#SINGLE:BASIC:4:20;\n",
    );
    let chart = decode(&data);
    assert_eq!(chart.metadata.title.as_str(), "Café - demo");
    assert_eq!(chart.metadata.artist.as_str(), "A:B");
    assert_eq!(chart.metadata.audio_file.as_str(), "C:\\music\\song.mp3");
    assert_eq!(chart.metadata.preview_time_us, 65_500_000);
    assert_eq!(chart.metadata.preview_duration_us, 15_000_000);
    assert_eq!(chart.metadata.difficulty_name.as_str(), "Easy");
    assert_eq!(chart.metadata.audio_offset_us, 0);
}

#[test]
fn comments_and_whitespace_inside_the_steps() {
    let chart = decode(&dwi("// a comment\n", "SINGLE:BASIC:4:2 0\n 2\t// gone\n"));
    assert_eq!(taps(&chart), [(0, 1), (500_000, 1)]);
}

#[test]
fn the_auto_decoders_route_dwi_files() {
    use crate::formats::{auto_decode, auto_decode_all, auto_decode_bytes, auto_decode_variant};
    let dir = std::env::temp_dir().join(format!("rox-dwi-auto-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("song.dwi");
    let data = dwi(
        "",
        "SINGLE:BASIC:4:20;\n#SINGLE:MANIAC:9:2020;\n#COUPLE:BASIC:4:20:20;\n#DOUBLE:ANOTHER:7:20:80",
    );
    std::fs::write(&path, &data).expect("write");
    assert_eq!(auto_decode(&path).expect("first").notes.len(), 1);
    assert_eq!(
        auto_decode_variant(&path, 1).expect("second").notes.len(),
        2
    );
    assert_eq!(auto_decode_variant(&path, 2).expect("double").key_count, 8);
    assert!(auto_decode_variant(&path, 3).is_err());
    assert_eq!(
        auto_decode_bytes(&path, &data).expect("bytes").notes.len(),
        1
    );
    let all = auto_decode_all(&path)
        .expect("all")
        .expect("a multi-chart format");
    let names: Vec<&str> = all.iter().map(|variant| variant.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "dance-single BASIC",
            "dance-single MANIAC",
            "dance-double ANOTHER"
        ]
    );
    assert!(all.iter().all(|variant| variant.chart.is_ok()));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_msd_is_read_as_dwi_with_either_parameter_order_and_no_gap() {
    use super::MsdDecoder;
    // 120 BPM: an 8th note is 0.25 s. The spec writes the level first, real files the mode first; `#GAP` is in the simulator's dots.
    let file = "#TITLE:T;\n#BPM:120;\n#GAP:-104\n\n#SINGLE:4:BASIC:2000 8000;\n#SINGLE:MANIAC:7:\n(4646) 0B;\n#COUPLE:BASIC:3:2:8;\n";
    let variants = MsdDecoder::variants(file.as_bytes()).unwrap();
    let names: Vec<(&str, u32)> = variants.iter().map(|v| (v.difficulty.as_str(), v.meter)).collect();
    assert_eq!(names, vec![("Easy", 4), ("Hard", 7)]);
    let (basic, report) = MsdDecoder::decode_with_report(file.as_bytes(), 0).unwrap();
    assert_eq!(taps(&basic), vec![(0, 1), (1_000_000, 2)]);
    assert!(report.warnings.iter().any(|w| w.contains("#GAP -104")));
    // `(4646)` is four 16th notes, then `B` (left + right) on the next 8th.
    let maniac = MsdDecoder::decode_variant(file.as_bytes(), 1).unwrap();
    assert_eq!(taps(&maniac), vec![(0, 0), (125_000, 3), (250_000, 0), (375_000, 3), (750_000, 0), (750_000, 3)]);
    // Shift-JIS text that is not UTF-8.
    let (sjis, _, _) = encoding_rs::SHIFT_JIS.encode("#TITLE:電車;\n#BPM:120;\n#SINGLE:BASIC:1:20;\n");
    assert_eq!(MsdDecoder::decode(&sjis).unwrap().metadata.title.as_str(), "電車");
}
