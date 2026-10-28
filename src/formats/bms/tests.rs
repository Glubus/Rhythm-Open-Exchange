#![allow(clippy::cast_possible_truncation)] // expected times are computed in f64 seconds
//! Synthetic charts: one per rule of the decoder. Every chart is written here, so the expected times can be
//! checked by hand (at 120 BPM a beat is 0.5 s and a 4/4 measure 2 s).
//!
//! `line(m, "11", "0101")` is the statement `#00111:0101`: measure `m`, channel `11`.

use rox::codec::Decoder;
use rox::formats::bms::{BmsDecoder, BmsReport, PmsDecoder};
use rox::model::{Note, NoteType, RoxChart, TimingPoint};

fn line(measure: u16, channel: &str, data: &str) -> String {
    format!("#{measure:03}{channel}:{data}\n")
}

/// `headers` then the channel lines.
fn doc(headers: &str, body: &[(u16, &str, &str)]) -> String {
    let mut text = format!("{headers}\n");
    for (m, ch, data) in body {
        text.push_str(&line(*m, ch, data));
    }
    text
}

fn decode(text: &str) -> RoxChart {
    BmsDecoder::decode(text.as_bytes()).expect("decodes")
}

fn report(text: &str) -> (RoxChart, BmsReport) {
    BmsDecoder::decode_with_report(text.as_bytes()).expect("decodes")
}

fn pms(text: &str) -> RoxChart {
    PmsDecoder::decode(text.as_bytes()).expect("decodes")
}

fn times(chart: &RoxChart) -> Vec<i64> {
    chart.notes.iter().map(|n| n.time_us).collect()
}

fn columns(chart: &RoxChart) -> Vec<u8> {
    chart.notes.iter().map(|n| n.column).collect()
}

fn bpms(chart: &RoxChart) -> Vec<(i64, f32)> {
    chart
        .timing_points
        .iter()
        .filter_map(|tp| match tp {
            TimingPoint::Bpm { time_us, bpm, .. } => Some((*time_us, *bpm)),
            TimingPoint::Sv { .. } => None,
        })
        .collect()
}

#[test]
fn positions_are_slots_of_the_measure() {
    // Measure 1, 4 slots, objects on slots 0 and 2: beats 4 and 6 = 2.0 s and 3.0 s.
    let chart = decode(&doc("#BPM 120", &[(1, "11", "01000100")]));
    assert_eq!(chart.key_count, 5);
    assert_eq!(times(&chart), [2_000_000, 3_000_000]);
    assert!(
        chart
            .notes
            .iter()
            .all(|n| n.column == 0 && matches!(n.note_type, NoteType::Tap))
    );
}

#[test]
fn equal_fractions_of_different_lines_are_one_position() {
    // 1/2 written as slot 1 of 2 and as slot 2 of 4: the later line wins, one note.
    let chart = decode(&doc(
        "#BPM 120",
        &[(0, "11", "0001"), (0, "11", "00000100")],
    ));
    assert_eq!(times(&chart), [1_000_000]);
}

#[test]
fn lines_of_one_channel_are_merged_and_zero_never_overwrites() {
    let body = [
        (0, "11", "01010101"),
        (0, "11", "0000020000000000"),
        (0, "11", "0003"),
    ];
    let chart = decode(&doc("#BPM 120", &body));
    // 02 replaces the object at 1/4, 03 the one at 1/2; the leading `00` of "0003" does not erase the one at 0.
    assert_eq!(times(&chart), [0, 500_000, 1_000_000, 1_500_000]);
}

#[test]
fn measure_length_scales_the_measure_only() {
    // Measure 0 is 3/4 (3 beats = 1.5 s): measure 1 starts at 1.5 s.
    let body = [(0, "02", "0.75"), (0, "11", "01"), (1, "11", "01")];
    assert_eq!(times(&decode(&doc("#BPM 120", &body))), [0, 1_500_000]);
}

#[test]
fn zero_length_measure_keeps_its_objects_at_one_instant() {
    let body = [
        (0, "02", "0"),
        (0, "11", "01"),
        (0, "12", "01"),
        (1, "13", "01"),
    ];
    let chart = decode(&doc("#BPM 120", &body));
    assert_eq!(times(&chart), [0, 0, 0]);
    assert_eq!(columns(&chart), [0, 1, 2]);
}

