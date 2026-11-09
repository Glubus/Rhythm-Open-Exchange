//! Tests of the `.sus` and `.usc` decoders. Every file here is written by hand in the test (the notes' own examples are used where they
//! give one); no real chart is used: community charts carry no licence to redistribute them. Times are computed by hand in the
//! comments: at 120 BPM a beat is 0.5 s and a 4/4 measure 2 s.

use rox::codec::Decoder;
use rox::model::{RoxChart, TimingPoint};
use serde_json::{Value, json};

use super::{COLUMNS, SekaiReport, SusDecoder, UscDecoder, looks_like_sus, looks_like_usc};

// ---------------------------------------------------------------------------------------------------- builders

/// 480 ticks per beat, 120 BPM from the first slot of the first measure.
const HEAD: &str = "#REQUEST \"ticks_per_beat 480\"\n#BPM01: 120\n#00008: 01\n";

fn sus_bytes(body: &str) -> Vec<u8> {
    format!("{HEAD}{body}").into_bytes()
}

fn sus(body: &str) -> (RoxChart, SekaiReport) {
    SusDecoder::decode_with_report(&sus_bytes(body)).unwrap()
}

fn sus_error(body: &str) -> String {
    SusDecoder::decode_with_report(&sus_bytes(body))
        .unwrap_err()
        .to_string()
}

/// `(time_us, column, duration_us)`, a tap's duration is 0.
fn notes_of(chart: &RoxChart) -> Vec<(i64, u8, i64)> {
    chart
        .notes
        .iter()
        .map(|n| (n.time_us, n.column, n.duration_us()))
        .collect()
}

fn usc_bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

fn usc(objects: &Value) -> (RoxChart, SekaiReport) {
    UscDecoder::decode_with_report(&usc_bytes(&json!({"offset": 0, "objects": objects}))).unwrap()
}

/// The USC numbers of a note of `width` lanes whose leftmost SUS lane is `x` (`susToUSC`: `lane = x - 8 + width / 2`).
fn lane_of(x: i32, width: i32) -> (f64, f64) {
    (
        f64::from(x) - 8.0 + f64::from(width) / 2.0,
        f64::from(width) / 2.0,
    )
}

fn usc_single(beat: f64, x: i32, width: i32) -> Value {
    let (lane, size) = lane_of(x, width);
    json!({"type": "single", "beat": beat, "lane": lane, "size": size, "trace": false, "critical": false})
}

fn bpm_object(beat: f64, bpm: f64) -> Value {
    json!({"type": "bpm", "beat": beat, "bpm": bpm})
}

// ---------------------------------------------------------------------------------------------------- SUS: lanes

#[test]
fn a_sus_tap_is_a_note_on_its_column_of_twelve() {
    // Lanes x = 2..=13 are the columns 0..=11. Measure 0: x=2 at beat 0, x=6 at slot 1 of 2 (beat 2 = 1 s), x=13 at beat 0;
    // measure 1 starts at beat 4 (2 s).
    let (chart, report) = sus("#00012: 11\n#00016: 0011\n#0001d: 1100\n#00112: 11\n");
    assert_eq!(chart.key_count, COLUMNS);
    assert_eq!(chart.key_count, 12);
    assert_eq!(
        notes_of(&chart),
        [(0, 0, 0), (0, 11, 0), (1_000_000, 4, 0), (2_000_000, 0, 0)]
    );
    assert_eq!(chart.timing_points.len(), 1);
    assert_eq!(chart.timing_points[0].time_us(), 0);
    assert_eq!(chart.timing_points[0].bpm_value(), Some(120.0));
    assert_eq!(report.ticks_per_beat, 480);
    assert_eq!(
        report,
        SekaiReport {
            ticks_per_beat: 480,
            bpm_changes: 1,
            ..SekaiReport::default()
        }
    );
}

#[test]
fn notes_off_the_stage_are_dropped_and_fever_markers_are_not_taps() {
    // x=1 and x=14 are outside the stage, x=15 ('f') with width 1 is a fever marker, x=12 width 2 ends on lane 13 (ok),
    // x=13 width 2 would end on lane 14 (outside), x=2 is fine.
    let (chart, report) =
        sus("#00011: 11\n#0001e: 11\n#0001f: 11\n#0001c: 12\n#0001d: 12\n#00012: 11\n");
    assert_eq!(notes_of(&chart), [(0, 0, 0), (0, 10, 0)]);
    assert_eq!(report.notes_outside_lanes, 3);
    assert_eq!(report.fever_markers, 1);
}

#[test]
fn a_wide_note_is_one_note_on_its_leftmost_column() {
    // Width 4 from x=2 and width 12 from x=2 (the whole stage) are one note each, on column 0; width 3 from x=6 is on column 4.
    let (chart, _) = sus("#00012: 14\n#00016: 0013\n");
    assert_eq!(notes_of(&chart), [(0, 0, 0), (1_000_000, 4, 0)]);
    let (whole, _) = sus("#00012: 1c\n");
    assert_eq!(notes_of(&whole), [(0, 0, 0)]);
    let (too_wide, report) = sus("#00012: 11\n#00013: 1c\n");
    assert_eq!(notes_of(&too_wide), [(0, 0, 0)]);
    assert_eq!(report.notes_outside_lanes, 1);
}

// ---------------------------------------------------------------------------------------------------- SUS: time

#[test]
fn bpm_changes_between_and_inside_measures_move_the_times() {
    // 120 BPM to beat 6 (measure 1, slot 1 of 2), then 240 BPM (0.25 s a beat). Beat 4 = 2.0 s, beat 6 = 3.0 s, beat 8 = 3.5 s.
    let (chart, report) = sus("#BPM02: 240\n#00108: 0002\n#00012: 11\n#00112: 11\n#00212: 11\n");
    assert_eq!(
        notes_of(&chart),
        [(0, 0, 0), (2_000_000, 0, 0), (3_500_000, 0, 0)]
    );
    let points: Vec<(i64, Option<f32>)> = chart
        .timing_points
        .iter()
        .map(|p| (p.time_us(), p.bpm_value()))
        .collect();
    assert_eq!(points, [(0, Some(120.0)), (3_000_000, Some(240.0))]);
    assert_eq!(report.bpm_changes, 2);
}

