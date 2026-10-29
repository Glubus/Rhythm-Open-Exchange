//! Tests of the Bestdori and BanG! Simulator decoders. Every file here is written by hand in the test (the simulator notes' own example is
//! used where it gives one); no real chart is used: community charts carry no licence to redistribute them. Times are computed by hand in the
//! comments: at 120 BPM a beat is 0.5 s.

use rox::codec::Decoder;
use rox::model::RoxChart;
use serde_json::{Value, json};

use super::{
    BandoriReport, BestdoriDecoder, COLUMNS, SimulatorDecoder, is_bandori_chart_file,
    is_bestdori_chart_name, looks_like_bestdori, looks_like_simulator,
};

// ---------------------------------------------------------------------------------------------------- builders

fn bestdori_bytes(elements: &Value) -> Vec<u8> {
    serde_json::to_vec(elements).unwrap()
}

fn bestdori(elements: &Value) -> (RoxChart, BandoriReport) {
    BestdoriDecoder::decode_with_report(&bestdori_bytes(elements)).unwrap()
}

fn bestdori_error(elements: &Value) -> String {
    BestdoriDecoder::decode_with_report(&bestdori_bytes(elements))
        .unwrap_err()
        .to_string()
}

fn sim(text: &str) -> (RoxChart, BandoriReport) {
    SimulatorDecoder::decode_with_report(text.as_bytes()).unwrap()
}

