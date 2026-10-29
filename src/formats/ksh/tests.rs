//! Tests of the `.ksh` decoder. Every file is written by hand in the test (the format's own examples are used where it gives one); no real
//! chart is used: fan charts carry no licence to redistribute them. Times are computed by hand in the comments: at 120 BPM a beat is
//! 0.5 s (500 000 µs) and a 4/4 measure 2 s.

use rox::codec::Decoder;
use rox::model::{RoxChart, TimingPoint};

use super::{COLUMNS, KshDecoder, KshReport, is_ksh_chart_file, looks_like_ksh};

// ---------------------------------------------------------------------------------------------------- builders

/// A header with the tempo 120, then the first bar line, then `body`.
fn file(header: &str, body: &str) -> Vec<u8> {
    format!("title=T\nt=120\n{header}--\n{body}").into_bytes()
}

fn decode(header: &str, body: &str) -> (RoxChart, KshReport) {
    KshDecoder::decode_with_report(&file(header, body)).unwrap()
}

fn error(header: &str, body: &str) -> String {
    KshDecoder::decode_with_report(&file(header, body))
        .unwrap_err()
        .to_string()
}

/// `(time_us, column, duration_us)` of every note (0 for a tap).
fn notes_of(chart: &RoxChart) -> Vec<(i64, u8, i64)> {
    chart
        .notes
        .iter()
        .map(|note| (note.time_us, note.column, note.duration_us()))
        .collect()
}