#[test]
fn the_first_bpm_goes_on_backwards_when_the_first_change_is_late() {
    // The first change is at beat 4. Before it the format gives no tempo: its BPM extends back, so beat 0 is 0 s and beat 4 is 2 s.
    let body = "#00012: 11\n#00112: 11\n";
    let data = format!("#REQUEST \"ticks_per_beat 480\"\n#BPM01: 120\n#00108: 01\n{body}");
    let (chart, _) = SusDecoder::decode_with_report(data.as_bytes()).unwrap();
    assert_eq!(notes_of(&chart), [(0, 0, 0), (2_000_000, 0, 0)]);
    // The first timing point sits no later than the first note, as the model requires.
    assert_eq!(chart.timing_points[0].time_us(), 0);
}

#[test]
fn a_measure_length_holds_until_the_next_one() {
    // Measure 1 has 3 beats, measure 3 has 2.5. Starts: m1 beat 4, m2 beat 7, m3 beat 10, m4 beat 12.5.
    // Measure 1, slot 1 of 3: beat 5 = 2.5 s. Measure 2: beat 7 = 3.5 s. Measure 4: beat 12.5 = 6.25 s.
    let (chart, _) =
        sus("#00102: 3\n#00302: 2.5\n#00012: 11\n#00112: 001100\n#00212: 11\n#00412: 11\n");
    assert_eq!(
        notes_of(&chart),
        [
            (0, 0, 0),
            (2_500_000, 0, 0),
            (3_500_000, 0, 0),
            (6_250_000, 0, 0)
        ]
    );
    // The measure length is the signature of a BPM change in it.
    let (signed, _) = sus("#00102: 3\n#00108: 01\n#00112: 11\n");
    assert_eq!(
        signed
            .timing_points
            .iter()
            .map(|p| match p {
                TimingPoint::Bpm { signature, .. } => *signature,
                TimingPoint::Sv { .. } => 0,
            })
            .collect::<Vec<_>>(),
        [4, 3]
    );
}

#[test]
fn an_invalid_measure_length_is_ignored_and_counted() {
    let (chart, report) = sus("#00102: 0\n#00202: abc\n#00302: 5000\n#00112: 11\n");
    assert_eq!(notes_of(&chart), [(2_000_000, 0, 0)]);
    assert_eq!(report.invalid_measure_lengths, 3);
}

#[test]
fn ticks_per_beat_do_not_change_the_times() {
    let body = "#00012: 11\n#00016: 0011\n#00112: 11\n";
    let with = |request: &str| {
        let data = format!("{request}#BPM01: 120\n#00008: 01\n{body}");
        SusDecoder::decode_with_report(data.as_bytes()).unwrap()
    };
    let (base, report_a) = with("#REQUEST \"ticks_per_beat 480\"\n");
    let (double, report_b) = with("#REQUEST \"ticks_per_beat 960\"\n");
    let (absent, report_c) = with("");
    let (zero, report_d) = with("#REQUEST \"ticks_per_beat 0\"\n");
    let (other_request, report_e) = with("#REQUEST \"enable_priority true\"\n");
    for other in [&double, &absent, &zero, &other_request] {
        assert_eq!(notes_of(&base), notes_of(other));
    }
    assert_eq!(
        notes_of(&base),
        [(0, 0, 0), (1_000_000, 4, 0), (2_000_000, 0, 0)]
    );
    assert_eq!(report_a.ticks_per_beat, 480);
    assert_eq!(report_b.ticks_per_beat, 960);
    assert_eq!(report_c.ticks_per_beat, 480);
    assert_eq!(
        (report_d.ticks_per_beat, report_d.invalid_directives),
        (480, 1)
    );
    assert_eq!(
        (report_e.ticks_per_beat, report_e.ignored_directives),
        (480, 1)
    );
}

#[test]
fn objects_on_one_tick_meet_whatever_their_slot_division() {
    // A tap on 3 slots (slot 1 = beat 4/3) and a directional on 6 slots (slot 2 = beat 4/3) are on the same tick: a flick.
    let (chart, report) = sus("#00012: 001100\n#00052: 000011000000\n");
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(report.flicks, 1);
    // 4/3 beat = 0.6666667 s, rounded to the microsecond.
    assert_eq!(chart.notes[0].time_us, 666_667);
    // A directional one tick away is not on it.
    let (apart, apart_report) = sus("#00012: 11\n#00052: 0011\n");
    assert_eq!(apart.notes.len(), 1);
    assert_eq!((apart_report.flicks, apart_report.stray_overlays), (0, 1));
}

#[test]
fn the_wave_offset_shifts_every_time_by_its_opposite() {
    // The notes: a positive #WAVEOFFSET makes the chart start first, the audio comes that much later: the audio position of
    // a note is its chart time minus the offset.
    let (chart, report) = sus("#WAVEOFFSET 0.5\n#00012: 11\n#00112: 11\n");
    assert_eq!(notes_of(&chart), [(-500_000, 0, 0), (1_500_000, 0, 0)]);
    assert_eq!(chart.timing_points[0].time_us(), -500_000);
    assert_eq!(report.shift_seconds, -0.5);
    // A negative offset: the audio comes first, the notes are later in it.
    let (late, late_report) = sus("#WAVEOFFSET -0.25\n#00012: 11\n");
    assert_eq!(notes_of(&late), [(250_000, 0, 0)]);
    assert_eq!(late_report.shift_seconds, 0.25);
    assert_eq!(chart.metadata.audio_offset_us, 0);
    // An offset that is not a number is ignored and counted.
    let (bad, bad_report) = sus("#WAVEOFFSET abc\n#00012: 11\n");
    assert_eq!(notes_of(&bad), [(0, 0, 0)]);
    assert_eq!(bad_report.invalid_directives, 1);
}

#[test]
fn measure_bases_add_to_the_measure_numbers_that_follow() {
    // `#MEASUREBS 1000`: the lines after it are measures 1000.. (beat 4000 = 2000 s).
    let (chart, _) = sus("#00012: 11\n#MEASUREBS 1000\n#00012: 11\n");
    assert_eq!(notes_of(&chart), [(0, 0, 0), (2_000_000_000, 0, 0)]);
    let (bad, report) = sus("#MEASUREBS -3\n#MEASUREBS x\n#00012: 11\n");
    assert_eq!(notes_of(&bad), [(0, 0, 0)]);
    assert_eq!(report.invalid_directives, 2);
}