#[test]
fn two_notes_at_one_time_in_one_column_keep_the_first() {
    let body = [(0, "02", "0"), (0, "11", "0101"), (1, "11", "01")];
    let (chart, report) = report(&doc("#BPM 120", &body));
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(report.overlapping_notes_dropped, 2);
}

#[test]
fn bpm_channels_03_and_08() {
    // 03 is hexadecimal: 3C = 60 BPM. 08 reads #BPM01.
    let headers = "#BPM 120\n#BPM01 150.5";
    let body = [
        (1, "03", "3C"),
        (2, "08", "01"),
        (0, "11", "01"),
        (1, "11", "01"),
        (2, "11", "01"),
        (3, "11", "01"),
    ];
    let chart = decode(&doc(headers, &body));
    // measure 1 at 2 s (4 beats at 120), measure 2 after 4 beats at 60 BPM (4 s), measure 3 after 4 beats at 150.5.
    let m3 = 6_000_000 + (240.0_f64 / 150.5 * 1e6).round() as i64;
    assert_eq!(times(&chart), [0, 2_000_000, 6_000_000, m3]);
    assert_eq!(
        bpms(&chart),
        [(0, 120.0), (2_000_000, 60.0), (6_000_000, 150.5)]
    );
}

#[test]
fn exbpm_is_the_same_table_as_bpmxx() {
    let chart = decode(&doc(
        "#BPM 120\n#EXBPM0A 90",
        &[(1, "08", "0A"), (0, "11", "01")],
    ));
    assert_eq!(bpms(&chart), [(0, 120.0), (2_000_000, 90.0)]);
}

#[test]
fn a_bpm_at_beat_zero_replaces_the_initial_one() {
    let body = [(0, "03", "78"), (0, "11", "01"), (1, "11", "01")];
    let chart = decode(&doc("#BPM 100", &body));
    assert_eq!(bpms(&chart), [(0, 120.0)]);
    assert_eq!(times(&chart), [0, 2_000_000]);
}

#[test]
fn negative_and_undefined_bpm_changes_are_ignored() {
    let body = [(1, "08", "01"), (2, "08", "02"), (0, "11", "01")];
    let (chart, report) = report(&doc("#BPM 120\n#BPM01 -5", &body));
    assert_eq!(bpms(&chart), [(0, 120.0)]);
    assert_eq!(report.ignored_bpm_changes, 2);
}

#[test]
fn stop_pauses_after_the_objects_of_its_position() {
    // #STOP01 96 = 96/48 = 2 beats = 1 s at 120 BPM, at the start of measure 1 (beat 4 = 2 s).
    let body = [
        (1, "09", "01"),
        (0, "11", "01"),
        (1, "11", "01"),
        (2, "11", "01"),
    ];
    let chart = decode(&doc("#BPM 120\n#STOP01 96", &body));
    // the note at the stop position is at the start of the pause; the next measure is 1 s later than without it.
    assert_eq!(times(&chart), [0, 2_000_000, 5_000_000]);
}

#[test]
fn stop_is_not_scaled_by_the_measure_length() {
    let body = [
        (0, "02", "0.5"),
        (0, "09", "01"),
        (0, "11", "01"),
        (1, "11", "01"),
    ];
    let chart = decode(&doc("#BPM 120\n#STOP01 96", &body));
    // measure 0 is 2 beats (1 s) and the stop at its start still lasts 2 beats (1 s).
    assert_eq!(times(&chart), [0, 2_000_000]);
}

#[test]
fn stop_uses_the_bpm_changed_at_the_same_position() {
    // 96 units = 2 beats; the BPM becomes 60 at the same position: the pause is 2 s, not 1 s.
    let body = [
        (1, "03", "3C"),
        (1, "09", "01"),
        (0, "11", "01"),
        (1, "11", "01"),
        (2, "11", "01"),
    ];
    let chart = decode(&doc("#BPM 120\n#STOP01 96", &body));
    assert_eq!(times(&chart), [0, 2_000_000, 8_000_000]);
    assert_eq!(bpms(&chart), [(0, 120.0), (2_000_000, 60.0)]);
}