/// `(time_us, bpm, signature)` of every timing point.
fn points_of(chart: &RoxChart) -> Vec<(i64, f32, u8)> {
    chart
        .timing_points
        .iter()
        .map(|point| match *point {
            TimingPoint::Bpm {
                time_us,
                bpm,
                signature,
            } => (time_us, bpm, signature),
            TimingPoint::Sv { .. } => panic!("a .ksh has no scroll velocity"),
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------- header

#[test]
fn the_header_gives_the_metadata() {
    let (chart, report) = decode(
        "artist=A\neffect=Charter\ndifficulty=extended\nlevel=15\nm=song.ogg\nbg=desert\njacket=cover.png\npo=1500\nplength=15000\nillustrator=I\n",
        "1000|00|--\n--\n",
    );
    assert_eq!(chart.key_count, COLUMNS);
    assert_eq!(chart.key_count, 6);
    let meta = &chart.metadata;
    assert_eq!(meta.title.as_str(), "T");
    assert_eq!(meta.artist.as_str(), "A");
    assert_eq!(
        meta.creator.as_str(),
        "Charter",
        "`effect` is the chart author"
    );
    assert_eq!(meta.difficulty_name.as_str(), "Extended");
    assert_eq!(meta.difficulty_value, Some(15.0));
    assert_eq!(meta.audio_file.as_str(), "song.ogg");
    assert_eq!(
        meta.background_file.as_deref(),
        Some("cover.png"),
        "a preset background is no file: the jacket is"
    );
    assert_eq!(
        (meta.preview_time_us, meta.preview_duration_us),
        (1_500_000, 15_000_000)
    );
    assert_eq!(meta.audio_offset_us, 0);
    assert_eq!(report.charset, "UTF-8");
    assert_eq!((report.measures, report.chart_lines), (1, 1));
}

#[test]
fn the_defaults_and_the_presets() {
    let (chart, report) = decode("", "1000|00|--\n--\n");
    let meta = &chart.metadata;
    assert_eq!(
        meta.difficulty_name.as_str(),
        "Light",
        "`light` when absent"
    );
    assert_eq!(meta.difficulty_value, Some(1.0), "level 1 when absent");
    assert!(meta.audio_file.is_empty());
    assert_eq!(meta.background_file, None);
    assert_eq!(
        report,
        KshReport {
            charset: "UTF-8",
            measures: 1,
            chart_lines: 1,
            bt_chips: 1,
            ..KshReport::default()
        }
    );
    // The title falls back to the file stem the caller gives, and presets are no files.
    let data = b"title=\ndifficulty=challenge\nlevel=99\nbg=sunset\njacket=nowprinting2\n--\n1000|00|--\n--\n";
    let (chart, report) = KshDecoder::decode_with_report(data).unwrap();
    assert_eq!(chart.metadata.title.as_str(), "");
    assert_eq!(
        KshDecoder::decode_with_title(data, "exh")
            .unwrap()
            .metadata
            .title
            .as_str(),
        "exh"
    );
    assert_eq!(chart.metadata.difficulty_name.as_str(), "Challenge");
    assert_eq!(
        chart.metadata.difficulty_value,
        Some(1.0),
        "a level out of 1..=20 is the default"
    );
    assert_eq!(report.invalid_option_values, 1);
    assert_eq!(chart.metadata.background_file, None);
    let (chart, report) = KshDecoder::decode_with_report(
        b"title=T\ndifficulty=Whatever\nbg=a.png;b.png\n--\n1000|00|--\n--\n",
    )
    .unwrap();
    assert_eq!(
        chart.metadata.difficulty_name.as_str(),
        "Infinite",
        "an unknown difficulty is `infinite`, as the format says"
    );
    assert!(report.unknown_difficulty);
    assert_eq!(
        chart.metadata.background_file.as_deref(),
        Some("a.png"),
        "the first of two backgrounds"
    );
    for (word, expected) in [
        ("light", "Light"),
        ("CHALLENGE", "Challenge"),
        ("infinite", "Infinite"),
    ] {
        let (chart, _) = decode(&format!("difficulty={word}\n"), "1000|00|--\n--\n");
        assert_eq!(chart.metadata.difficulty_name.as_str(), expected);
    }
}

#[test]
fn a_value_is_everything_after_the_first_equal_sign_and_the_last_option_wins() {
    let (chart, _) = decode("title=1+1=2\ntitle=Second=\n", "1000|00|--\n--\n");
    // The first `title=T` of the helper, then the two above: the last one wins.
    assert_eq!(chart.metadata.title.as_str(), "Second=");
}

#[test]
fn m_takes_the_first_file_and_counts_the_others() {
    let (chart, report) = decode(
        "m=song.ogg;song_f.ogg;song_p.ogg;song_fp.ogg\n",
        "1000|00|--\n--\n",
    );
    assert_eq!(chart.metadata.audio_file.as_str(), "song.ogg");
    assert_eq!(report.extra_audio_files, 3);
    let (chart, report) = decode("m=song.ogg;song_f.ogg\n", "1000|00|--\n--\n");
    assert_eq!(
        (chart.metadata.audio_file.as_str(), report.extra_audio_files),
        ("song.ogg", 1)
    );
    let (chart, report) = decode("m=sub\\song.ogg\n", "1000|00|--\n--\n");
    assert_eq!(
        (chart.metadata.audio_file.as_str(), report.extra_audio_files),
        ("sub/song.ogg", 0)
    );
    for hostile in ["../x.ogg", "/etc/x.ogg", "C:\\x.ogg", "a/../../x.ogg"] {
        let (chart, report) = decode(&format!("m={hostile};ok.ogg\n"), "1000|00|--\n--\n");
        assert!(chart.metadata.audio_file.is_empty(), "{hostile}");
        assert_eq!(report.unsafe_names, 1, "{hostile}");
    }
    let (chart, _) = decode("m=\n", "1000|00|--\n--\n");
    assert!(chart.metadata.audio_file.is_empty());
}

// ---------------------------------------------------------------------------------------------------- notes

/// The 6 columns are BT A..D then FX-L, FX-R; four lines of a 4/4 measure at 120 BPM are 0, 0.5, 1 and 1.5 s.
#[test]
fn every_lane_character() {
    let (chart, report) = decode("", "1000|00|--\n0100|00|--\n0010|20|--\n0001|02|--\n--\n");
    assert_eq!(
        notes_of(&chart),
        [
            (0, 0, 0),
            (500_000, 1, 0),
            (1_000_000, 2, 0),
            (1_000_000, 4, 0),
            (1_500_000, 3, 0),
            (1_500_000, 5, 0),
        ]
    );
    assert!(chart.notes.iter().all(|note| !note.is_hold()));
    assert_eq!(
        (
            report.bt_chips,
            report.fx_chips,
            report.bt_holds,
            report.fx_holds
        ),
        (4, 2, 0, 0)
    );
}

#[test]
fn a_bt_hold_is_the_run_of_2_and_ends_at_the_first_other_line() {
    let (chart, report) = decode("", "0200|00|--\n0200|00|--\n0000|00|--\n0000|00|--\n--\n");
    // Starts at line 0 (0 s), still held at line 1, over at line 2 (1 s).
    assert_eq!(notes_of(&chart), [(0, 1, 1_000_000)]);
    assert_eq!((report.bt_holds, report.bt_chips), (1, 0));
    // A single `2` is a hold one line long, not a chip.
    let (chart, _) = decode("", "0000|00|--\n0002|00|--\n0000|00|--\n0000|00|--\n--\n");
    assert_eq!(notes_of(&chart), [(500_000, 3, 500_000)]);
}

#[test]
fn a_chip_ends_a_bt_hold_and_is_a_note_of_its_own() {
    let (chart, report) = decode("", "2000|00|--\n2000|00|--\n1000|00|--\n0000|00|--\n--\n");
    assert_eq!(notes_of(&chart), [(0, 0, 1_000_000), (1_000_000, 0, 0)]);
    assert_eq!((report.bt_holds, report.bt_chips), (1, 1));
}

#[test]
fn an_fx_hold_is_a_run_of_any_character_but_0_and_2() {
    let (chart, report) = decode("", "0000|10|--\n0000|11|--\n0000|01|--\n0000|00|--\n--\n");
    // FX-L (column 4) from 0 to 1 s (line 2 is the first one without it), FX-R (column 5) from 0.5 to 1.5 s.
    assert_eq!(
        notes_of(&chart),
        [(0, 4, 1_000_000), (500_000, 5, 1_000_000)]
    );
    assert_eq!(
        (
            report.fx_holds,
            report.fx_chips,
            report.fx_legacy_effect_holds
        ),
        (2, 0, 0)
    );
}

#[test]
fn an_fx_chip_ends_an_fx_hold() {
    let (chart, report) = decode("", "0000|11|--\n0000|22|--\n0000|00|--\n0000|00|--\n--\n");
    assert_eq!(
        notes_of(&chart),
        [
            (0, 4, 500_000),
            (0, 5, 500_000),
            (500_000, 4, 0),
            (500_000, 5, 0)
        ]
    );
    assert_eq!((report.fx_holds, report.fx_chips), (2, 2));
}

#[test]
fn legacy_fx_letters_are_holds_and_their_effect_is_dropped() {
    // `S` (Retrigger) then `G` (Gate) in one run: one hold, as the format says (it goes on while the characters are not `0` or `2`).
    let (chart, report) = decode("", "0000|S0|--\n0000|G0|--\n0000|00|--\n0000|00|--\n--\n");
    assert_eq!(notes_of(&chart), [(0, 4, 1_000_000)]);
    assert_eq!((report.fx_holds, report.fx_legacy_effect_holds), (1, 1));
}

#[test]
fn unknown_bt_characters_are_empty_and_counted() {
    let (chart, report) = decode("", "x000|00|--\n1000|00|--\n--\n");
    assert_eq!(notes_of(&chart), [(1_000_000, 0, 0)]);
    assert_eq!(report.unknown_buttons, 1);
}

#[test]
fn a_hold_goes_across_bar_lines() {
    // Measure 1: 4 lines (0, 0.5, 1, 1.5 s), BT-C held from line 2. Measure 2: 2 lines of 1 s (2 and 3 s), held at the first, over at the second:
    // the hold lasts from 1 s to 3 s.
    let (chart, report) = decode(
        "",
        "0000|00|--\n0000|00|--\n0020|00|--\n0020|00|--\n--\n0020|00|--\n0000|00|--\n--\n",
    );
    assert_eq!(notes_of(&chart), [(1_000_000, 2, 2_000_000)]);
    assert_eq!(
        (report.measures, report.chart_lines, report.bt_holds),
        (2, 6, 1)
    );
}

#[test]
fn a_hold_still_open_at_the_end_ends_with_the_last_measure() {
    let (chart, report) = decode("", "0000|00|--\n0000|00|--\n2000|01|--\n2000|01|--\n--\n");
    // Lines at 0, 0.5, 1, 1.5 s; the measure ends at 2 s.
    assert_eq!(
        notes_of(&chart),
        [(1_000_000, 0, 1_000_000), (1_000_000, 5, 1_000_000)]
    );
    assert_eq!(report.holds_open_at_end, 2);
}

// ---------------------------------------------------------------------------------------------------- time

#[test]
fn a_measure_is_cut_in_equal_parts_whatever_the_number_of_lines() {
    // 3 lines of a 2 s measure: 0, 2/3 s, 4/3 s (rounded to µs); then 2 lines of the next one: 2 s and 3 s.
    let (chart, _) = decode(
        "",
        "1000|00|--\n1000|00|--\n1000|00|--\n--\n0100|00|--\n0100|00|--\n--\n",
    );
    assert_eq!(
        notes_of(&chart),
        [
            (0, 0, 0),
            (666_667, 0, 0),
            (1_333_333, 0, 0),
            (2_000_000, 1, 0),
            (3_000_000, 1, 0)
        ]
    );
}

#[test]
fn the_time_signature_sets_the_length_of_a_measure() {
    // 3/4: three beats, 1.5 s, 3 lines of 0.5 s; then 4/4 again: the note of the second measure is at 1.5 s.
    let (chart, report) = decode(
        "",
        "beat=3/4\n1000|00|--\n1000|00|--\n1000|00|--\n--\nbeat=4/4\n0100|00|--\n0100|00|--\n--\n",
    );
    assert_eq!(
        notes_of(&chart),
        [
            (0, 0, 0),
            (500_000, 0, 0),
            (1_000_000, 0, 0),
            (1_500_000, 1, 0),
            (2_500_000, 1, 0)
        ]
    );
    // The model counts quarter notes: 3 then 4.
    assert_eq!(points_of(&chart), [(0, 120.0, 3), (1_500_000, 120.0, 4)]);
    assert_eq!(
        report.time_signature_changes, 2,
        "3/4 at the start, 4/4 back"
    );
}

#[test]
fn the_time_signature_stays_until_changed_and_the_header_one_is_the_first() {
    let (chart, _) = decode("beat=6/8\n", "1000|00|--\n--\n1000|00|--\n--\n");
    // 6/8 is 3 quarter notes: 1.5 s a measure; the second measure starts at 1.5 s.
    assert_eq!(notes_of(&chart), [(0, 0, 0), (1_500_000, 0, 0)]);
    assert_eq!(points_of(&chart), [(0, 120.0, 3)]);
}

#[test]
fn a_beat_after_the_first_line_is_for_the_next_measure() {
    // The `beat=3/4` comes after the first line of the 4/4 measure: that one lasts 2 s (lines at 0 and 1 s), the next one 1.5 s.
    let (chart, _) = decode(
        "",
        "1000|00|--\nbeat=3/4\n1000|00|--\n--\n0100|00|--\n0100|00|--\n--\n0010|00|--\n--\n",
    );
    assert_eq!(
        notes_of(&chart),
        [
            (0, 0, 0),
            (1_000_000, 0, 0),
            (2_000_000, 1, 0),
            (2_750_000, 1, 0),
            (3_500_000, 2, 0)
        ]
    );
    assert_eq!(points_of(&chart), [(0, 120.0, 4), (2_000_000, 120.0, 3)]);
    // A `beat` after the last line of a measure is for the next one too.
    let (chart, _) = decode(
        "",
        "1000|00|--\nbeat=2/4\n--\n1000|00|--\n--\n1000|00|--\n--\n",
    );
    assert_eq!(
        notes_of(&chart),
        [(0, 0, 0), (2_000_000, 0, 0), (3_000_000, 0, 0)]
    );
}

#[test]
fn a_tempo_change_in_the_middle_of_a_measure() {
    // 4 lines of a 4/4 measure at 120 BPM, with 240 BPM from the third line: lines at 0, 0.5, 1 s; the line after is a beat at 240 BPM
    // (0.25 s) later, 1.25 s; the measure ends at 1.5 s. The next measure (4 lines) lasts 1 s at 240 BPM: 0.25 s apart.
    let (chart, report) = decode(
        "",
        "1000|00|--\n1000|00|--\nt=240\n1000|00|--\n1000|00|--\n--\n0100|00|--\n0100|00|--\n0100|00|--\n0100|00|--\n--\n",
    );
    assert_eq!(
        notes_of(&chart),
        [
            (0, 0, 0),
            (500_000, 0, 0),
            (1_000_000, 0, 0),
            (1_250_000, 0, 0),
            (1_500_000, 1, 0),
            (1_750_000, 1, 0),
            (2_000_000, 1, 0),
            (2_250_000, 1, 0),
        ]
    );
    assert_eq!(points_of(&chart), [(0, 120.0, 4), (1_000_000, 240.0, 4)]);
    assert_eq!(report.bpm_changes, 1);
}

#[test]
fn a_tempo_written_after_the_last_line_applies_from_the_next_chart_line() {
    // 2 lines of 1 s each; `t=60` before the bar line; the next measure (1 line) starts at 2 s and its line lasts 4 s at 60 BPM, so the one
    // after starts at 6 s.
    let (chart, _) = decode(
        "",
        "1000|00|--\n1000|00|--\nt=60\n--\n0100|00|--\n--\n0010|00|--\n--\n",
    );
    assert_eq!(
        notes_of(&chart),
        [
            (0, 0, 0),
            (1_000_000, 0, 0),
            (2_000_000, 1, 0),
            (6_000_000, 2, 0)
        ]
    );
    assert_eq!(points_of(&chart), [(0, 120.0, 4), (2_000_000, 60.0, 4)]);
}

#[test]
fn a_tempo_before_the_first_line_replaces_the_header_one() {
    let (chart, _) = decode("", "t=240\n1000|00|--\n1000|00|--\n--\n");
    // 240 BPM: a 4/4 measure is 1 s; 2 lines: 0 and 0.5 s. The first point is the body's tempo.
    assert_eq!(notes_of(&chart), [(0, 0, 0), (500_000, 0, 0)]);
    assert_eq!(points_of(&chart), [(0, 240.0, 4)]);
}

#[test]
fn a_header_tempo_range_is_not_the_tempo() {
    let data = b"title=T\nt=180-220\n--\nt=200\n1000|00|--\n1000|00|--\n--\n";
    let (chart, report) = KshDecoder::decode_with_report(data).unwrap();
    // 200 BPM: a beat is 0.3 s, a measure 1.2 s, 2 lines: 0 and 0.6 s.
    assert_eq!(notes_of(&chart), [(0, 0, 0), (600_000, 0, 0)]);
    assert!(report.header_bpm_range);
    // Without a `t=` in the body, the start of the range stands in: 180 BPM, a beat 1/3 s, 2 lines 2/3 s apart.
    let (chart, report) =
        KshDecoder::decode_with_report(b"title=T\nt=180-220\n--\n1000|00|--\n1000|00|--\n--\n")
            .unwrap();
    assert_eq!(notes_of(&chart), [(0, 0, 0), (666_667, 0, 0)]);
    assert_eq!(points_of(&chart), [(0, 180.0, 4)]);
    assert!(report.header_bpm_range);
}

#[test]
fn a_tempo_that_is_not_a_number_is_ignored_and_counted() {
    for bad in [
        "nan", "NaN", "inf", "-120", "0", "120-180", "abc", "", "1e9",
    ] {
        let (chart, report) = decode("", &format!("t={bad}\n1000|00|--\n1000|00|--\n--\n"));
        assert_eq!(
            notes_of(&chart),
            [(0, 0, 0), (1_000_000, 0, 0)],
            "t={bad}: the header's 120 stays"
        );
        assert_eq!(report.invalid_bpms, 1, "t={bad}");
        assert_eq!(report.bpm_changes, 0, "t={bad}");
    }
    let (chart, report) =
        KshDecoder::decode_with_report(b"title=T\nt=NaN\n--\n1000|00|--\n--\n").unwrap();
    assert_eq!(
        points_of(&chart),
        [(0, 120.0, 4)],
        "no usable header tempo: the format's default 120"
    );
    assert_eq!(report.invalid_bpms, 1);
}

#[test]
fn a_time_signature_that_is_not_n_over_d_is_ignored_and_counted() {
    for bad in [
        "0/4", "4/0", "x/4", "4", "-3/4", "4/4/4", "3/", "/4", "4.5/4",
    ] {
        let (chart, report) = decode("", &format!("beat={bad}\n1000|00|--\n--\n1000|00|--\n--\n"));
        assert_eq!(
            notes_of(&chart),
            [(0, 0, 0), (2_000_000, 0, 0)],
            "beat={bad}: 4/4 stays"
        );
        assert_eq!(report.invalid_time_signatures, 1, "beat={bad}");
    }
}

#[test]
fn stop_moves_no_time() {
    // A stop freezes the scroll, not the song: the second measure still starts at 2 s.
    let (chart, report) = decode("", "1000|00|--\nstop=192\n1000|00|--\n--\n0100|00|--\n--\n");
    assert_eq!(
        notes_of(&chart),
        [(0, 0, 0), (1_000_000, 0, 0), (2_000_000, 1, 0)]
    );
    assert_eq!(report.stops, 1);
}

#[test]
fn the_offset_moves_every_note_and_every_timing_point() {
    let (chart, _) = decode(
        "o=250\n",
        "1000|00|--\n0200|00|--\nt=240\n0200|00|--\n0000|00|--\n--\n",
    );
    // Without the offset: a chip at 0, a hold from 0.5 s; 240 BPM from the third line (1 s), so the fourth line, where the hold ends, is at
    // 1.25 s: the hold lasts 0.75 s.
    assert_eq!(notes_of(&chart), [(250_000, 0, 0), (750_000, 1, 750_000)]);
    assert_eq!(
        points_of(&chart),
        [(250_000, 120.0, 4), (1_250_000, 240.0, 4)]
    );
    assert_eq!(chart.metadata.audio_offset_us, 0);
    // A negative offset puts the first note before the audio starts.
    let (chart, _) = decode("o=-100\n", "1000|00|--\n--\n");
    assert_eq!(notes_of(&chart), [(-100_000, 0, 0)]);
    assert_eq!(points_of(&chart), [(-100_000, 120.0, 4)]);
}

#[test]
fn an_offset_that_is_not_an_integer_is_the_default_and_counted() {
    for bad in ["abc", "12.5", "nan", "99999999999", ""] {
        let (chart, report) = decode(&format!("o={bad}\n"), "1000|00|--\n--\n");
        assert_eq!(notes_of(&chart), [(0, 0, 0)], "o={bad}");
        assert_eq!(report.invalid_option_values, 1, "o={bad}");
    }
}

#[test]
fn an_empty_measure_lasts_its_time_signature() {
    // The first measure has no line: 2 s; the note of the second is at 2 s.
    let (chart, report) = decode("", "--\n1000|00|--\n--\n");
    assert_eq!(notes_of(&chart), [(2_000_000, 0, 0)]);
    assert_eq!(report.empty_measures, 1);
    // The first point is at the start of the first measure, before the first note.
    assert_eq!(points_of(&chart), [(0, 120.0, 4)]);
}

#[test]
fn a_last_measure_with_no_bar_line_is_read_and_counted() {
    let (chart, report) = decode("", "1000|00|--\n--\n0100|00|--\n0100|00|--");
    assert_eq!(
        notes_of(&chart),
        [(0, 0, 0), (2_000_000, 1, 0), (3_000_000, 1, 0)]
    );
    assert!(report.unterminated_last_measure);
    assert_eq!(report.measures, 2);
}

#[test]
fn a_hold_that_rounds_to_no_time_is_a_tap() {
    // 1e7 BPM: a beat is 6 µs; 64 lines share 24 µs: 0.375 µs apart, so lines 0 and 1 are both at 0 µs.
    let mut body = String::from("t=10000000\n2000|00|--\n0000|00|--\n");
    for _ in 2..64 {
        body.push_str("0000|00|--\n");
    }
    body.push_str("--\n");
    let (chart, report) = decode("", &body);
    assert_eq!(notes_of(&chart), [(0, 0, 0)]);
    assert_eq!(report.holds_as_taps, 1);
}

#[test]
fn a_chart_that_lasts_too_long_is_refused() {
    let message = error("", "t=0.000000001\n1000|00|--\n--\n");
    assert!(message.contains("lasts more than"), "{message}");
}

// ---------------------------------------------------------------------------------------------------- dropped

#[test]
fn lasers_are_counted_and_make_no_note() {
    // Left laser: `0` then `:` then `o` (one note, over 3 lines, a slam: no, 1/4 of a measure apart) then none; right laser: `o` and `-`.
    let (chart, report) = decode("", "1000|00|0o\n0000|00|:-\n0000|00|o-\n0000|00|--\n--\n");
    assert_eq!(notes_of(&chart), [(0, 0, 0)]);
    assert_eq!(report.laser_notes, 2, "one note per lane");
    assert_eq!(
        report.laser_slams, 0,
        "2 lines of a quarter note apart are no slam"
    );
}

#[test]
fn a_laser_slam_is_two_positions_a_32nd_of_a_whole_note_apart_or_less() {
    // 32 lines of a 4/4 measure: a line is 1/32 of a whole note. `0` then `o` on the next line: a slam (left lane); `-` ends it.
    let mut body = String::from("1000|00|0-\n0000|00|o-\n");
    for _ in 2..32 {
        body.push_str("0000|00|--\n");
    }
    body.push_str("--\n");
    let (chart, report) = decode("", &body);
    assert_eq!(notes_of(&chart), [(0, 0, 0)]);
    assert_eq!((report.laser_notes, report.laser_slams), (1, 1));
    // 16 lines: 1/16 apart: no slam. With `:` between two positions 1/32 apart each (64 lines), the span is 3/64: no slam.
    let mut body = String::from("1000|00|0-\n0000|00|o-\n");
    for _ in 2..16 {
        body.push_str("0000|00|--\n");
    }
    body.push_str("--\n");
    let (_, report) = decode("", &body);
    assert_eq!((report.laser_notes, report.laser_slams), (1, 0));
    let mut body = String::from("1000|00|0-\n0000|00|:-\n0000|00|:-\n0000|00|o-\n");
    for _ in 4..64 {
        body.push_str("0000|00|--\n");
    }
    body.push_str("--\n");
    let (_, report) = decode("", &body);
    assert_eq!((report.laser_notes, report.laser_slams), (1, 0));
    // A 3-line span at 64 lines is 1/64 each: 2 lines apart = 1/32: a slam.
    let mut body = String::from("1000|00|0-\n0000|00|:-\n0000|00|o-\n");
    for _ in 3..64 {
        body.push_str("0000|00|--\n");
    }
    body.push_str("--\n");
    let (_, report) = decode("", &body);
    assert_eq!(report.laser_slams, 1);
}

#[test]
fn lane_spins_half_spins_and_swings_are_counted() {
    let (chart, report) = decode(
        "",
        "1000|00|0-@(192\n0000|00|--@)96\n0000|00|--@<192\n0000|00|--@>192\n--\n0000|00|-0S<192;250;3;2\n0000|00|--S>192\n0000|00|--zz9\n0000|00|--\n--\n",
    );
    assert_eq!(notes_of(&chart), [(0, 0, 0)]);
    assert_eq!(
        (report.lane_spins, report.half_spins, report.swings),
        (2, 2, 2)
    );
    assert_eq!(report.unknown_spin_suffixes, 1);
}

#[test]
fn an_invalid_laser_character_ends_the_laser_and_is_counted() {
    let (_, report) = decode("", "1000|00|0?\n0000|00|?-\n0000|00|:-\n0000|00|--\n--\n");
    assert_eq!(
        report.invalid_laser_chars, 3,
        "`?` twice and a `:` with no laser before it"
    );
    assert_eq!(report.laser_notes, 1);
}

#[test]
fn fx_effects_and_other_options_are_counted() {
    let (chart, report) = decode(
        "ver=167\nmvol=100\nlayer=arrow\n",
        "fx-l=Retrigger;8\nfx-r_param1=4\nfx-l_se=clap;100\n0000|11|--\n0000|00|--\n0000|00|--\n0000|00|--\n--\nstop=96\nscroll_speed=2\ntilt=zoom\nzoom_top=5\ncenter_split=1\nrotation_deg=4\ntilt_curve=0;1\nlaserrange_l=2x\nlaser_r_curve=0;1\nchokkakuvol=40\nchokkakuse=up\npfiltergain=10\nfiltertype=peak\nchiprate=2x\nfx:dla2:mix=0.5\nfilter:RE12:mix=1\nwhatever=1\n1000|00|--\n--\n",
    );
    assert_eq!(
        notes_of(&chart),
        [(0, 4, 500_000), (0, 5, 500_000), (2_000_000, 0, 0)]
    );
    assert_eq!(
        report.fx_options, 4,
        "fx-l, fx-r_param1, fx-l_se, fx:dla2:mix"
    );
    assert_eq!(report.stops, 1);
    assert_eq!(
        report.scroll_options, 6,
        "scroll_speed, tilt, zoom_top, center_split, rotation_deg, tilt_curve"
    );
    assert_eq!(report.laser_options, 2);
    assert_eq!(
        report.audio_options, 6,
        "chokkakuvol, chokkakuse, pfiltergain, filtertype, chiprate, filter:RE12:mix"
    );
    assert_eq!(report.unknown_options, 1);
}

#[test]
fn definitions_comments_and_unknown_lines_are_counted() {
    let (chart, report) = decode(
        "// a comment\nfoo=bar\n",
        "// note\n;unofficial\n1000|00|--\n--\nnot a line\n#define_fx LoFl type=Flanger;delay=80samples\n#define_filter TSTP type=TapeStop\n#other\n",
    );
    assert_eq!(notes_of(&chart), [(0, 0, 0)]);
    assert_eq!(report.comments, 3);
    assert_eq!(report.definitions, 2);
    assert_eq!(report.unknown_lines, 2, "`not a line` and `#other`");
    assert_eq!(report.unknown_options, 1, "`foo` in the header");
}

// ---------------------------------------------------------------------------------------------------- text

#[test]
fn the_format_is_read_with_crlf_lf_and_cr_and_a_mix() {
    let text =
        "title=T\r\nt=120\r\n--\r\n1000|00|--\r\n0100|00|--\n0010|00|--\r0001|00|--\r\n--\r\n";
    let (chart, _) = KshDecoder::decode_with_report(text.as_bytes()).unwrap();
    assert_eq!(
        notes_of(&chart),
        [
            (0, 0, 0),
            (500_000, 1, 0),
            (1_000_000, 2, 0),
            (1_500_000, 3, 0)
        ]
    );
    let only_cr = text.replace("\r\n", "\r").replace('\n', "\r");
    assert_eq!(
        KshDecoder::decode(only_cr.as_bytes()).unwrap().notes.len(),
        4
    );
    let only_lf = text.replace("\r\n", "\n").replace('\r', "\n");
    assert_eq!(
        KshDecoder::decode(only_lf.as_bytes()).unwrap().notes.len(),
        4
    );
    // Spaces and tabs at the ends of a line are not part of it, and a blank line is nothing.
    let spaced = "title=T\n\n  t=120 \n--\n\t1000|00|--  \n\n--\n";
    assert_eq!(
        KshDecoder::decode(spaced.as_bytes()).unwrap().notes.len(),
        1
    );
}

#[test]
fn a_byte_order_mark_and_the_charsets() {
    let mut bom = b"\xEF\xBB\xBF".to_vec();
    bom.extend(file("artist=\u{3042}\n", "1000|00|--\n--\n"));
    let (chart, report) = KshDecoder::decode_with_report(&bom).unwrap();
    assert_eq!(
        (chart.metadata.artist.as_str(), report.charset),
        ("\u{3042}", "UTF-8")
    );
    // No byte order mark and valid UTF-8: UTF-8.
    let (chart, report) =
        KshDecoder::decode_with_report(&file("artist=\u{3042}\n", "1000|00|--\n--\n")).unwrap();
    assert_eq!(
        (chart.metadata.artist.as_str(), report.charset),
        ("\u{3042}", "UTF-8")
    );
    // Shift_JIS: "あ" is 0x82 0xA0.
    let mut sjis = b"title=T\nt=120\nartist=".to_vec();
    sjis.extend([0x82, 0xA0]);
    sjis.extend(b"\n--\n1000|00|--\n--\n");
    let (chart, report) = KshDecoder::decode_with_report(&sjis).unwrap();
    assert_eq!(
        (chart.metadata.artist.as_str(), report.charset),
        ("\u{3042}", "Shift_JIS")
    );
    // GBK is reached by a sequence Shift_JIS refuses (0x30 is no trail byte of it): GB18030's 4-byte form 0x81 0x30 0x81 0x30.
    let mut gbk = b"title=T\nt=120\nartist=".to_vec();
    gbk.extend([0x81, 0x30, 0x81, 0x30]);
    gbk.extend(b"\n--\n1000|00|--\n--\n");
    let (_, report) = KshDecoder::decode_with_report(&gbk).unwrap();
    assert!(
        ["GBK", "Shift_JIS (lossy)"].contains(&report.charset),
        "{}",
        report.charset
    );
}

// ---------------------------------------------------------------------------------------------------- hostile

#[test]
fn a_file_with_no_bar_line_is_refused() {
    for data in [
        &b""[..],
        b"title=T\nt=120\n",
        b"title=T\n1000|00|--\n",
        b"#!/bin/ksh\necho hi\n",
    ] {
        let message = KshDecoder::decode_with_report(data)
            .unwrap_err()
            .to_string();
        assert!(message.contains("no bar line"), "{message}");
    }
}

#[test]
fn a_file_with_no_note_is_refused() {
    let message = error("", "0000|00|--\n--\n");
    assert!(message.contains("no playable note"), "{message}");
    let message = error("", "--\n");
    assert!(message.contains("no playable note"), "{message}");
    // A shell script with a `--` line decodes to nothing.
    let message = KshDecoder::decode_with_report(b"echo a\n--\necho b\n")
        .unwrap_err()
        .to_string();
    assert!(message.contains("no playable note"), "{message}");
}

#[test]
fn a_malformed_chart_line_is_an_error_not_a_dropped_line() {
    for line in [
        "1000|0|--",
        "1000|000|--",
        "100|00|--",
        "1000|00|-",
        "1000|00-",
        "a|b",
        "|",
    ] {
        let message = error("", &format!("{line}\n--\n"));
        assert!(message.contains("is not a chart line"), "{line}: {message}");
    }
}

#[test]
fn hostile_sizes_and_bytes_do_not_panic() {
    // A line of a megabyte with no structure, then a real chart.
    let long = "x".repeat(1 << 20);
    let (chart, report) =
        KshDecoder::decode_with_report(format!("title=T\n--\n{long}\n1000|00|--\n--\n").as_bytes())
            .unwrap();
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(report.unknown_lines, 1);
    // A huge option value, a NUL, a lone `=`, and invalid UTF-8 everywhere.
    let mut data = b"title=T\n=\n\0\xFF\xFE=\n".to_vec();
    data.extend(format!("artist={long}\n").bytes());
    data.extend(b"--\n1000|00|--\n=\n--\n");
    assert!(KshDecoder::decode_with_report(&data).is_ok());
    for data in [
        &b"--"[..],
        b"--\n--\n--\n",
        b"\xFF\xFE\xFD",
        b"title=\n--\n\0\0\0",
        b"=\n--\n=\n",
    ] {
        let _ = KshDecoder::decode_with_report(data);
    }
}

#[test]
fn a_file_above_the_size_limit_is_refused_before_it_is_read() {
    let data = vec![b'a'; super::decoder::MAX_FILE_SIZE + 1];
    let message = KshDecoder::decode_with_report(&data)
        .unwrap_err()
        .to_string();
    assert!(message.contains("too large"), "{message}");
}

#[test]
fn too_many_chart_lines_are_refused() {
    let mut body = String::new();
    for _ in 0..=500_000 {
        body.push_str("1000|00|--\n");
    }
    body.push_str("--\n");
    let message = error("", &body);
    assert!(message.contains("too many chart lines"), "{message}");
}

#[test]
fn a_footer_after_the_last_bar_line_is_not_read() {
    // Footer lines after the last bar line are definitions; a stray option there has no chart line to apply to and is only counted.
    let (chart, report) = decode("", "1000|00|--\n--\n#define_fx A type=Flanger\nt=200\n");
    assert_eq!(notes_of(&chart), [(0, 0, 0)]);
    assert_eq!(report.definitions, 1);
}

// ---------------------------------------------------------------------------------------------------- sniff

#[test]
fn a_chart_is_told_from_a_korn_shell_script() {
    let chart = file("artist=A\n", "1000|00|--\n--\n");
    assert!(looks_like_ksh(&chart));
    let mut bom = b"\xEF\xBB\xBF".to_vec();
    bom.extend(&chart);
    assert!(looks_like_ksh(&bom));
    assert!(looks_like_ksh(b"title=\r\n//comment\r\nt=120\r\n--\r\n"));
    assert!(looks_like_ksh(b"\n\ntitle=x\nfx-l=a\n--"));
    for script in [
        &b"#!/bin/ksh\n# title=x\ntitle=x\n--\n"[..],
        b"#!/bin/ksh\necho hello\n",
        b"title=x\necho hello\n--\n",
        b"title=x\nif [ -f a ]; then\n--\n",
        b"echo title=x\n--\n",
        b"typeset title=x\n--\n",
        b"title=x\nname=1\n",
        b"title=x\n",
        b"",
        b"--\n",
        b"// c\ntitle=x\n--\n",
        b"title=x\n\0\n--\n",
        b"Title=x\n--\n",
        b"title=x\n=x\n--\n",
        b"title=x\nbad name=1\n--\n",
    ] {
        assert!(
            !looks_like_ksh(script),
            "{}",
            String::from_utf8_lossy(script)
        );
    }
}

#[test]
fn a_cut_head_passes_when_what_it_holds_looks_like_a_header() {
    let mut head = b"title=x\n".to_vec();
    while head.len() < crate::formats::SNIFF_BYTES {
        head.extend(
            b"information=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
        );
    }
    head.truncate(crate::formats::SNIFF_BYTES);
    assert!(looks_like_ksh(&head), "cut in the middle of a header line");
    // A script of the same length does not.
    let mut script = b"title=x\n".to_vec();
    script.extend(b"echo hello\n");
    while script.len() < crate::formats::SNIFF_BYTES {
        script.extend(
            b"information=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
        );
    }
    script.truncate(crate::formats::SNIFF_BYTES);
    assert!(!looks_like_ksh(&script));
}

#[test]
fn the_sniff_by_path_looks_at_the_extension_and_the_content() {
    let dir = std::env::temp_dir().join(format!("rox_ksh_sniff_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let write = |name: &str, data: &[u8]| {
        let path = dir.join(name);
        std::fs::write(&path, data).unwrap();
        path
    };
    let chart = file("", "1000|00|--\n--\n");
    assert!(is_ksh_chart_file(&write("exh.ksh", &chart)));
    assert!(is_ksh_chart_file(&write("INF.KSH", &chart)));
    assert!(!is_ksh_chart_file(&write("exh.txt", &chart)));
    assert!(!is_ksh_chart_file(&write(
        "script.ksh",
        b"#!/bin/ksh\nprint hello\n"
    )));
    assert!(!is_ksh_chart_file(&write("empty.ksh", b"")));
    assert!(!is_ksh_chart_file(&dir.join("missing.ksh")));
    std::fs::remove_dir_all(&dir).unwrap();
}

// ---------------------------------------------------------------------------------------------------- routing

#[test]
fn auto_decode_routes_a_ksh_by_its_path() {
    use crate::formats::auto::{
        auto_decode, auto_decode_all, auto_decode_bytes, auto_decode_variant,
    };
    let dir = std::env::temp_dir().join(format!("rox_ksh_auto_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("exh.ksh");
    let titled = dir.join("titled.ksh");
    let data = b"t=120\n--\n1000|00|--\n--\n".to_vec();
    std::fs::write(&path, &data).unwrap();
    std::fs::write(&titled, file("", "1000|00|--\n--\n")).unwrap();

    let chart = auto_decode(&path).unwrap();
    assert_eq!(chart.key_count, 6);
    assert_eq!(
        chart.metadata.title.as_str(),
        "exh",
        "the file stem stands in for an absent title"
    );
    assert_eq!(auto_decode(&titled).unwrap().metadata.title.as_str(), "T");
    assert_eq!(auto_decode_variant(&path, 0).unwrap().notes.len(), 1);
    assert!(
        auto_decode_variant(&path, 1)
            .unwrap_err()
            .to_string()
            .contains("single chart")
    );
    assert!(
        auto_decode_all(&path).unwrap().is_none(),
        "one chart per file"
    );
    assert_eq!(
        auto_decode_bytes(&path, &data)
            .unwrap()
            .metadata
            .title
            .as_str(),
        "exh"
    );
    assert!(
        crate::formats::auto::auto_encode(&chart, dir.join("out.ksh")).is_err(),
        "decode only"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