#[test]
fn bpm_definitions_are_validated_and_ids_are_case_sensitive() {
    // BPM01 is 0 and BPM02 is not a number (both refused), BPM0a is 100 and BPM0A is 200: `0A` names the second.
    let data = "#BPM01: 0\n#BPM02: abc\n#BPM0a: 100\n#BPM0A: 200\n#00008: 0A\n#00108: 01\n#00012: 11\n#00112: 11\n";
    let (chart, report) = SusDecoder::decode_with_report(data.as_bytes()).unwrap();
    // 200 BPM: 0.3 s a beat, beat 4 = 1.2 s. `01` is no BPM: ignored.
    assert_eq!(notes_of(&chart), [(0, 0, 0), (1_200_000, 0, 0)]);
    assert_eq!(report.invalid_bpm, 2);
    assert_eq!(report.unknown_bpm_ids, 1);
    assert_eq!(chart.timing_points.len(), 1);
}

#[test]
fn a_chart_with_no_valid_bpm_is_an_error() {
    let error = SusDecoder::decode_with_report(b"#00012: 11\n").unwrap_err();
    assert!(error.to_string().contains("no valid BPM"), "{error}");
    let error =
        SusDecoder::decode_with_report(b"#BPM01: -5\n#00008: 01\n#00012: 11\n").unwrap_err();
    assert!(error.to_string().contains("no valid BPM"), "{error}");
}

#[test]
fn a_time_beyond_ten_million_seconds_is_dropped() {
    // 0.01 BPM is 6000 s a beat: measure 999 starts at beat 3996 = 23 976 000 s.
    let data = "#BPM01: 0.01\n#00008: 01\n#00012: 11\n#99912: 11\n";
    let (chart, report) = SusDecoder::decode_with_report(data.as_bytes()).unwrap();
    assert_eq!(notes_of(&chart), [(0, 0, 0)]);
    assert_eq!(report.notes_out_of_range, 1);
}

// ---------------------------------------------------------------------------------------------------- SUS: notes

#[test]
fn flicks_critical_taps_traces_and_skills_follow_the_notes() {
    let (chart, report) = sus(concat!(
        "#00012: 11\n#00052: 11\n", // a tap with a directional on it: a flick, still a tap
        "#00013: 21\n",             // critical tap
        "#00014: 51\n#00015: 61\n", // trace and critical trace: dropped
        "#00010: 41\n",             // skill activation on lane 0
        "#00016: 71\n#00017: 31\n", // an elaser and an attach flick with nothing under them
        "#00058: 11\n",             // a directional with no note
        "#00018: z1\n",             // a kind nobody knows
    ));
    assert_eq!(notes_of(&chart), [(0, 0, 0), (0, 1, 0)]);
    let expected = SekaiReport {
        ticks_per_beat: 480,
        bpm_changes: 1,
        flicks: 1,
        critical_flags: 1,
        traces: 2,
        skill_markers: 1,
        stray_overlays: 3,
        unsupported_notes: 1,
        ..SekaiReport::default()
    };
    assert_eq!(report, expected);
}

#[test]
fn a_file_of_dropped_kinds_has_no_playable_note() {
    let error = sus_error("#00012: 51\n#00013: 61\n#00010: 41\n");
    assert!(error.contains("no playable note"), "{error}");
}

#[test]
fn a_slide_is_one_hold_from_its_start_to_its_end() {
    // Channel a: start x=2 at beat 0, end x=3 at slot 4 of 8 (beat 2 = 1 s). The path across lanes is not kept.
    let (chart, report) = sus("#00032a: 1100000000000000\n#00033a: 0000000021000000\n");
    assert_eq!(notes_of(&chart), [(0, 0, 1_000_000)]);
    assert!(chart.notes[0].is_hold());
    assert_eq!(report.slide_joints_dropped, 0);
}

#[test]
fn the_joints_of_a_slide_are_dropped_and_counted() {
    // Start x=2 (beat 0), visible relay x=5 (slot 1 = beat 0.5), invisible relay x=4 (slot 2 = beat 1), Bezier control x=6 (kind 4,
    // slot 3), end x=3 (beat 2).
    let (chart, report) = sus(concat!(
        "#00032a: 1100000000000000\n",
        "#00035a: 0031000000000000\n",
        "#00034a: 0000510000000000\n",
        "#00036a: 0000004100000000\n",
        "#00033a: 0000000021000000\n",
    ));
    assert_eq!(notes_of(&chart), [(0, 0, 1_000_000)]);
    assert_eq!(report.slide_joints_dropped, 3);
}

#[test]
fn slides_are_paired_by_channel_and_cut_after_each_end() {
    // Channel a holds two slides one after the other (beats 0..2 and 4..6, on x=2); channel b crosses the first one (x=6, beats 1..3).
    let (chart, report) = sus(concat!(
        "#00032a: 1100000021000000\n",
        "#00132a: 1100000021000000\n",
        "#00036b: 0000110000002100\n",
    ));
    assert_eq!(
        notes_of(&chart),
        [
            (0, 0, 1_000_000),         // a, first
            (500_000, 4, 1_000_000),   // b: beat 1 = 0.5 s to beat 3 = 1.5 s
            (2_000_000, 0, 1_000_000), // a, second: beat 4 = 2 s to beat 6 = 3 s
        ]
    );
    assert_eq!(report.unterminated_slides, 0);
}

#[test]
fn a_channel_is_case_insensitive() {
    let (chart, _) = sus("#00032A: 1100000000000000\n#00033a: 0000000021000000\n");
    assert_eq!(notes_of(&chart), [(0, 0, 1_000_000)]);
}

#[test]
fn overlaid_notes_on_a_slide_are_modifiers_not_notes() {
    // A critical tap, a directional and an elaser on the start and end of a slide change how it is judged: they are not notes.
    let (chart, report) = sus(concat!(
        "#00032a: 1100000000000000\n",
        "#00033a: 0000000021000000\n",
        "#00012: 21\n",
        "#00052: 11\n",
        "#00013: 0071\n",
    ));
    assert_eq!(notes_of(&chart), [(0, 0, 1_000_000)]);
    assert_eq!(report.slide_modifiers, 3);
    assert_eq!(
        (report.stray_overlays, report.critical_flags, report.flicks),
        (0, 0, 0)
    );
}

#[test]
fn a_slide_hidden_by_an_elaser_is_still_a_hold() {
    // The elasers (7 and 8) on its start and end take their judgement away in the game; the model can not say so, the hold stays.
    let (chart, report) = sus(concat!(
        "#00032a: 1100000000000000\n",
        "#00033a: 0000000021000000\n",
        "#00012: 71\n",
        "#00013: 0081\n",
    ));
    assert_eq!(notes_of(&chart), [(0, 0, 1_000_000)]);
    assert_eq!(report.slide_modifiers, 2);
}