#[test]
fn long_notes_with_lntype_1_channels() {
    // Channel 51 alternates head / tail: 0 to 1/2 of measure 0, then measure 1 to measure 2.
    let body = [(0, "51", "01000100"), (1, "51", "01"), (2, "51", "01")];
    let chart = decode(&doc("#LNTYPE 1\n#BPM 120", &body));
    let holds: Vec<(i64, i64)> = chart
        .notes
        .iter()
        .map(|n| (n.time_us, n.duration_us()))
        .collect();
    assert_eq!(holds, [(0, 1_000_000), (2_000_000, 2_000_000)]);
    assert!(chart.notes.iter().all(Note::is_hold));
}

#[test]
fn lntype_1_is_the_default() {
    let chart = decode(&doc("#BPM 120", &[(0, "51", "0101")]));
    assert!(chart.notes[0].is_hold());
}

#[test]
fn an_unclosed_long_note_head_is_a_tap() {
    let (chart, report) = report(&doc("#BPM 120", &[(0, "51", "01")]));
    assert!(matches!(chart.notes[0].note_type, NoteType::Tap));
    assert_eq!(report.unclosed_long_notes, 1);
}

#[test]
fn lnobj_closes_the_note_before_it_and_is_not_a_note() {
    let body = [
        (0, "11", "01000000"),
        (0, "12", "0100"),
        (2, "11", "ZZ"),
        (2, "12", "0001"),
    ];
    let (chart, report) = report(&doc("#LNOBJ zz\n#BPM 120", &body));
    let hold = chart.notes.iter().find(|n| n.is_hold()).expect("one hold");
    assert_eq!(
        (hold.time_us, hold.duration_us(), hold.column),
        (0, 4_000_000, 0)
    );
    assert_eq!(chart.notes.iter().filter(|n| n.column == 0).count(), 1);
    assert_eq!(chart.notes.len(), 3);
    assert_eq!(report.orphan_long_note_ends, 0);
}

#[test]
fn an_lnobj_without_a_note_before_it_is_dropped() {
    let (chart, report) = report(&doc("#LNOBJ ZZ\n#BPM 120", &[(0, "11", "ZZ01")]));
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(report.orphan_long_note_ends, 1);
}

#[test]
fn lntype_2_runs_end_at_the_first_empty_slot() {
    // 8 slots of 0.25 s: a run on slots 1..=3 ends at slot 4; slot 6 alone lasts one slot.
    let chart = decode(&doc(
        "#LNTYPE 2\n#BPM 120",
        &[(0, "51", "0001010100000100")],
    ));
    let holds: Vec<(i64, i64)> = chart
        .notes
        .iter()
        .map(|n| (n.time_us, n.duration_us()))
        .collect();
    assert_eq!(holds, [(250_000, 750_000), (1_500_000, 250_000)]);
}

#[test]
fn lntype_2_runs_continue_over_a_bar_line() {
    // last slot of measure 0 (beat 3.5 = 1.75 s) and first slot of measure 1: ends after that slot (beat 6 = 3 s).
    let body = [(0, "52", "0000000000000001"), (1, "52", "0100")];
    let chart = decode(&doc("#LNTYPE 2\n#BPM 120", &body));
    let holds: Vec<(i64, i64, u8)> = chart
        .notes
        .iter()
        .map(|n| (n.time_us, n.duration_us(), n.column))
        .collect();
    assert_eq!(holds, [(1_750_000, 1_250_000, 1)]);
}

#[test]
fn random_always_takes_the_first_branch() {
    let text = "#BPM 120\n#RANDOM 3\n#IF 1\n#00011:01\n#ENDIF\n#IF 2\n#00012:01\n#ENDIF\n#IF 3\n#00013:01\n#ENDIF\n";
    let (chart, report) = report(text);
    assert_eq!(columns(&chart), [0]);
    assert_eq!(report.branches_skipped, 2);
}