fn sim_error(text: &str) -> String {
    SimulatorDecoder::decode_with_report(text.as_bytes())
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

fn points_of(chart: &RoxChart) -> Vec<(i64, Option<f32>)> {
    chart
        .timing_points
        .iter()
        .map(|p| (p.time_us(), p.bpm_value()))
        .collect()
}

fn bpm(beat: f64, bpm: f64) -> Value {
    json!({"type": "BPM", "beat": beat, "bpm": bpm})
}

fn single(beat: f64, lane: f64) -> Value {
    json!({"type": "Single", "beat": beat, "lane": lane})
}

/// A `Slide` or `Long` from `(beat, lane)` pairs.
fn slide(kind: &str, joints: &[(f64, f64)]) -> Value {
    let connections: Vec<Value> = joints
        .iter()
        .map(|&(beat, lane)| json!({"beat": beat, "lane": lane}))
        .collect();
    json!({"type": kind, "connections": connections})
}

/// 46 ms offset, 120 BPM, the music at beat 0: the head of every file `BanGConverter` writes.
const HEAD: &str = "46\n120\n0/0/0\n";

// ---------------------------------------------------------------------------------------------------- Bestdori: lanes and notes

#[test]
fn a_bestdori_single_is_a_tap_on_its_column_of_seven() {
    // 120 BPM: a beat is 0.5 s. Lanes 0 and 6 are the outer columns.
    let (chart, report) = bestdori(&json!([
        bpm(0.0, 120.0),
        single(2.0, 6.0),
        single(1.0, 0.0),
        single(1.5, 3.0)
    ]));
    assert_eq!((chart.key_count, COLUMNS), (7, 7));
    assert_eq!(
        notes_of(&chart),
        [(500_000, 0, 0), (750_000, 3, 0), (1_000_000, 6, 0)]
    );
    assert_eq!(points_of(&chart), [(0, Some(120.0))]);
    assert_eq!(chart.metadata.audio_file, "");
    assert_eq!(
        report,
        BandoriReport {
            bpm_changes: 1,
            ..BandoriReport::default()
        }
    );
}

#[test]
fn lanes_outside_the_seven_or_not_whole_are_dropped_and_counted() {
    // -1 and 7 are not lanes (ayachan would clamp them onto lanes 0 and 7); 2.5 is not a whole lane; 3.00001 is lane 3 as a float rounding.
    let (chart, report) = bestdori(&json!([
        bpm(0.0, 120.0),
        single(0.0, -1.0),
        single(1.0, 7.0),
        single(2.0, 2.5),
        single(3.0, 3.00001),
        single(4.0, 1e30)
    ]));
    assert_eq!(notes_of(&chart), [(1_500_000, 3, 0)]);
    assert_eq!(report.lanes_out_of_range, 3);
    assert_eq!(report.fractional_lanes, 1);
}

#[test]
fn flags_are_counted_and_the_note_stays() {
    let (chart, report) = bestdori(&json!([
        bpm(0.0, 120.0),
        {"type": "Single", "beat": 1, "lane": 0, "flick": true},
        {"type": "Single", "beat": 2, "lane": 1, "skill": true},
        {"type": "Single", "beat": 3, "lane": 2, "charge": true},
        {"type": "Single", "beat": 4, "lane": 3, "flick": false, "skill": false},
        {"type": "Directional", "beat": 5, "lane": 4, "direction": "Left", "width": 2},
        {"type": "Directional", "beat": 6, "lane": 5, "direction": "Right", "width": 7}
    ]));
    assert_eq!(chart.notes.len(), 6);
    assert!(chart.notes.iter().all(|note| !note.is_hold()));
    assert_eq!(
        (
            report.flicks,
            report.skill_flags,
            report.charge_flags,
            report.directional_flicks
        ),
        (1, 1, 1, 2)
    );
}

#[test]
fn a_directional_without_a_lane_or_a_beat_is_an_invalid_note() {
    let (chart, report) = bestdori(&json!([
        bpm(0.0, 120.0),
        single(1.0, 0.0),
        {"type": "Directional", "beat": 2, "direction": "Left", "width": 1},
        {"type": "Single", "lane": 2},
        {"type": "Single", "beat": "2", "lane": 2},
        {"type": "Single", "beat": 2, "lane": null}
    ]));
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(report.invalid_notes, 4);
}

#[test]
fn unknown_types_and_elements_that_are_not_objects_are_counted() {
    // A Bestdori v1 chart is `System` and `Note` elements; none is read.
    let (chart, report) = bestdori(&json!([
        bpm(0.0, 120.0),
        single(1.0, 0.0),
        {"type": "System", "cmd": "BPM", "beat": 0, "bpm": 120},
        {"type": "Note", "note": "Single", "beat": 1, "lane": 1},
        {"type": "single", "beat": 2, "lane": 1},
        {"beat": 2, "lane": 1},
        {"type": 7},
        3,
        "x",
        null
    ]));
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(report.unknown_types, 5);
    assert_eq!(report.invalid_objects, 3);
}

// ---------------------------------------------------------------------------------------------------- Bestdori: long notes and slides

#[test]
fn a_long_is_one_hold_on_its_lane() {
    // Beats 2 and 4 are 1 s and 2 s.
    let (chart, report) = bestdori(&json!([
        bpm(0.0, 120.0),
        slide("Long", &[(2.0, 3.0), (4.0, 3.0)])
    ]));
    assert_eq!(notes_of(&chart), [(1_000_000, 3, 1_000_000)]);
    assert!(chart.notes[0].is_hold());
    assert_eq!(
        report,
        BandoriReport {
            bpm_changes: 1,
            ..BandoriReport::default()
        }
    );
}

#[test]
fn a_slide_is_one_hold_from_its_first_joint_to_its_last_on_the_first_lane() {
    // Joints at beats 1, 2 (hidden), 3 and 4 (a flick): the hold is 0.5 s to 2 s on the first joint's lane 1; the two joints between are
    // dropped and the path across lanes 4 and 2 is lost.
    let slide = json!({"type": "Slide", "connections": [
        {"beat": 1, "lane": 1},
        {"beat": 2, "lane": 4, "hidden": true},
        {"beat": 3, "lane": 2},
        {"beat": 4, "lane": 5, "flick": true}
    ]});
    let (chart, report) = bestdori(&json!([bpm(0.0, 120.0), slide]));
    assert_eq!(notes_of(&chart), [(500_000, 1, 1_500_000)]);
    assert_eq!(report.slide_joints_dropped, 2);
    assert_eq!(report.slides_with_lane_changes, 1);
    assert_eq!(report.flicks, 1);
}

#[test]
fn a_flick_in_the_middle_of_a_slide_is_not_counted_only_the_release_is() {
    let slide = json!({"type": "Slide", "connections": [
        {"beat": 1, "lane": 1, "flick": true},
        {"beat": 2, "lane": 1, "flick": true},
        {"beat": 3, "lane": 1, "skill": true, "charge": true}
    ]});
    let (_, report) = bestdori(&json!([bpm(0.0, 120.0), slide]));
    assert_eq!(report.flicks, 0);
    assert_eq!((report.skill_flags, report.charge_flags), (1, 1));
}

#[test]
fn a_slide_with_one_connection_is_a_tap_and_one_with_none_is_dropped() {
    let (chart, report) = bestdori(&json!([
        bpm(0.0, 120.0),
        slide("Slide", &[(2.0, 5.0)]),
        {"type": "Long", "connections": []},
        {"type": "Long"},
        {"type": "Long", "connections": 3}
    ]));
    assert_eq!(notes_of(&chart), [(1_000_000, 5, 0)]);
    assert_eq!(report.single_connection_slides, 1);
    assert_eq!(report.invalid_slides, 3);
}

#[test]
fn a_slide_that_goes_back_in_time_or_lacks_a_field_is_dropped() {
    let (chart, report) = bestdori(&json!([
        bpm(0.0, 120.0),
        single(1.0, 0.0),
        slide("Slide", &[(4.0, 1.0), (2.0, 1.0)]),
        {"type": "Slide", "connections": [{"beat": 1, "lane": 1}, {"beat": 2}]},
        {"type": "Slide", "connections": [{"beat": 1, "lane": 1}, {"lane": 2}]},
        {"type": "Slide", "connections": [{"beat": 1, "lane": 1}, 5]}
    ]));
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(report.invalid_slides, 4);
}

#[test]
fn a_slide_that_ends_on_its_start_is_a_tap() {
    let (chart, report) = bestdori(&json!([
        bpm(0.0, 120.0),
        slide("Long", &[(2.0, 5.0), (2.0, 5.0)])
    ]));
    assert_eq!(notes_of(&chart), [(1_000_000, 5, 0)]);
    assert_eq!(report.empty_slides_as_taps, 1);
}

#[test]
fn a_slide_on_a_lane_the_game_does_not_have_is_dropped_with_its_other_joints() {
    // Only the first joint's lane decides the column; a hidden joint off the stage (ayachan does not clamp it) is harmless.
    let (chart, report) = bestdori(&json!([
        bpm(0.0, 120.0),
        single(0.0, 0.0),
        slide("Slide", &[(1.0, 9.0), (2.0, 2.0)]),
        slide("Slide", &[(3.0, 2.0), (4.0, 12.0)])
    ]));
    assert_eq!(notes_of(&chart), [(0, 0, 0), (1_500_000, 2, 500_000)]);
    assert_eq!(report.lanes_out_of_range, 1);
}

#[test]
fn a_note_inside_a_hold_of_its_column_is_dropped_and_a_second_tap_on_one_time_too() {
    // The hold is on column 2 from beat 1 to beat 3 (0.5 s to 1.5 s): the tap on column 2 at beat 2 is inside it, the tap at beat 3 (its
    // release) is not. The two taps at beat 5 on column 4 are one.
    let (chart, report) = bestdori(&json!([
        bpm(0.0, 120.0),
        slide("Long", &[(1.0, 2.0), (3.0, 2.0)]),
        single(2.0, 2.0),
        single(2.0, 3.0),
        single(3.0, 2.0),
        single(5.0, 4.0),
        single(5.0, 4.0)
    ]));
    assert_eq!(
        notes_of(&chart),
        [
            (500_000, 2, 1_000_000),
            (1_000_000, 3, 0),
            (1_500_000, 2, 0),
            (2_500_000, 4, 0)
        ]
    );
    assert_eq!(report.overlapping_notes_dropped, 1);
    assert_eq!(report.duplicate_taps, 1);
}

// ---------------------------------------------------------------------------------------------------- Bestdori: time

#[test]
fn a_bpm_change_moves_the_times_after_it() {
    // 120 BPM to beat 4 (2 s), then 240 BPM (0.25 s a beat): beat 2 = 1 s, beat 4 = 2 s, beat 6 = 2.5 s.
    let (chart, report) = bestdori(&json!([
        bpm(4.0, 240.0),
        bpm(0.0, 120.0),
        single(6.0, 0.0),
        single(2.0, 0.0),
        single(4.0, 0.0)
    ]));
    assert_eq!(
        notes_of(&chart),
        [(1_000_000, 0, 0), (2_000_000, 0, 0), (2_500_000, 0, 0)]
    );
    assert_eq!(
        points_of(&chart),
        [(0, Some(120.0)), (2_000_000, Some(240.0))]
    );
    assert_eq!(report.bpm_changes, 2);
}

#[test]
fn a_chart_with_no_bpm_at_beat_zero_is_at_120_until_its_first_event() {
    // BanGConverter writes 120 as the simulator's BPM line then. 240 BPM from beat 4: beat 2 = 1 s, beat 6 = 2 s + 0.5 s.
    let (chart, _) = bestdori(&json!([
        bpm(4.0, 240.0),
        single(2.0, 0.0),
        single(6.0, 0.0)
    ]));
    assert_eq!(notes_of(&chart), [(1_000_000, 0, 0), (2_500_000, 0, 0)]);
    assert_eq!(
        points_of(&chart),
        [(0, Some(120.0)), (2_000_000, Some(240.0))]
    );
    // And a chart with no BPM at all.
    let (chart, report) = bestdori(&json!([single(2.0, 0.0)]));
    assert_eq!(notes_of(&chart), [(1_000_000, 0, 0)]);
    assert_eq!(points_of(&chart), [(0, Some(120.0))]);
    assert_eq!(report.bpm_changes, 1);
}

#[test]
fn the_bpm_at_beat_zero_is_the_last_event_at_or_before_it_and_the_last_of_a_beat_wins() {
    // Events at beats -1 (60), 0 (120): 120 from beat 0. Two events at beat 4 (240 then 300): 300 wins, 0.2 s a beat: beat 6 = 2.4 s.
    let (chart, report) = bestdori(&json!([
        bpm(-1.0, 60.0),
        bpm(0.0, 120.0),
        bpm(4.0, 240.0),
        bpm(4.0, 300.0),
        single(1.0, 0.0),
        single(6.0, 0.0)
    ]));
    assert_eq!(notes_of(&chart), [(500_000, 0, 0), (2_400_000, 0, 0)]);
    assert_eq!(
        points_of(&chart),
        [(0, Some(120.0)), (2_000_000, Some(300.0))]
    );
    assert_eq!(report.bpm_changes, 2);
}

#[test]
fn a_negative_bpm_is_its_absolute_value_and_an_invalid_one_is_ignored() {
    let (chart, report) = bestdori(&json!([
        bpm(0.0, -120.0),
        bpm(2.0, 0.0),
        bpm(2.0, 1e300),
        {"type": "BPM", "bpm": 90},
        {"type": "BPM", "beat": 3},
        single(2.0, 0.0)
    ]));
    assert_eq!(notes_of(&chart), [(1_000_000, 0, 0)]);
    assert_eq!(points_of(&chart), [(0, Some(120.0))]);
    assert_eq!(
        (report.negative_bpms, report.invalid_bpm, report.bpm_changes),
        (1, 4, 1)
    );
}

#[test]
fn a_note_before_beat_zero_is_before_the_audio_and_dropped() {
    let (chart, report) = bestdori(&json!([
        bpm(0.0, 120.0),
        single(-1.0, 0.0),
        slide("Long", &[(-2.0, 1.0), (1.0, 1.0)]),
        single(0.0, 2.0),
        single(-1e-9, 3.0)
    ]));
    assert_eq!(notes_of(&chart), [(0, 2, 0)]);
    assert_eq!(report.notes_before_start, 3);
}

#[test]
fn a_time_beyond_ten_million_seconds_is_dropped() {
    // 1e308 beats is finite and far beyond; so is a beat that overflows the seconds.
    let (chart, report) = bestdori(&json!([
        bpm(0.0, 120.0),
        single(1.0, 0.0),
        single(1e308, 1.0),
        slide("Long", &[(1.0, 2.0), (1e308, 2.0)])
    ]));
    assert_eq!(notes_of(&chart), [(500_000, 0, 0)]);
    assert_eq!(report.notes_out_of_range, 2);
}

#[test]
fn the_title_is_given_by_the_caller_and_the_audio_is_never_named() {
    let data = bestdori_bytes(&json!([bpm(0.0, 120.0), single(1.0, 0.0)]));
    let chart = BestdoriDecoder::decode_with_title(&data, "Song").unwrap();
    assert_eq!(chart.metadata.title, "Song");
    assert_eq!(chart.metadata.audio_file, "");
    assert_eq!(chart.metadata.audio_offset_us, 0);
    assert_eq!(BestdoriDecoder::decode(&data).unwrap().metadata.title, "");
    let mut bom = b"\xEF\xBB\xBF".to_vec();
    bom.extend_from_slice(&data);
    assert!(BestdoriDecoder::decode(&bom).is_ok());
}

// ---------------------------------------------------------------------------------------------------- Bestdori: hostile input

#[test]
fn bestdori_input_that_is_not_a_chart_is_an_error_never_a_panic() {
    for data in [
        &b""[..],
        b"[",
        b"[{\"type\": \"BPM\", \"beat\": 0, \"bpm\": 120},",
        b"{\"type\": \"BPM\"}",
        b"\"text\"",
        b"[NaN]",
        b"[{\"type\":\"Single\",\"beat\":NaN,\"lane\":1}]",
        b"[{\"type\":\"Single\",\"beat\":1e999,\"lane\":1}]",
        b"\xFF\xFE[]",
        b"[]",
        b"[{}]",
    ] {
        assert!(
            BestdoriDecoder::decode(data).is_err(),
            "{}",
            String::from_utf8_lossy(data)
        );
    }
    assert!(bestdori_error(&json!({"type": "BPM"})).contains("array"));
    assert!(bestdori_error(&json!([])).contains("no playable note"));
    assert!(bestdori_error(&json!([bpm(0.0, 120.0)])).contains("no playable note"));
}

#[test]
fn a_deeply_nested_or_huge_bestdori_file_is_refused() {
    let nested = "[".repeat(100_000);
    assert!(BestdoriDecoder::decode(nested.as_bytes()).is_err());
    let too_many = format!("[{}0]", "0,".repeat(4_000_000));
    let error = BestdoriDecoder::decode(too_many.as_bytes()).unwrap_err();
    assert!(error.to_string().contains("elements"), "{error}");
    let huge = vec![b' '; 16 * 1024 * 1024 + 1];
    let error = BestdoriDecoder::decode(&huge).unwrap_err();
    assert!(error.to_string().contains("too large"), "{error}");
}

// ---------------------------------------------------------------------------------------------------- Simulator: notes

#[test]
#[allow(clippy::cast_possible_truncation)] // a few hundred thousand microseconds
fn the_notes_own_example_is_read() {
    // `Bang info.txt`: offset 46, BPM 131, the music at beat 0, then normal notes. A beat is 60 / 131 s.
    let (chart, report) = sim("46\n131\n0/0/0\n8/1/3\n8.5/1/6\n8.75/1/6\n9.25/1/1\n");
    assert_eq!(chart.key_count, 7);
    let beat = 60.0 / 131.0;
    let expected: Vec<(i64, u8)> = [(8.0, 2), (8.5, 5), (8.75, 5), (9.25, 0)]
        .iter()
        .map(|&(b, column)| ((b * beat * 1e6_f64).round() as i64, column))
        .collect();
    let got: Vec<(i64, u8)> = notes_of(&chart).iter().map(|n| (n.0, n.1)).collect();
    assert_eq!(got, expected);
    assert_eq!(report.note_offset, Some(46.0));
    assert_eq!(points_of(&chart), [(0, Some(131.0))]);
}

#[test]
fn a_simulator_lane_is_one_to_seven() {
    // 120 BPM. Lanes 1 and 7 are the outer columns; 0 and 8 are not lanes; 2.5 is not a whole lane.
    let (chart, report) = sim(&format!(
        "{HEAD}1/1/1\n2/1/7\n3/1/0\n4/1/8\n5/1/2.5\n6/1/4.00001\n"
    ));
    assert_eq!(
        notes_of(&chart),
        [(500_000, 0, 0), (1_000_000, 6, 0), (3_000_000, 3, 0)]
    );
    assert_eq!(report.lanes_out_of_range, 2);
    assert_eq!(report.fractional_lanes, 1);
}

#[test]
fn flicks_skill_fever_and_directional_notes_are_taps_and_counted() {
    // 2 flick, 10 fever single, 11 skill, 51 / 57 directional left width 1 / 7, 61 / 67 right.
    let (chart, report) = sim(&format!(
        "{HEAD}1/1/1\n2/2/2\n3/10/3\n4/11/4\n5/51/5\n6/57/6\n7/61/7\n8/67/1\n"
    ));
    assert_eq!(chart.notes.len(), 8);
    assert!(chart.notes.iter().all(|n| !n.is_hold()));
    assert_eq!(
        (
            report.flicks,
            report.charge_flags,
            report.skill_flags,
            report.directional_flicks
        ),
        (1, 1, 1, 4)
    );
}

#[test]
fn unknown_types_and_malformed_lines_are_counted_never_taps() {
    // 9, 14, 40, 68 and 99 are no type of the notes; the others are not `beat/type/lane` (a 4-field line is only a numbered slide).
    let (chart, report) = sim(&format!(
        "{HEAD}1/1/1\n1/9/1\n1/14/1\n1/40/1\n1/68/1\n1/99/1\nabc\n1/1\n1/x/3\n1/1/3/4\nnan/1/3\n1/1/inf\n1/71/3\n\n   \n"
    ));
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(report.unknown_types, 5);
    assert_eq!(report.invalid_notes, 7);
}

// ---------------------------------------------------------------------------------------------------- Simulator: long notes and slides

#[test]
fn a_long_note_is_one_hold_and_longs_are_paired_by_lane() {
    // Lane 1: beats 1 to 3 (0.5 s, 1 s long); lane 5, starting inside it: beats 2 to 4 (1 s, 1 s long). Types 31 and 32 carry the skill
    // flag, 26 ends with a flick.
    let (chart, report) = sim(&format!("{HEAD}1/21/1\n2/31/5\n3/25/1\n4/26/5\n"));
    assert_eq!(
        notes_of(&chart),
        [(500_000, 0, 1_000_000), (1_000_000, 4, 1_000_000)]
    );
    assert_eq!((report.skill_flags, report.flicks), (1, 1));
    let (_, ends_with_skill) = sim(&format!("{HEAD}1/21/1\n3/32/1\n"));
    assert_eq!(ends_with_skill.skill_flags, 1);
}

#[test]
fn slides_a_and_b_interleave_and_each_is_one_hold_on_its_first_lane() {
    // A: lane 2 at beat 1, 4 (among) at beat 2, lane 3 at beat 3 (end with a flick, type 12): 0.5 s to 1.5 s on column 1.
    // B: lane 6 at beat 1.5, 5 at beat 2.5 (hidden among, type 42), lane 6 at beat 4 (end with a flick, 13): 0.75 s to 2 s on column 5.
    let (chart, report) = sim(&format!(
        "{HEAD}1/3/2\n1.5/6/6\n2/4/4\n2.5/42/5\n3/12/3\n4/13/6\n"
    ));
    assert_eq!(
        notes_of(&chart),
        [(500_000, 1, 1_000_000), (750_000, 5, 1_250_000)]
    );
    assert_eq!(report.slide_joints_dropped, 2);
    assert_eq!(report.slides_with_lane_changes, 2);
    assert_eq!(report.flicks, 2);
}

#[test]
fn slides_with_skill_starts_and_ends_and_a_slide_that_stays_on_its_lane() {
    // 33 / 34 chain A, 35 / 36 chain B, skill on the start and on the end.
    let (chart, report) = sim(&format!("{HEAD}1/33/1\n2/34/1\n3/35/4\n5/36/4\n"));
    assert_eq!(
        notes_of(&chart),
        [(500_000, 0, 500_000), (1_500_000, 3, 1_000_000)]
    );
    assert_eq!(report.skill_flags, 4);
    assert_eq!(report.slides_with_lane_changes, 0);
}

#[test]
fn the_numbered_slides_of_types_71_to_77_are_told_by_their_number() {
    // `beat/type/number/lane`: slide 1 on lane 2 from beat 1 to 3, slide 2 on lane 6 from beat 1 to 2 (an end with a flick, 74).
    let (chart, report) = sim(&format!(
        "{HEAD}1/71/1/2\n1/71/2/6\n2/77/1/3\n2/74/2/6\n3/73/1/4\n"
    ));
    assert_eq!(
        notes_of(&chart),
        [(500_000, 1, 1_000_000), (500_000, 5, 500_000)]
    );
    assert_eq!((report.flicks, report.slide_joints_dropped), (1, 1));
    assert_eq!(report.slides_with_lane_changes, 1);
}

#[test]
fn a_slide_with_no_end_or_no_start_is_dropped_and_counted() {
    // Chain A: start at 1, a second start at 2 (the first never ended), an end at 3; a joint and an end with no start; a long note never
    // ended; chain B starts and the file ends.
    let (chart, report) = sim(&format!(
        "{HEAD}0.5/5/1\n0.5/4/1\n1/3/1\n2/3/2\n3/5/2\n4/5/2\n5/4/2\n6/21/7\n7/6/3\n"
    ));
    assert_eq!(notes_of(&chart), [(1_000_000, 1, 500_000)]);
    assert_eq!(report.unterminated_slides, 3);
    assert_eq!(report.slide_points_without_start, 4);
}

#[test]
fn a_zero_length_slide_is_a_tap_and_a_slide_on_a_bad_lane_is_dropped_whole() {
    let (chart, report) = sim(&format!(
        "{HEAD}1/21/3\n1/25/3\n2/3/9\n3/5/2\n4/21/0\n5/25/0\n"
    ));
    assert_eq!(notes_of(&chart), [(500_000, 2, 0)]);
    assert_eq!(report.empty_slides_as_taps, 1);
    assert_eq!(report.lanes_out_of_range, 3);
}

#[test]
fn a_tap_inside_a_hold_of_its_column_is_dropped() {
    let (chart, report) = sim(&format!("{HEAD}1/21/2\n2/1/2\n2/1/3\n3/25/2\n3/1/2\n"));
    assert_eq!(
        notes_of(&chart),
        [
            (500_000, 1, 1_000_000),
            (1_000_000, 2, 0),
            (1_500_000, 1, 0)
        ]
    );
    assert_eq!(report.overlapping_notes_dropped, 1);
}

#[test]
fn simulator_lines_are_read_in_beat_order() {
    // The slide's end is written before its start; the lines are sorted by beat, as BanGConverter does.
    let (chart, _) = sim(&format!("{HEAD}3/5/2\n1/3/2\n"));
    assert_eq!(notes_of(&chart), [(500_000, 1, 1_000_000)]);
}

// ---------------------------------------------------------------------------------------------------- Simulator: time

#[test]
fn a_type_20_line_changes_the_bpm_at_its_beat() {
    // 120 BPM, 240 from beat 4: beat 2 = 1 s, beat 4 = 2 s, beat 6 = 2.5 s. The BPM is the third field.
    let (chart, report) = sim(&format!("{HEAD}6/1/1\n4/20/240\n2/1/1\n4/1/1\n"));
    assert_eq!(
        notes_of(&chart),
        [(1_000_000, 0, 0), (2_000_000, 0, 0), (2_500_000, 0, 0)]
    );
    assert_eq!(
        points_of(&chart),
        [(0, Some(120.0)), (2_000_000, Some(240.0))]
    );
    assert_eq!(report.bpm_changes, 2);
}

#[test]
fn a_type_20_line_at_beat_zero_replaces_the_header_bpm_and_an_invalid_one_is_ignored() {
    let (chart, report) = sim(&format!("{HEAD}0/20/60\n1/1/1\n2/20/0\n3/20/nan\n"));
    assert_eq!(notes_of(&chart), [(1_000_000, 0, 0)]);
    assert_eq!(points_of(&chart), [(0, Some(60.0))]);
    assert_eq!(report.invalid_bpm, 1);
    assert_eq!(report.invalid_notes, 1);
}

#[test]
fn the_music_start_line_says_which_beat_is_time_zero() {
    // The music starts at beat 2: a note at beat 3 is 0.5 s in, a note at beat 1 is before the audio, a second `0/0/0` is ignored.
    let (chart, report) = sim("46\n120\n2/0/0\n1/1/1\n3/1/2\n2/1/3\n5/0/0\n");
    assert_eq!(notes_of(&chart), [(0, 2, 0), (500_000, 1, 0)]);
    assert_eq!(report.notes_before_start, 1);
    assert_eq!(report.extra_music_starts, 1);
    // Beat 0 is 1 s before the music: the first timing point is at time 0 all the same.
    assert_eq!(points_of(&chart), [(0, Some(120.0))]);
}

#[test]
fn the_note_offset_is_read_and_not_applied() {
    let (late, late_report) = sim("999\n120\n0/0/0\n1/1/1\n");
    let (early, early_report) = sim("-250.5\n120\n0/0/0\n1/1/1\n");
    assert_eq!(notes_of(&late), [(500_000, 0, 0)]);
    assert_eq!(notes_of(&early), notes_of(&late));
    assert_eq!(late_report.note_offset, Some(999.0));
    assert_eq!(early_report.note_offset, Some(-250.5));
    assert_eq!(late.metadata.audio_offset_us, 0);
}

#[test]
fn a_note_before_the_music_or_beyond_ten_million_seconds_is_dropped() {
    let (chart, report) = sim(&format!("{HEAD}-1/1/1\n1/1/2\n1e300/1/3\n"));
    assert_eq!(notes_of(&chart), [(500_000, 1, 0)]);
    assert_eq!(report.notes_before_start, 1);
    assert_eq!(report.notes_out_of_range, 1);
}

#[test]
fn a_crlf_file_with_a_bom_blank_lines_and_spaces_is_read() {
    // `BanGConverter` downloads the score with CRLF line ends.
    let (chart, _) = sim("\u{feff}46\r\n120\r\n\r\n0/0/0\r\n 1 / 1 / 2 \r\n\r\n2/1/3\r\n");
    assert_eq!(notes_of(&chart), [(500_000, 1, 0), (1_000_000, 2, 0)]);
}

// ---------------------------------------------------------------------------------------------------- Simulator: hostile input

#[test]
fn a_simulator_header_that_is_not_one_is_an_error() {
    for text in [
        "",
        "\n\n",
        "abc\n120\n1/1/1\n",
        "46\n",
        "46\nabc\n1/1/1\n",
        "46\n0\n1/1/1\n",
        "46\n-120\n1/1/1\n",
        "46\nnan\n1/1/1\n",
        "46\n1e999\n1/1/1\n",
        "inf\n120\n1/1/1\n",
    ] {
        assert!(
            SimulatorDecoder::decode(text.as_bytes()).is_err(),
            "{text:?}"
        );
    }
    assert!(sim_error("46\n120\n0/0/0\n").contains("no playable note"));
    assert!(sim_error("46\n120\nhello\n").contains("no playable note"));
}

#[test]
fn bytes_that_are_not_text_are_read_lossily_and_flagged() {
    let mut data = format!("{HEAD}1/1/1\n").into_bytes();
    data.extend_from_slice(b"\xFF\xFE\x00junk\n2/1/2\n");
    let (chart, report) = SimulatorDecoder::decode_with_report(&data).unwrap();
    assert_eq!(chart.notes.len(), 2);
    assert!(report.invalid_text);
    assert!(!sim(&format!("{HEAD}1/1/1\n")).1.invalid_text);
}

#[test]
fn a_huge_simulator_file_is_refused() {
    let lines = format!("{HEAD}{}", "1/1\n".repeat(4_000_001));
    let error = SimulatorDecoder::decode(lines.as_bytes()).unwrap_err();
    assert!(error.to_string().contains("lines"), "{error}");
    let huge = vec![b'\n'; 16 * 1024 * 1024 + 1];
    let error = SimulatorDecoder::decode(&huge).unwrap_err();
    assert!(error.to_string().contains("too large"), "{error}");
}

// ---------------------------------------------------------------------------------------------------- telling a chart from another file

#[test]
fn a_bestdori_chart_is_told_by_its_name_and_its_content() {
    for name in [
        "expert.json",
        "Expert.JSON",
        "special.json",
        "1.hard.json",
        "a.b.easy.json",
        "song.bestdori",
        "Song.Bestdori.json",
        "SONG.BESTDORI.JSON",
    ] {
        assert!(is_bestdori_chart_name(name), "{name}");
    }
    for name in [
        "song.json",
        "chart.json",
        "package.json",
        "experts.json",
        "hard-expert.json",
        "expert",
        "expert.txt",
        "expert.jrox",
        ".bestdori",
        ".bestdori.json",
        "bestdori.json",
        "song-chart.json",
        "song-metadata.json",
    ] {
        assert!(!is_bestdori_chart_name(name), "{name}");
    }
    let chart = bestdori_bytes(&json!([bpm(0.0, 120.0), single(1.0, 0.0)]));
    assert!(looks_like_bestdori(&chart));
    assert!(looks_like_bestdori(
        b"\xEF\xBB\xBF \n [ {\"type\": \"Long\"}"
    ));
    assert!(looks_like_bestdori(
        b"[\n{\"beat\":1,\"type\":\"Directional\""
    ));
    for other in [
        &b""[..],
        b"[]",
        b"[ ]",
        b"[1, 2, 3]",
        b"[{\"name\": \"x\"}]",
        b"[{\"type\": \"other\"}]",
        b"[{\"type\": \"note\", \"lane\": 1}]",
        b"[[{\"type\": \"BPM\"}]]",
        b"{\"type\": \"BPM\"}",
        b"{\"name\": \"x\", \"version\": \"1.0.0\"}",
        b"{\"song\": {\"notes\": [], \"type\": \"Single\"}}",
        b"{\"objects\": [{\"type\": \"bpm\"}]}",
        b"BPM Single",
        b"[\x00{\"type\": \"BPM\"}]",
    ] {
        assert!(
            !looks_like_bestdori(other),
            "{}",
            String::from_utf8_lossy(other)
        );
    }
    // What the sniff accepts is what the decoder reads.
    assert!(BestdoriDecoder::decode(&chart).is_ok());
}

#[test]
fn a_simulator_score_is_told_by_its_first_lines() {
    assert!(looks_like_simulator(
        b"46\n131\n0/0/0\n8/1/3\n8.5/1/6\n8.75/1/6\n9.25/1/1\n"
    ));
    assert!(looks_like_simulator(
        b"\xEF\xBB\xBF46\r\n120\r\n\r\n0/0/0\r\n1/71/2/3\r\n"
    ));
    assert!(looks_like_simulator(b"46\n120\n8/1/3"));
    for other in [
        &b""[..],
        b"hello",
        b"just text\nwith lines\nand 1/2/3 later\n",
        b"46\n131\n",
        b"46\n131\n\n",
        b"46\n0\n0/0/0\n",
        b"46\n-5\n0/0/0\n",
        b"46\nnan\n0/0/0\n",
        b"1\n2\n3\n4\n",
        b"a,b,c\n1,2,3\n",
        b"46\n131\n0/0/0\nnot a note\n",
        b"46\n131\n0/0/0\n1/2\n",
        b"46\n131\n0/0/0\n1/1/1/1\n",
        b"46\n131\n0/0/0\n1/x/1\n",
        b"46\n131\n1/1/1\n8/1/3\x00\n",
        b"{\"a\": 1}",
        b"#TITLE \"x\"\n#BPM01: 120\n",
        b"2020/10/29\n12/5/1\n1/1/1\n",
        b"46\n131\nSee 1/1/1 above\n",
    ] {
        assert!(
            !looks_like_simulator(other),
            "{}",
            String::from_utf8_lossy(other)
        );
    }
    // Only the first 4 KiB are read, and the line cut at the end of them does not count.
    let long = format!("{HEAD}{}", "1/1/1\n".repeat(2000));
    assert!(looks_like_simulator(long.as_bytes()));
    let cut_late = format!("{long}1/1/1 trailing words\n");
    assert!(looks_like_simulator(cut_late.as_bytes()));
    assert!(!looks_like_simulator(
        format!("{HEAD}1/1/1 words\n").as_bytes()
    ));
    // What the sniff accepts is what the decoder reads.
    assert!(SimulatorDecoder::decode(format!("{HEAD}1/1/1\n").as_bytes()).is_ok());
}

fn folder(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rox_bandori_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_file_rule_lists_a_chart_by_name_and_content_and_nothing_else() {
    let dir = folder("rule");
    let chart = bestdori_bytes(&json!([bpm(0.0, 120.0), single(1.0, 0.0)]));
    let write = |name: &str, bytes: &[u8]| {
        std::fs::write(dir.join(name), bytes).unwrap();
        dir.join(name)
    };
    let listed = [
        write("expert.json", &chart),
        write("Song.bestdori.json", &chart),
        write("x.bestdori", &chart),
        write("ex.txt", format!("{HEAD}1/1/1\n").as_bytes()),
    ];
    for path in &listed {
        assert!(is_bandori_chart_file(path), "{}", path.display());
    }
    let not_listed = [
        // The name is right, the content is not.
        write("hard.json", b"{\"song\": {\"notes\": []}}"),
        write("normal.json", b"[1, 2]"),
        write("package.json", b"{\"name\": \"x\"}"),
        write("legacy.bestdori.json", b"{\"notes\": []}"),
        // The content is right, the name is not.
        write("song.json", &chart),
        write("chart.jrox", &chart),
        // Ordinary text.
        write("readme.txt", b"Read me\nFirst line\n"),
        write("empty.txt", b""),
        write("credits.txt", b"46\n120\nBy someone\n"),
    ];
    for path in &not_listed {
        assert!(!is_bandori_chart_file(path), "{}", path.display());
    }
    assert!(!is_bandori_chart_file(&dir.join("missing.txt")));
    assert!(!is_bandori_chart_file(&dir.join("missing.json")));
    std::fs::remove_dir_all(&dir).unwrap();
}

// ---------------------------------------------------------------------------------------------------- dispatch

#[test]
fn auto_decode_routes_a_bandori_chart_by_name_and_content() {
    use crate::formats::auto::{
        auto_decode, auto_decode_all, auto_decode_bytes, auto_decode_variant, auto_encode,
        detect::{DetectedFormat, detect, detect_content},
    };
    use std::path::Path;
    let dir = folder("auto");
    let chart = bestdori_bytes(&json!([bpm(0.0, 120.0), single(1.0, 2.0)]));
    let score = format!("{HEAD}1/1/3\n");
    let write = |name: &str, bytes: &[u8]| {
        std::fs::write(dir.join(name), bytes).unwrap();
        dir.join(name)
    };
    let named = write("Song.bestdori.json", &chart);
    let by_extension = write("other.bestdori", &chart);
    let by_difficulty = write("expert.json", &chart);
    let by_id = write("12.special.json", &chart);
    let text = write("hbd.txt", score.as_bytes());
    let object = write("hard.json", b"{\"name\": \"x\"}");
    let readme = write("readme.txt", b"Read me\n");
    let plain = write("plain.json", &chart);

    let decoded = [&named, &by_extension, &by_difficulty, &by_id, &text]
        .map(|path| auto_decode(path).unwrap());
    let titles: Vec<&str> = decoded
        .iter()
        .map(|chart| chart.metadata.title.as_str())
        .collect();
    assert_eq!(titles, ["Song", "other", "expert", "12.special", "hbd"]);
    assert!(decoded.iter().all(|chart| chart.key_count == 7));
    assert_eq!(notes_of(&decoded[0]), [(500_000, 2, 0)]);
    assert_eq!(notes_of(&decoded[4]), [(500_000, 2, 0)]);
    // Any other `.json` or `.txt` is read, or refused, as before.
    assert!(auto_decode(&object).is_err());
    assert!(auto_decode(&readme).is_err());
    assert!(auto_decode(&plain).is_err(), "a name no Bestdori chart has");
    // One chart per file.
    assert_eq!(
        auto_decode_variant(&named, 0).unwrap().notes,
        decoded[0].notes
    );
    assert_eq!(
        auto_decode_variant(&text, 0).unwrap().notes,
        decoded[4].notes
    );
    assert!(auto_decode_variant(&named, 1).is_err());
    assert!(auto_decode_variant(&text, 1).is_err());
    assert!(auto_decode_all(&named).unwrap().is_none());
    assert!(auto_decode_all(&by_difficulty).unwrap().is_none());
    assert!(auto_decode_all(&text).unwrap().is_none());
    let hinted = auto_decode_bytes("x/Song.bestdori.json", &chart).unwrap();
    let hinted_difficulty = auto_decode_bytes("x/expert.json", &chart).unwrap();
    let hinted_text = auto_decode_bytes("x/hbd.txt", score.as_bytes()).unwrap();
    assert_eq!(
        (
            hinted.metadata.title.as_str(),
            hinted_difficulty.metadata.title.as_str(),
            hinted_text.metadata.title.as_str()
        ),
        ("Song", "expert", "hbd")
    );
    assert!(auto_decode_bytes("x/readme.txt", b"Read me\n").is_err());
    assert!(auto_decode_bytes("x/hard.json", b"{\"name\": \"x\"}").is_err());
    // Decode only.
    assert!(auto_encode(&decoded[0], dir.join("out.bestdori")).is_err());
    assert!(auto_encode(&decoded[4], dir.join("out.txt")).is_err());
    std::fs::remove_dir_all(&dir).unwrap();

    // Detection: ROX claims `.bestdori` and `.bestdori.json` and nothing generic.
    assert_eq!(
        detect(Path::new("a.bestdori.json")).unwrap(),
        DetectedFormat::Bestdori
    );
    assert_eq!(
        detect(Path::new("expert.json")).unwrap(),
        DetectedFormat::Fnf
    );
    assert_eq!(detect(Path::new("a.json")).unwrap(), DetectedFormat::Fnf);
    assert!(detect(Path::new("a.txt")).is_err());
    assert_eq!(
        detect_content(Path::new("a.txt"), score.as_bytes()),
        Some(DetectedFormat::BangSimulator)
    );
    assert_eq!(
        detect_content(Path::new("expert.json"), &chart),
        Some(DetectedFormat::Bestdori)
    );
    assert_eq!(detect_content(Path::new("a.json"), &chart), None);
    assert_eq!(detect_content(Path::new("a.txt"), b"Read me"), None);
    assert_eq!(detect_content(Path::new("a.sus"), score.as_bytes()), None);
}