#[test]
fn a_slide_without_an_end_or_a_start_is_dropped() {
    let (chart, report) = sus(concat!(
        "#00016: 11\n",
        "#00032a: 1100\n", // start, never ended
        "#00033c: 0021\n", // an end with no start
    ));
    assert_eq!(notes_of(&chart), [(0, 4, 0)]);
    assert_eq!(report.unterminated_slides, 1);
    assert_eq!(report.slide_points_without_start, 1);
}

#[test]
fn a_slide_that_ends_when_it_starts_is_a_tap() {
    // Start x=2 and end x=3 on the same tick (file order inside a tick): no length, so a tap on the start's column.
    let (chart, report) = sus("#00032a: 11\n#00033a: 21\n");
    assert_eq!(notes_of(&chart), [(0, 0, 0)]);
    assert!(!chart.notes[0].is_hold());
    assert_eq!(report.empty_slides_as_taps, 1);
}

#[test]
fn guides_are_dropped_and_their_overlays_are_not_stray() {
    // A guide on x=2 (beats 0..2) with an elaser on its start, and a real tap.
    let (chart, report) = sus(concat!(
        "#00092a: 1100000021000000\n",
        "#00012: 81\n",
        "#00016: 11\n",
    ));
    assert_eq!(notes_of(&chart), [(0, 4, 0)]);
    assert_eq!(report.guides, 1);
    assert_eq!(report.guide_modifiers, 1);
    assert_eq!(report.stray_overlays, 0);
}

#[test]
fn a_note_inside_a_hold_of_its_column_is_dropped() {
    // Hold on column 0 from beat 0 to beat 2 (1 s); taps on x=2 at beats 1 and 2 (the second starts as the hold ends: kept).
    let (chart, report) = sus(concat!(
        "#00032a: 1100000000000000\n",
        "#00033a: 0000000021000000\n",
        "#00012: 00111100\n",
    ));
    assert_eq!(notes_of(&chart), [(0, 0, 1_000_000), (1_000_000, 0, 0)]);
    assert_eq!(report.overlapping_notes_dropped, 1);
}

#[test]
fn two_taps_on_one_column_and_tick_are_one_note() {
    let (chart, report) = sus("#00012: 11\n#00012: 13\n");
    assert_eq!(notes_of(&chart), [(0, 0, 0)]);
    assert_eq!(report.duplicate_taps, 1);
}

#[test]
fn kinds_this_decoder_does_not_read_are_counted() {
    let (chart, report) = sus(concat!(
        "#00016: 11\n",
        "#00022a: 1100\n", // a generic hold (SUS 2.7, not Project SEKAI)
        "#00044b: 1100\n", // a generic slide 2
        "#00040: 11\n",    // a special number nobody reads
        "#ZZZZZ: 11\n",
        "#TIL00: \"0'0:1.0, 0'960:2.0\"\n",
        "#HISPEED 00\n#NOSPEED\n#BASEBPM 120\n#ATR01: \"pr:100\"\n#SUBTITLE \"x\"\n",
    ));
    assert_eq!(notes_of(&chart), [(0, 4, 0)]);
    assert_eq!(report.unsupported_notes, 2);
    assert_eq!(report.unsupported_lines, 2);
    assert_eq!(report.time_scale_changes, 2);
    assert_eq!(report.ignored_directives, 5);
}

#[test]
fn malformed_data_lines_are_counted_not_fatal() {
    // An odd length drops the last character, `!!` is not a pair, `10` is a note of no width, `0z` is unassigned.
    let (chart, report) = sus("#00012: 111\n#00013: !!\n#00014: 10\n#00015: 0z\n#00016: 11\n");
    assert_eq!(notes_of(&chart), [(0, 0, 0), (0, 4, 0)]);
    assert_eq!(report.odd_data_lines, 1);
    assert_eq!(report.invalid_pairs, 1);
    assert_eq!(report.zero_width_notes, 1);
}

// ---------------------------------------------------------------------------------------------------- SUS: metadata

#[test]
fn the_header_gives_the_metadata() {
    let data = sus_bytes(concat!(
        "#TITLE \"Song: Part 2\"\n",
        "#ARTIST \"Some Artist\"\n",
        "#DESIGNER \"Chart Maker\"\n",
        "#DIFFICULTY 3\n",
        "#PLAYLEVEL 14+\n",
        "#WAVE \"song.ogg\"\n",
        "#GENRE \"Pop\"\n",
        "#BACKGROUND \"bg.png\"\n",
        "#00012: 11\n",
    ));
    let chart = SusDecoder::decode(&data).unwrap();
    let meta = &chart.metadata;
    assert_eq!(meta.title, "Song: Part 2");
    assert_eq!(meta.artist, "Some Artist");
    assert_eq!(meta.creator, "Chart Maker");
    assert_eq!(meta.difficulty_name, "3");
    assert_eq!(meta.difficulty_value, Some(14.5));
    assert_eq!(meta.audio_file, "song.ogg");
    assert_eq!(meta.genre.as_deref(), Some("Pop"));
    assert_eq!(meta.background_file.as_deref(), Some("bg.png"));
    assert_eq!(meta.audio_offset_us, 0);
}

#[test]
fn without_a_header_the_title_is_the_fallback_and_the_audio_is_empty() {
    let data = sus_bytes("#00012: 11\n");
    let chart = SusDecoder::decode_with_title(&data, "master").unwrap();
    assert_eq!(chart.metadata.title, "master");
    assert!(chart.metadata.audio_file.is_empty());
    assert_eq!(chart.metadata.difficulty_value, None);
    // The header's title wins over the fallback; an empty one does not.
    let titled =
        SusDecoder::decode_with_title(&sus_bytes("#TITLE \"Real\"\n#00012: 11\n"), "master")
            .unwrap();
    assert_eq!(titled.metadata.title, "Real");
    let empty =
        SusDecoder::decode_with_title(&sus_bytes("#TITLE \"\"\n#00012: 11\n"), "master").unwrap();
    assert_eq!(empty.metadata.title, "master");
    let (_, report) = sus("#PLAYLEVEL abc\n#00012: 11\n");
    assert_eq!(report.invalid_directives, 1);
}

#[test]
fn crlf_lines_a_bom_and_comments_are_read() {
    let mut data = vec![0xEF, 0xBB, 0xBF];
    data.extend_from_slice(
        b"This line is a comment\r\n#BPM01: 120\r\n#00008: 01\r\n\r\n#00012: 11\r\nanother comment\r\n",
    );
    let (chart, report) = SusDecoder::decode_with_report(&data).unwrap();
    assert_eq!(notes_of(&chart), [(0, 0, 0)]);
    assert!(!report.invalid_text);
}