#[test]
fn random_else_elseif_setrandom_and_nesting() {
    let text = "#BPM 120\n#RANDOM 2\n#IF 2\n#00011:01\n#ELSEIF 1\n#00012:01\n#ELSE\n#00013:01\n#ENDIF\n\
                #SETRANDOM 2\n#IF 2\n#00111:01\n#RANDOM 2\n#IF 1\n#00112:01\n#ENDIF\n#IF 2\n#00113:01\n#ENDIF\n\
                #ENDRANDOM\n#ENDIF\n#ENDRANDOM\n";
    let chart = decode(text);
    // `#RANDOM 2` draws 1 -> the ELSEIF 1 branch (column 1, measure 0). `#SETRANDOM 2` -> measure 1 column 0 and,
    // inside it, a new `#RANDOM 2` draws 1 -> column 1.
    assert_eq!(times(&chart), [0, 2_000_000, 2_000_000]);
    assert_eq!(columns(&chart), [1, 0, 1]);
}

#[test]
fn if_without_random_uses_one_and_headers_can_be_inside_a_branch() {
    let chart =
        decode("#BPM 120\n#IF 1\n#TITLE Taken\n#ENDIF\n#IF 2\n#TITLE Skipped\n#ENDIF\n#00011:01\n");
    assert_eq!(chart.metadata.title, "Taken");
}

#[test]
fn an_unbalanced_endif_and_an_unterminated_if_do_not_break_the_file() {
    let chart = decode("#BPM 120\n#ENDIF\n#ENDRANDOM\n#00011:01\n#IF 2\n#00012:01\n");
    assert_eq!(columns(&chart), [0]);
}

#[test]
fn shift_jis_metadata_is_read() {
    let (bytes, _, _) = encoding_rs::SHIFT_JIS
        .encode("#TITLE 日本語のタイトル\n#ARTIST ＡＢＣ\n#BPM 120\n#00011:01\n");
    let (chart, report) = BmsDecoder::decode_with_report(&bytes).unwrap();
    assert_eq!(chart.metadata.title, "日本語のタイトル");
    assert_eq!(chart.metadata.artist, "ＡＢＣ");
    assert_eq!(report.charset, "Shift_JIS");
}

#[test]
fn utf8_utf16_and_euc_kr_are_detected() {
    let (chart, report) = report("#TITLE こんにちは\n#BPM 120\n#00011:01\n");
    assert_eq!(
        (chart.metadata.title.as_str(), report.charset),
        ("こんにちは", "UTF-8")
    );
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice("#TITLE bom\n#00011:01\n".as_bytes());
    assert_eq!(BmsDecoder::decode(&bom).unwrap().metadata.title, "bom");
    let mut utf16 = vec![0xFF, 0xFE];
    for unit in "#TITLE wide\n#00011:01\n".encode_utf16() {
        utf16.extend_from_slice(&unit.to_le_bytes());
    }
    assert_eq!(BmsDecoder::decode(&utf16).unwrap().metadata.title, "wide");
    let (korean, _, _) = encoding_rs::EUC_KR.encode("#TITLE 한국어\n#00011:01\n");
    let (chart, report) = BmsDecoder::decode_with_report(&korean).unwrap();
    assert_eq!(
        (chart.metadata.title.as_str(), report.charset),
        ("한국어", "EUC-KR")
    );
}

#[test]
fn keysounds_go_to_the_hitsound_table_in_order_of_use() {
    let body = [
        (0, "11", "0100Z9"),
        (0, "12", "Z9000100"),
        (0, "13", "0200"),
    ];
    let chart = decode(&doc(
        "#BPM 120\n#WAV01 kick.wav\n#WAVZ9 snare\\hit.wav",
        &body,
    ));
    let files: Vec<&str> = chart.hitsounds.iter().map(|h| h.file.as_str()).collect();
    assert_eq!(files, ["kick.wav", "snare/hit.wav"]);
    // by time: t0 key1 `01`, key2 `Z9`, key3 `02` (undefined, no hitsound); 1 s key2 `01`; 1.33 s key1 `Z9`.
    let indexes: Vec<Option<u16>> = chart.notes.iter().map(|n| n.hitsound_index).collect();
    assert_eq!(indexes, [Some(0), Some(1), None, Some(0), Some(1)]);
}