#[test]
fn text_that_is_not_utf8_is_read_lossily_and_flagged() {
    let mut data = sus_bytes("#TITLE \"caf");
    data.extend_from_slice(&[0xE9, b'"', b'\n']);
    data.extend_from_slice(b"#00012: 11\n");
    let (chart, report) = SusDecoder::decode_with_report(&data).unwrap();
    assert!(report.invalid_text);
    assert_eq!(chart.notes.len(), 1);
    assert!(chart.metadata.title.starts_with("caf"));
}

// ---------------------------------------------------------------------------------------------------- SUS: hostile input

#[test]
fn empty_truncated_and_garbage_files_are_errors_not_panics() {
    assert!(SusDecoder::decode(b"").is_err());
    assert!(SusDecoder::decode(b"#").is_err());
    assert!(SusDecoder::decode(b"#00012").is_err());
    assert!(SusDecoder::decode(b"#BPM01: 120\n#00008: 01\n#0001").is_err());
    assert!(SusDecoder::decode(b"#BPM01: 120\n#00008: 01\n#00012: 1").is_err());
    assert!(SusDecoder::decode(&[0xFF, 0xFE, 0x00, 0x41, 0x23, 0x3A]).is_err());
    // A line cut inside a multi-byte character, and one with non-ASCII in the header.
    let cut = format!("{HEAD}#00012: 1\u{e9}1\n#00\u{e9}12: 11\n");
    assert!(SusDecoder::decode(cut.as_bytes()).is_err());
}

#[test]
fn pseudo_random_bytes_never_panic() {
    let mut state = 0x2545_F491_4F6C_DD1D_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let alphabet = b"#:\n\r \"0123456789abcdefzBPMTILA.-+\xFF\xC3";
    for _ in 0..200 {
        let length = usize::try_from(next() % 400).unwrap();
        let data: Vec<u8> = (0..length)
            .map(|_| alphabet[usize::try_from(next() % alphabet.len() as u64).unwrap()])
            .collect();
        let _ = SusDecoder::decode(&data);
        let _ = UscDecoder::decode(&data);
        // The same with a valid header in front: the lines are parsed far enough to reach the note code.
        let mut with_head = sus_bytes("");
        with_head.extend_from_slice(&data);
        let _ = SusDecoder::decode(&with_head);
    }
}

#[test]
fn a_huge_line_of_empty_slots_is_fine_and_too_many_notes_are_refused() {
    // A million empty slots and one tap on the last: the tap is at 1 - 1/1_000_001 of the measure, not near beat 0.
    let mut line = String::from("#00012: ");
    line.push_str(&"00".repeat(1_000_000));
    line.push_str("11\n");
    let (chart, _) = sus(&line);
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(chart.notes[0].time_us, 1_999_998); // 4 beats x 1_000_000 / 1_000_001 x 0.5 s
    // More than four million notes in one file: an error, not an allocation of the lot.
    let many = format!("#00012: {}\n", "11".repeat(4_000_001));
    let error = sus_error(&many);
    assert!(error.contains("objects"), "{error}");
}

#[test]
fn a_file_above_the_size_limit_is_refused() {
    let data = vec![b'#'; 64 * 1024 * 1024 + 1];
    assert!(
        SusDecoder::decode(&data)
            .unwrap_err()
            .to_string()
            .contains("too large")
    );
    assert!(
        UscDecoder::decode(&data)
            .unwrap_err()
            .to_string()
            .contains("too large")
    );
}

// ---------------------------------------------------------------------------------------------------- USC

#[test]
fn a_usc_tap_is_a_note_on_its_column() {
    // bpm 120, a tap at beat 4 (2 s) on lane x=2 (centre -5.5, half width 0.5).
    let (chart, report) = usc(&json!([bpm_object(0.0, 120.0), usc_single(4.0, 2, 1)]));
    assert_eq!(chart.key_count, 12);
    assert_eq!(notes_of(&chart), [(2_000_000, 0, 0)]);
    assert_eq!(chart.metadata.title, "");
    assert!(chart.metadata.audio_file.is_empty());
    assert_eq!(chart.timing_points.len(), 1);
    assert_eq!(chart.timing_points[0].time_us(), 0); // beat 0, no later than the first note
    assert_eq!(report.ticks_per_beat, 0);
}

#[test]
fn usc_lanes_are_the_leftmost_lane_of_the_centre_and_half_width() {
    // (leftmost SUS lane x, width) -> column x - 2, the same as in a SUS.
    for (x, width, column) in [
        (2, 1, 0),
        (6, 4, 4),
        (13, 1, 11),
        (2, 12, 0),
        (3, 2, 1),
        (12, 2, 10),
    ] {
        let (chart, _) = usc(&json!([bpm_object(0.0, 120.0), usc_single(0.0, x, width)]));
        assert_eq!(notes_of(&chart), [(0, column, 0)], "x {x} width {width}");
    }
    // Not entirely on the stage: left of it, right of it, too wide.
    for (x, width) in [(1, 1), (14, 1), (12, 3), (13, 2), (0, 2)] {
        let objects = json!([
            bpm_object(0.0, 120.0),
            usc_single(0.0, x, width),
            usc_single(1.0, 7, 1)
        ]);
        let (chart, report) = usc(&objects);
        assert_eq!(notes_of(&chart), [(500_000, 5, 0)], "x {x} width {width}");
        assert_eq!(report.notes_outside_lanes, 1);
    }
    // A lane that is not a whole number of lanes from the left is floored; one within 1e-4 of a whole lane is that lane.
    let quarter = json!({"type": "single", "beat": 0, "lane": -5.25, "size": 0.5});
    let near = json!({"type": "single", "beat": 1, "lane": -5.49999, "size": 0.5});
    let (chart, report) = usc(&json!([bpm_object(0.0, 120.0), quarter, near]));
    assert_eq!(notes_of(&chart), [(0, 0, 0), (500_000, 0, 0)]);
    assert_eq!(report.fractional_lanes, 1);
    // No width, or a negative one.
    let flat = json!({"type": "single", "beat": 0, "lane": 0, "size": 0});
    let (_, report) = usc(&json!([
        bpm_object(0.0, 120.0),
        flat,
        usc_single(0.0, 2, 1)
    ]));
    assert_eq!(report.zero_width_notes, 1);
}

#[test]
fn a_usc_may_be_wrapped_the_way_the_editors_write_it() {
    let inner = json!({"offset": 0, "objects": [bpm_object(0.0, 120.0), usc_single(2.0, 2, 1)]});
    for version in [1, 2] {
        let wrapped = json!({"usc": inner, "version": version});
        let chart = UscDecoder::decode(&usc_bytes(&wrapped)).unwrap();
        assert_eq!(notes_of(&chart), [(1_000_000, 0, 0)]);
    }
    let bare = UscDecoder::decode(&usc_bytes(&inner)).unwrap();
    assert_eq!(notes_of(&bare), [(1_000_000, 0, 0)]);
    // The offset is optional, a BOM is allowed.
    let mut with_bom = vec![0xEF, 0xBB, 0xBF];
    with_bom.extend(usc_bytes(
        &json!({"objects": [bpm_object(0.0, 120.0), usc_single(2.0, 2, 1)]}),
    ));
    assert_eq!(
        notes_of(&UscDecoder::decode(&with_bom).unwrap()),
        [(1_000_000, 0, 0)]
    );
}

#[test]
fn usc_bpm_changes_are_beats_and_the_last_of_a_beat_wins() {
    // 120 BPM to beat 4, 240 after: beat 8 is 2 s + 1 s = 3 s. Two changes on beat 0: the last one (120) wins.
    let objects = json!([
        bpm_object(0.0, 60.0),
        bpm_object(0.0, 120.0),
        bpm_object(4.0, 240.0),
        usc_single(0.0, 2, 1),
        usc_single(4.0, 2, 1),
        usc_single(8.0, 2, 1),
    ]);
    let (chart, report) = usc(&objects);
    assert_eq!(
        notes_of(&chart),
        [(0, 0, 0), (2_000_000, 0, 0), (3_000_000, 0, 0)]
    );
    let points: Vec<(i64, Option<f32>)> = chart
        .timing_points
        .iter()
        .map(|p| (p.time_us(), p.bpm_value()))
        .collect();
    assert_eq!(points, [(0, Some(120.0)), (2_000_000, Some(240.0))]);
    assert_eq!(report.bpm_changes, 2);
    // Objects need not be in order.
    let shuffled = json!([
        usc_single(8.0, 2, 1),
        bpm_object(4.0, 240.0),
        bpm_object(0.0, 120.0)
    ]);
    assert_eq!(notes_of(&usc(&shuffled).0), [(3_000_000, 0, 0)]);
}

#[test]
fn the_usc_offset_shifts_the_times_by_itself() {
    // `susToUSC` writes offset = -WAVEOFFSET, so the same shift as a SUS with that offset.
    let objects = json!([
        bpm_object(0.0, 120.0),
        usc_single(0.0, 2, 1),
        usc_single(2.0, 2, 1)
    ]);
    let shifted = |offset: f64| {
        UscDecoder::decode_with_report(&usc_bytes(&json!({"offset": offset, "objects": objects})))
            .unwrap()
    };
    let (early, report) = shifted(-0.5);
    assert_eq!(notes_of(&early), [(-500_000, 0, 0), (500_000, 0, 0)]);
    assert_eq!(early.timing_points[0].time_us(), -500_000);
    assert_eq!(report.shift_seconds, -0.5);
    assert_eq!(
        notes_of(&shifted(0.25).0),
        [(250_000, 0, 0), (1_250_000, 0, 0)]
    );
}

#[test]
fn a_sus_and_the_usc_of_the_same_chart_decode_the_same() {
    // The chart `susToUSC` would write for this SUS: WAVEOFFSET 0.25 is offset -0.25; x=2 w=1 is lane -5.5 size 0.5;
    // x=6 w=4 is lane 0 size 2; the slide x=3 w=2 is lane -4 size 1; 480 ticks a beat.
    let (from_sus, _) = sus(concat!(
        "#WAVEOFFSET 0.25\n#BPM02: 240\n#00108: 02\n",
        "#00012: 11\n#00016: 0014\n#00133a: 1200000022000000\n",
    ));
    let slide = json!({"type": "slide", "active": true, "critical": false, "connections": [
        {"type": "start", "beat": 4, "lane": -4, "size": 1, "trace": false, "critical": false, "ease": "linear"},
        {"type": "end", "beat": 6, "lane": -4, "size": 1, "trace": false, "critical": false}]});
    let objects = json!([
        bpm_object(0.0, 120.0),
        bpm_object(4.0, 240.0),
        usc_single(0.0, 2, 1),
        usc_single(2.0, 6, 4),
        slide
    ]);
    let from_usc =
        UscDecoder::decode(&usc_bytes(&json!({"offset": -0.25, "objects": objects}))).unwrap();
    assert_eq!(notes_of(&from_sus), notes_of(&from_usc));
    assert_eq!(
        notes_of(&from_sus),
        [
            (-250_000, 0, 0),        // beat 0
            (750_000, 4, 0),         // beat 2 = 1 s, minus 0.25
            (1_750_000, 1, 500_000), // beat 4 = 2 s, minus 0.25; two beats at 240 BPM = 0.5 s
        ]
    );
}

#[test]
fn a_usc_slide_is_one_hold_and_its_joints_are_counted() {
    let slide = json!({"type": "slide", "active": true, "critical": true, "connections": [
        {"type": "start", "beat": 0, "lane": -5.5, "size": 0.5, "trace": false, "critical": true, "ease": "linear"},
        {"type": "tick", "beat": 0.5, "lane": -4.5, "size": 0.5, "trace": false, "critical": true, "ease": "out"},
        {"type": "hidden", "beat": 1},
        {"type": "attach", "beat": 1.5, "critical": true},
        {"type": "ignore", "beat": 1.75, "lane": -3.5, "size": 0.5, "ease": "in"},
        {"type": "end", "beat": 2, "lane": -2.5, "size": 0.5, "trace": false, "critical": true, "direction": "up"}]});
    let (chart, report) = usc(&json!([bpm_object(0.0, 120.0), slide]));
    assert_eq!(notes_of(&chart), [(0, 0, 1_000_000)]);
    assert!(chart.notes[0].is_hold());
    assert_eq!(report.slide_joints_dropped, 4);
    assert_eq!(report.critical_flags, 1);
    assert_eq!(report.flicks, 1);
}