#[test]
fn a_single_background_sound_is_the_audio_and_sets_the_time_origin() {
    // The only BGM object is at measure 1 (2 s): the audio starts there, so every time moves back by 2 s.
    let body = [(1, "01", "01"), (0, "11", "02"), (2, "11", "02")];
    let (chart, report) = report(&doc("#BPM 120\n#WAV01 song.ogg\n#WAV02 unused.wav", &body));
    assert_eq!(chart.metadata.audio_file, "song.ogg");
    assert!(report.single_audio);
    assert_eq!(times(&chart), [-2_000_000, 2_000_000]);
    assert_eq!(bpms(&chart), [(-2_000_000, 120.0)]);
}

#[test]
fn several_background_sounds_leave_the_audio_empty() {
    let body = [(0, "01", "0102"), (0, "11", "01")];
    let (chart, report) = report(&doc("#BPM 120\n#WAV01 a.wav\n#WAV02 b.wav", &body));
    assert_eq!(chart.metadata.audio_file, "");
    assert!(!report.single_audio);
    assert_eq!(report.bgm_objects_skipped, 2);
    assert_eq!(times(&chart), [0]);
}

#[test]
fn layouts_follow_the_lanes_used() {
    assert_eq!(
        decode(&doc("#BPM 120", &[(0, "11", "01"), (0, "15", "01")])).key_count,
        5
    );
    // 5 keys + scratch (16): the scratch is column 0, key 1 column 1
    let chart = decode(&doc("#BPM 120", &[(0, "16", "01"), (0, "11", "0001")]));
    assert_eq!(chart.key_count, 6);
    assert_eq!(columns(&chart), [0, 1]);
    // 7 keys + scratch: channels 18 and 19 are keys 6 and 7
    let body = [
        (0, "16", "01"),
        (0, "18", "0100"),
        (0, "19", "0001"),
        (0, "11", "01"),
    ];
    let chart = decode(&doc("#BPM 120", &body));
    assert_eq!(chart.key_count, 8);
    let mut by_column: Vec<(u8, i64)> = chart.notes.iter().map(|n| (n.column, n.time_us)).collect();
    by_column.sort_unstable();
    assert_eq!(by_column, [(0, 0), (1, 0), (6, 0), (7, 1_000_000)]);
    // 7 keys, no scratch
    let chart = decode(&doc("#BPM 120", &[(0, "18", "01")]));
    assert_eq!((chart.key_count, chart.notes[0].column), (7, 5));
}

#[test]
fn double_play_has_both_halves_and_both_scratches() {
    let body = [
        (0, "16", "01"),
        (0, "26", "01"),
        (0, "11", "0001"),
        (0, "21", "01"),
        (0, "28", "0001"),
        (0, "29", "01"),
    ];
    let chart = decode(&doc("#BPM 120", &body));
    // scratch1 0 | 1P keys 1-7 = 1..=7 | 2P keys 1-7 = 8..=14 | scratch2 15
    assert_eq!(chart.key_count, 16);
    let mut got: Vec<(u8, i64)> = chart.notes.iter().map(|n| (n.column, n.time_us)).collect();
    got.sort_unstable();
    assert_eq!(
        got,
        [
            (0, 0),
            (1, 1_000_000),
            (8, 0),
            (13, 1_000_000),
            (14, 0),
            (15, 0)
        ]
    );
    // 10K (5 + 5) and 12K (5K+scratch each side)
    assert_eq!(
        decode(&doc("#BPM 120", &[(0, "11", "01"), (0, "21", "01")])).key_count,
        10
    );
    assert_eq!(
        decode(&doc(
            "#BPM 120",
            &[(0, "16", "01"), (0, "11", "01"), (0, "21", "01")]
        ))
        .key_count,
        12
    );
}

#[test]
fn a_chart_with_only_2p_lanes_is_laid_out_like_a_single_chart() {
    let chart = decode(&doc("#BPM 120", &[(0, "26", "01"), (0, "21", "0001")]));
    assert_eq!(chart.key_count, 6);
    assert_eq!(columns(&chart), [0, 1]);
}

#[test]
fn the_free_zone_lane_is_dropped() {
    let (chart, report) = report(&doc("#BPM 120", &[(0, "17", "01"), (0, "11", "01")]));
    assert_eq!(chart.key_count, 5);
    assert_eq!(report.notes_outside_layout, 1);
}

#[test]
fn invisible_objects_are_not_notes_and_mines_are_mines() {
    let body = [
        (0, "31", "01"),
        (0, "11", "01"),
        (0, "D1", "000A"),
        (0, "D6", "01"),
    ];
    let (chart, report) = report(&doc("#BPM 120", &body));
    assert_eq!(chart.notes.len(), 2, "the tap and the mine of key 1");
    assert!(chart.notes[0].time_us == 0 && !chart.notes[0].is_mine());
    assert!(chart.notes[1].is_mine() && chart.notes[1].time_us == 1_000_000);
    // no scratch column, so the scratch mine has no place
    assert_eq!(report.notes_outside_layout, 1);
}

#[test]
fn notes_inside_a_long_note_are_dropped() {
    let body = [(0, "51", "0101"), (0, "11", "00010000")];
    let (chart, report) = report(&doc("#LNTYPE 1\n#BPM 120", &body));
    assert_eq!(chart.notes.len(), 1);
    assert!(chart.notes[0].is_hold());
    assert_eq!(report.overlapping_notes_dropped, 1);
}

#[test]
fn pms_standard_has_nine_columns() {
    // buttons 1..5 = 11..15, buttons 6..9 = 22..25
    let body = [
        (0, "11", "01"),
        (0, "15", "0001"),
        (0, "22", "01"),
        (0, "25", "0001"),
        (0, "21", "01"),
    ];
    let chart = pms(&doc("#BPM 120", &body));
    assert_eq!(chart.key_count, 9);
    let mut cols = columns(&chart);
    cols.sort_unstable();
    assert_eq!(cols, [0, 4, 5, 8], "channel 21 is not a PMS button");
}

#[test]
fn pms_bme_type_uses_channels_18_19_16_17() {
    let body = [
        (0, "11", "01"),
        (0, "18", "01"),
        (0, "19", "01"),
        (0, "16", "01"),
        (0, "17", "01"),
    ];
    let chart = pms(&doc("#BPM 120", &body));
    assert_eq!(chart.key_count, 9);
    assert_eq!(columns(&chart), [0, 5, 6, 7, 8]);
}

#[test]
fn lowercase_blanks_and_line_ends_are_tolerated() {
    let text =
        "#bpm 120\r#title  Spaced Out  \r\n  #00011:01\n\n#0011a:zz\n*comment\n#00112:0001\n";
    let chart = decode(text);
    assert_eq!(chart.metadata.title, "Spaced Out");
    assert_eq!(chart.notes.len(), 2);
}

#[test]
fn headers_given_twice_keep_the_last_and_the_bpm_defaults_to_130() {
    let chart = decode("#TITLE first\n#TITLE second\n#00011:01\n");
    assert_eq!(chart.metadata.title, "second");
    assert_eq!(bpms(&chart), [(0, 130.0)]);
}

#[test]
fn metadata_fields() {
    let text = "#TITLE t\n#SUBTITLE [ANOTHER]\n#ARTIST a\n#GENRE g\n#PLAYLEVEL 12\n#STAGEFILE bg\\x.png\n#BPM 150\n#00011:01\n";
    let chart = decode(text);
    assert_eq!(chart.metadata.difficulty_name, "[ANOTHER]");
    assert_eq!(chart.metadata.difficulty_value, Some(12.0));
    assert_eq!(chart.metadata.genre.as_deref(), Some("g"));
    assert_eq!(chart.metadata.background_file.as_deref(), Some("bg/x.png"));
    assert_eq!(
        decode("#DIFFICULTY 4\n#00011:01\n")
            .metadata
            .difficulty_name,
        "Another"
    );
}