#[test]
fn a_usc_slide_that_starts_hidden_is_still_a_hold_and_a_flat_one_a_tap() {
    let hidden_start = json!({"type": "slide", "active": true, "critical": false, "connections": [
        {"type": "ignore", "beat": 0, "lane": -5.5, "size": 0.5, "ease": "linear"},
        {"type": "end", "beat": 1, "lane": -5.5, "size": 0.5, "trace": false, "critical": false}]});
    let flat = json!({"type": "slide", "active": true, "critical": false, "connections": [
        {"type": "start", "beat": 2, "lane": -3.5, "size": 0.5, "trace": false, "critical": false, "ease": "linear"},
        {"type": "end", "beat": 2, "lane": -3.5, "size": 0.5, "trace": false, "critical": false}]});
    let (chart, report) = usc(&json!([bpm_object(0.0, 120.0), hidden_start, flat]));
    assert_eq!(notes_of(&chart), [(0, 0, 500_000), (1_000_000, 2, 0)]);
    assert_eq!(report.empty_slides_as_taps, 1);
}

#[test]
fn usc_guides_traces_damage_and_markers_are_dropped_and_counted() {
    let guide_slide = json!({"type": "slide", "active": false, "critical": false, "connections": [
        {"type": "ignore", "beat": 0, "lane": 0, "size": 1, "ease": "linear"},
        {"type": "ignore", "beat": 1, "lane": 0, "size": 1, "ease": "linear"}]});
    let objects = json!([
        bpm_object(0.0, 120.0),
        usc_single(0.0, 2, 1),
        guide_slide,
        {"type": "guide", "color": "green", "fade": "out", "midpoints": []},
        {"type": "single", "beat": 1, "lane": 0, "size": 1, "trace": true, "critical": true},
        {"type": "single", "beat": 2, "lane": 0, "size": 1, "trace": false, "critical": true, "direction": "left"},
        {"type": "damage", "beat": 3, "lane": 0, "size": 1},
        {"type": "skill", "beat": 4},
        {"type": "feverChance", "beat": 5},
        {"type": "feverStart", "beat": 6},
        {"type": "timeScale", "beat": 0, "timeScale": 2},
        {"type": "timeScaleGroup", "changes": [{"beat": 0, "timeScale": 1}]},
        {"type": "mystery"},
        {"beat": 1},
        7,
    ]);
    let (chart, report) = usc(&objects);
    assert_eq!(notes_of(&chart), [(0, 0, 0), (1_000_000, 5, 0)]);
    assert_eq!(report.guides, 2);
    assert_eq!(report.traces, 1);
    assert_eq!((report.flicks, report.critical_flags), (1, 1));
    assert_eq!(report.damage_notes, 1);
    assert_eq!(report.skill_markers, 1);
    assert_eq!(report.fever_markers, 2);
    assert_eq!(report.time_scale_changes, 2);
    assert_eq!(report.unsupported_objects, 3);
}

#[test]
fn malformed_usc_notes_and_slides_are_counted() {
    let end = |beat: f64| json!({"type": "end", "beat": beat, "lane": 0, "size": 1, "trace": false, "critical": false});
    let start = |beat: f64| json!({"type": "start", "beat": beat, "lane": 0, "size": 1, "trace": false, "critical": false, "ease": "linear"});
    let objects = json!([
        bpm_object(0.0, 120.0),
        usc_single(0.0, 2, 1),
        {"type": "single", "beat": 1, "size": 1},                                       // no lane
        {"type": "single", "beat": "x", "lane": 0, "size": 1},                          // beat is not a number
        {"type": "slide", "active": true, "critical": false, "connections": [start(1.0)]},             // one joint
        {"type": "slide", "active": true, "critical": false},                                          // none
        {"type": "slide", "active": true, "critical": false, "connections": [start(2.0), end(1.0)]},   // back in time
        {"type": "slide", "active": true, "critical": false, "connections": [end(1.0), end(2.0)]},     // no start
        {"type": "slide", "active": true, "critical": false, "connections": [start(1.0), start(2.0)]}, // no end
        {"type": "slide", "active": true, "critical": false, "connections": [start(1.0), {"type": "wiggle", "beat": 1.5}, end(2.0)]},
        {"type": "bpm", "beat": 0, "bpm": 0},
        {"type": "bpm", "beat": 0, "bpm": -1},
        {"type": "bpm", "bpm": 100},
    ]);
    let (chart, report) = usc(&objects);
    assert_eq!(notes_of(&chart), [(0, 0, 0), (500_000, 5, 500_000)]);
    assert_eq!(report.invalid_notes, 2);
    assert_eq!(report.invalid_slides, 3);
    assert_eq!(report.slide_points_without_start, 2);
    assert_eq!(report.unterminated_slides, 1);
    assert_eq!(report.unsupported_notes, 1);
    assert_eq!(report.invalid_bpm, 3);
}

#[test]
fn a_usc_with_no_bpm_or_no_note_is_an_error() {
    let error =
        UscDecoder::decode_with_report(&usc_bytes(&json!({"objects": [usc_single(0.0, 2, 1)]})))
            .unwrap_err();
    assert!(error.to_string().contains("no valid BPM"), "{error}");
    let error =
        UscDecoder::decode_with_report(&usc_bytes(&json!({"objects": [bpm_object(0.0, 120.0)]})))
            .unwrap_err();
    assert!(error.to_string().contains("no playable note"), "{error}");
}

#[test]
fn hostile_usc_input_is_an_error_not_a_panic() {
    for data in [
        &b""[..],
        b"{",
        b"not json",
        b"[]",
        b"null",
        b"{}",
        b"{\"objects\": 3}",
        b"{\"objects\": [], \"offset\": \"x\"}",
        b"{\"usc\": 4, \"objects\": {}}",
        b"{\"objects\": [{\"type\": \"bpm\", \"beat\": 1e999, \"bpm\": 120}]}",
    ] {
        assert!(
            UscDecoder::decode(data).is_err(),
            "{}",
            String::from_utf8_lossy(data)
        );
    }
    // Nesting far beyond what serde_json reads.
    let deep = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
    assert!(UscDecoder::decode(deep.as_bytes()).is_err());
    let deep_object = format!("{{\"objects\": [{}]}}", "{\"a\":".repeat(50_000));
    assert!(UscDecoder::decode(deep_object.as_bytes()).is_err());
    // A beat so far that its time is out of range is dropped; so is one that overflows to infinity.
    let objects = json!([
        bpm_object(0.0, 120.0),
        usc_single(0.0, 2, 1),
        usc_single(1e300, 2, 1),
        usc_single(1e12, 2, 1)
    ]);
    let (chart, report) = usc(&objects);
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(report.notes_out_of_range, 2);
}

#[test]
fn too_many_usc_objects_are_refused() {
    let data = format!("{{\"objects\": [{}0]}}", "0,".repeat(4_000_000));
    let error = UscDecoder::decode(data.as_bytes()).unwrap_err();
    assert!(error.to_string().contains("objects"), "{error}");
}