#[test]
fn a_file_without_playable_objects_is_refused() {
    assert!(BmsDecoder::decode(b"#TITLE nothing\n#00101:01\n").is_err());
    assert!(BmsDecoder::decode(b"").is_err());
    assert!(PmsDecoder::decode(b"#BPM 120\n#001D1:01\n").is_err());
}

#[test]
fn hostile_input_does_not_loop_or_panic() {
    let text = "#BPM 120\n#STOP01 2147483647\n#00109:01\n#99999:01\n#99911:01\n#00002:1e300\n#00011:0\n#00111:01 trailing\n\u{0}\u{1}#";
    let _ = BmsDecoder::decode(text.as_bytes());
    let _ = BmsDecoder::decode(&[0xFF; 64]);
    let _ =
        BmsDecoder::decode(b"#00011:01\n#00012:010101\n#LNOBJ 01\n#RANDOM\n#IF\n#ELSE\n#ENDIF\n");
}

/// The hand-written chart of `assets/bms/features.bms`: every feature of the decoder in one file, times worked out by hand
/// (120 BPM, 180 BPM from measure 1, measure 2 half long, a one-beat stop at the start of measure 3).
#[test]
fn the_features_asset_decodes_as_worked_out_by_hand() {
    let data = rox_test_utils::get_test_asset("bms/features.bms");
    let (chart, report) = BmsDecoder::decode_with_report(&data).unwrap();
    assert_eq!(report.layout, "1P 5K+scratch (6 columns)");
    assert_eq!(
        (
            chart.key_count,
            report.single_audio,
            report.branches_skipped
        ),
        (6, true, 1)
    );
    assert_eq!(chart.metadata.title, "ROX BMS features (first branch)");
    assert_eq!(chart.metadata.difficulty_name, "Hyper");
    assert_eq!(chart.metadata.audio_file, "song.ogg");
    assert_eq!(bpms(&chart), [(0, 120.0), (2_000_000, 180.0)]);
    let files: Vec<&str> = chart.hitsounds.iter().map(|h| h.file.as_str()).collect();
    assert_eq!(files, ["kick.wav", "snare.wav"]);

    let us = |seconds: f64| (seconds * 1e6).round() as i64;
    // 180 BPM from beat 4 (2 s): a beat is 1/3 s. Measure 2 starts at beat 8, measure 3 at beat 10 (2 + 6/3 = 4 s, the
    // note there is at the start of the one-beat stop), measure 4 at beat 14 after the stop (4 + 1/3 + 4/3 s), measure 5 at beat 18.
    let (m4, m5) = (
        us(4.0 + 1.0 / 3.0 + 4.0 / 3.0),
        us(4.0 + 1.0 / 3.0 + 8.0 / 3.0),
    );
    // (time, column, duration, mine)
    let expected = [
        (0, 1, 0, false),
        (1_000_000, 0, 0, false),
        (1_000_000, 1, 0, false),
        (2_000_000, 1, 0, false),
        (us(2.0 + 1.0 / 3.0), 1, 0, false),
        (us(2.0 + 2.0 / 3.0), 1, 0, false),
        (3_000_000, 1, 0, false),
        (us(2.0 + 5.0 / 3.0), 2, 0, false),
        (4_000_000, 3, m4 - 4_000_000, false), // at one time a hold comes before a tap
        (4_000_000, 1, 0, false),
        (m4, 4, m5 - m4, false),
        (m4, 1, 0, true),
        (m5, 1, 0, false),
        (m5, 5, 0, false),
    ];
    let got: Vec<(i64, u8, i64, bool)> = chart
        .notes
        .iter()
        .map(|n| (n.time_us, n.column, n.duration_us(), n.is_mine()))
        .collect();
    assert_eq!(got, expected);
}

#[test]
fn the_pms_asset_uses_the_nine_buttons() {
    let data = rox_test_utils::get_test_asset("bms/nine.pms");
    let (chart, report) = PmsDecoder::decode_with_report(&data).unwrap();
    assert_eq!(
        (chart.key_count, report.layout.as_str()),
        (9, "PMS 9 buttons (standard)")
    );
    let got: Vec<(i64, u8)> = chart.notes.iter().map(|n| (n.time_us, n.column)).collect();
    let expected = [
        (0, 0),
        (0, 4),
        (0, 8),
        (500_000, 1),
        (500_000, 5),
        (1_000_000, 2),
        (1_000_000, 6),
        (1_500_000, 3),
        (1_500_000, 7),
    ];
    assert_eq!(got, expected);
}

#[test]
fn auto_decode_reads_every_bms_extension_and_refuses_to_write() {
    use rox::formats::{auto_decode_bytes, auto_encode};
    let data = rox_test_utils::get_test_asset("bms/features.bms");
    for name in ["a.bms", "a.BME", "a.bml"] {
        assert_eq!(
            auto_decode_bytes(name, &data).unwrap().key_count,
            6,
            "{name}"
        );
    }
    let pms = rox_test_utils::get_test_asset("bms/nine.pms");
    assert_eq!(auto_decode_bytes("a.pms", &pms).unwrap().key_count, 9);
    let chart = auto_decode_bytes("a.bms", &data).unwrap();
    let out = std::env::temp_dir().join(format!("rox-bms-{}.bms", std::process::id()));
    assert!(auto_encode(&chart, &out).is_err());
    assert!(!out.exists());
}

#[test]
fn unsafe_file_names_are_read_as_undefined() {
    let body = [(0, "01", "01"), (0, "11", "0203"), (0, "12", "0400")];
    let head = "#BPM 120\n#WAV01 ../song.ogg\n#WAV02 /etc/kick.wav\n#WAV03 C:\\snare.wav\n#WAV04 ok\\hit.wav\n#STAGEFILE ..\\bg.png\n#BACKBMP sub/bg.png";
    let (chart, report) = report(&doc(head, &body));
    assert_eq!(report.unsafe_names, 4);
    assert!(!report.single_audio);
    assert_eq!(chart.metadata.audio_file, "");
    assert_eq!(chart.metadata.background_file.as_deref(), Some("sub/bg.png"), "the refused STAGEFILE falls back to BACKBMP");
    let files: Vec<&str> = chart.hitsounds.iter().map(|h| h.file.as_str()).collect();
    assert_eq!(files, ["ok/hit.wav"]);
}

#[test]
fn the_audio_plan_has_every_background_and_note_sound_at_chart_time() {
    use rox::audio_plan::SoundEvent;
    // 120 BPM. Two BGM lines at once (never merged), a keysound on key 1 and one in the free zone (a dropped lane),
    // an invisible object (silent), a mine (silent), an LNOBJ long note (head sounds, tail does not).
    let body = [
        (0, "01", "0100"),
        (0, "01", "0200"),
        (0, "11", "0003"),
        (0, "17", "04"),
        (0, "31", "05"),
        (0, "D1", "06"),
        (1, "12", "03"),
        (1, "52", "0301"),
        (2, "12", "ZZ"),
    ];
    let head = "#BPM 120\n#LNOBJ ZZ\n#WAV01 a.wav\n#WAV02 b.wav\n#WAV03 k.wav\n#WAV04 f.wav\n#WAV05 i.wav\n#WAV06 m.wav";
    let (_, _, plan) = BmsDecoder::decode_with_audio_plan(doc(head, &body).as_bytes()).unwrap();
    let got: Vec<(i64, &str)> = plan.events.iter().map(|e| (e.time_us, e.file.as_str())).collect();
    assert_eq!(
        got,
        [(0, "a.wav"), (0, "b.wav"), (0, "f.wav"), (1_000_000, "k.wav"), (2_000_000, "k.wav"), (2_000_000, "k.wav")]
    );
    assert_eq!(plan.events[0], SoundEvent::whole(0, "a.wav"));
    assert_eq!(plan.files(), ["a.wav", "b.wav", "f.wav", "k.wav"]);
}

#[test]
fn the_audio_plan_does_not_change_the_chart() {
    let body = [(0, "01", "01"), (0, "11", "0202")];
    let text = doc("#BPM 120\n#WAV01 a.wav\n#WAV02 k.wav", &body);
    let (with, ..) = BmsDecoder::decode_with_audio_plan(text.as_bytes()).unwrap();
    assert_eq!(with, decode(&text));
}