// ---------------------------------------------------------------------------------------------------- dispatch

#[test]
fn auto_decode_routes_a_sus_and_a_usc_by_their_path() {
    use crate::formats::auto::{
        auto_decode, auto_decode_all, auto_decode_bytes, auto_decode_variant,
    };
    let dir = std::env::temp_dir().join(format!("rox_sekai_auto_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sus_path = dir.join("master.sus");
    let titled_path = dir.join("titled.sus");
    let usc_path = dir.join("chart.usc");
    let sus_data = sus_bytes("#00012: 11\n");
    std::fs::write(&sus_path, &sus_data).unwrap();
    std::fs::write(&titled_path, sus_bytes("#TITLE \"Real\"\n#00012: 11\n")).unwrap();
    let usc_data = usc_bytes(
        &json!({"offset": 0, "objects": [bpm_object(0.0, 120.0), usc_single(0.0, 2, 1)]}),
    );
    std::fs::write(&usc_path, &usc_data).unwrap();

    let sus_chart = auto_decode(&sus_path).unwrap();
    let titled = auto_decode(&titled_path).unwrap();
    let usc_chart = auto_decode(&usc_path).unwrap();
    let sus_zero = auto_decode_variant(&sus_path, 0).unwrap();
    let usc_zero = auto_decode_variant(&usc_path, 0).unwrap();
    let sus_one = auto_decode_variant(&sus_path, 1);
    let usc_one = auto_decode_variant(&usc_path, 1);
    let sus_all = auto_decode_all(&sus_path).unwrap();
    let usc_all = auto_decode_all(&usc_path).unwrap();
    let hinted_sus = auto_decode_bytes("x/master.sus", &sus_data).unwrap();
    let hinted_usc = auto_decode_bytes("x/chart.usc", &usc_data).unwrap();
    let written = crate::formats::auto::auto_encode(&sus_chart, dir.join("out.sus"));
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(
        (sus_chart.key_count, sus_chart.metadata.title.as_str()),
        (12, "master")
    );
    assert_eq!(titled.metadata.title, "Real");
    assert_eq!(
        (usc_chart.key_count, usc_chart.metadata.title.as_str()),
        (12, "chart")
    );
    assert_eq!(sus_zero.notes, sus_chart.notes);
    assert_eq!(usc_zero.notes, usc_chart.notes);
    assert!(sus_one.is_err() && usc_one.is_err());
    assert!(
        sus_all.is_none() && usc_all.is_none(),
        "one chart per file: read with auto_decode_variant"
    );
    assert_eq!(hinted_sus.metadata.title, "master");
    assert_eq!(hinted_usc.metadata.title, "chart");
    assert!(written.is_err(), "decode only");
}

#[test]
fn the_first_bytes_tell_a_sus_and_a_usc_from_other_files() {
    assert!(looks_like_sus(&sus_bytes("#00012: 11\n")));
    assert!(looks_like_sus(b"\xEF\xBB\xBFcomment\r\n  #TITLE \"x\"\r\n"));
    assert!(looks_like_sus(b"#BPM01: 120"));
    for other in [
        &b""[..],
        b"plain text\nwith no directive\n",
        b"# \n#\n",
        b"\x00#TITLE \"x\"",
        b"{\"objects\": []}",
    ] {
        assert!(!looks_like_sus(other), "{}", String::from_utf8_lossy(other));
    }
    assert!(looks_like_usc(br#"{"offset": 0, "objects": []}"#));
    assert!(looks_like_usc(
        b"\xEF\xBB\xBF \n {\"usc\": {\"objects\": []}, \"version\": 2}"
    ));
    for other in [
        &b""[..],
        b"[]",
        b"{\"tracks\": []}",
        b"#TITLE \"x\"",
        b"\"objects\"",
    ] {
        assert!(!looks_like_usc(other), "{}", String::from_utf8_lossy(other));
    }
    // What the sniff accepts is what the decoders read.
    let usc_text = usc_bytes(&json!({"objects": [bpm_object(0.0, 120.0), usc_single(0.0, 2, 1)]}));
    assert!(looks_like_usc(&usc_text) && UscDecoder::decode(&usc_text).is_ok());
}

// ---------------------------------------------------------------------------------------------------- BDON

#[test]
fn a_bdon_chart_maps_its_24_unit_board_onto_the_twelve_columns() {
    // 480 ticks per beat; no `bpm` event: 120 BPM, a beat is 0.5 s. `pos` is the left edge on 24 units: `pos / 2` is the column.
    let file = json!({"score": {
        "events": {"skill": [960], "fever": [[0, 960]]},
        "notes": [
            {"t": 480, "pos": 12, "size": 8},
            {"type": "flick", "t": 960, "pos": 0, "size": 6, "dir": "right"},
            {"type": "long", "node": [{"t": 1440, "pos": 22, "size": 2}, {"t": 1680, "pos": "auto"}, {"t": 1920, "pos": 4}]},
            {"type": "trace", "t": 480, "pos": 0},
            {"type": "guide", "node": [{"t": 0, "pos": 0}, {"t": 480, "pos": 0}]},
            {"t": 480, "pos": 4, "visible": false},
            {"t": 480, "pos": 20, "size": 8}
        ]
    }});
    let (chart, report) = super::BdonDecoder::decode_with_report(&serde_json::to_vec(&file).unwrap()).unwrap();
    assert_eq!(chart.key_count, COLUMNS);
    assert_eq!(notes_of(&chart), vec![(500_000, 6, 0), (1_000_000, 0, 0), (1_500_000, 11, 500_000)]);
    assert_eq!(chart.timing_points, vec![TimingPoint::bpm(0, 120.0)]);
    assert_eq!(
        (report.flicks, report.traces, report.guides, report.hidden_notes, report.slide_joints_dropped, report.notes_outside_lanes),
        (1, 1, 1, 1, 1, 1)
    );
    assert_eq!((report.skill_markers, report.fever_markers), (1, 1));
}

#[test]
fn a_bdon_json_is_told_by_its_score_key() {
    assert!(super::looks_like_bdon(br#"{"meta":{"version":100},"score":{"notes":[]}}"#));
    assert!(!super::looks_like_bdon(br#"{"song":{"notes":[]}}"#));
    assert!(!super::looks_like_bdon(br#"[{"type":"BPM","bpm":120,"beat":0}]"#));
    assert!(super::BdonDecoder::decode(&[0x1F, 0x8B, 8, 0]).unwrap_err().to_string().contains("gunzip"));
}
