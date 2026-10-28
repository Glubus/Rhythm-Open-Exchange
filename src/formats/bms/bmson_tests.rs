#![allow(clippy::cast_possible_truncation, clippy::type_complexity)]
//! bmson charts: every one is written here (or `assets/bms/example.bmson`), times worked out by hand
//! (120 BPM and 240 pulses per quarter: 240 pulses are 0.5 s).

use rox::codec::Decoder;
use rox::formats::bms::{BmsonDecoder, BmsonReport};
use rox::model::{NoteType, RoxChart, TimingPoint};

/// A chart with the given `info` extras and channels, each `(name, [(x, y, l)])`.
fn chart(info: &str, channels: &[(&str, &[(i64, i64, i64)])]) -> String {
    let channels: Vec<String> = channels
        .iter()
        .map(|(name, notes)| {
            let notes: Vec<String> = notes
                .iter()
                .map(|(x, y, l)| format!(r#"{{"x":{x},"y":{y},"l":{l},"c":false}}"#))
                .collect();
            format!(r#"{{"name":"{name}","notes":[{}]}}"#, notes.join(","))
        })
        .collect();
    format!(
        r#"{{"version":"1.0.0","info":{{"init_bpm":120{info}}},"sound_channels":[{}]}}"#,
        channels.join(",")
    )
}

fn decode(text: &str) -> RoxChart {
    BmsonDecoder::decode(text.as_bytes()).expect("decodes")
}

fn report(text: &str) -> (RoxChart, BmsonReport) {
    BmsonDecoder::decode_with_report(text.as_bytes()).expect("decodes")
}

fn notes(c: &RoxChart) -> Vec<(i64, u8, i64)> {
    c.notes
        .iter()
        .map(|n| (n.time_us, n.column, n.duration_us()))
        .collect()
}

#[test]
fn pulses_are_time_and_long_notes_are_holds() {
    let c = decode(&chart(
        "",
        &[("a.wav", &[(1, 0, 0), (2, 240, 480), (3, 960, 0)])],
    ));
    assert_eq!(
        notes(&c),
        [(0, 0, 0), (500_000, 1, 1_000_000), (2_000_000, 2, 0)]
    );
    assert!(matches!(c.notes[1].note_type, NoteType::Hold { .. }));
    assert_eq!(c.key_count, 7);
}

#[test]
fn bpm_events_and_stops() {
    let text = r#"{"version":"1.0.0","info":{"init_bpm":120},
        "bpm_events":[{"y":960,"bpm":60},{"y":0,"bpm":0},{"y":1,"bpm":-3}],
        "stop_events":[{"y":480,"duration":240},{"y":480,"duration":240}],
        "sound_channels":[{"name":"a.wav","notes":[{"x":1,"y":480},{"x":2,"y":720},{"x":3,"y":960},{"x":4,"y":1200}]}]}"#;
    let (c, report) = report(text);
    // y=480 is 1 s, at the start of the pauses (two add up to 1 s); y=720 is 1.5 s + 1 s; y=960 is 2 s + 1 s; 60 BPM then: y=1200 is one beat (1 s) later.
    assert_eq!(
        notes(&c).iter().map(|n| n.0).collect::<Vec<_>>(),
        [1_000_000, 2_500_000, 3_000_000, 4_000_000]
    );
    let bpms: Vec<(i64, f32)> = c
        .timing_points
        .iter()
        .filter_map(|t| match t {
            TimingPoint::Bpm { time_us, bpm, .. } => Some((*time_us, *bpm)),
            TimingPoint::Sv { .. } => None,
        })
        .collect();
    assert_eq!(bpms, [(0, 120.0), (3_000_000, 60.0)]);
    assert_eq!(report.ignored_bpm_events, 2);
}

#[test]
fn a_bpm_at_pulse_zero_replaces_init_bpm_and_the_last_event_of_a_pulse_wins() {
    let text = r#"{"version":"1.0.0","info":{"init_bpm":100},"bpm_events":[{"y":0,"bpm":60},{"y":0,"bpm":240}],
        "sound_channels":[{"name":"a","notes":[{"x":1,"y":240}]}]}"#;
    let c = decode(text);
    assert_eq!(notes(&c), [(250_000, 0, 0)]);
}

#[test]
fn resolution_defaults_and_negative_values() {
    let one = |res: &str| {
        decode(&format!(
            r#"{{"version":"1.0.0","info":{{"init_bpm":120{res}}},"sound_channels":[{{"name":"a","notes":[{{"x":1,"y":480}}]}}]}}"#
        ))
    };
    assert_eq!(one("").notes[0].time_us, 1_000_000);
    assert_eq!(one(r#","resolution":0"#).notes[0].time_us, 1_000_000);
    assert_eq!(one(r#","resolution":480"#).notes[0].time_us, 500_000);
    assert_eq!(one(r#","resolution":-480"#).notes[0].time_us, 500_000);
}

#[test]
fn beat_7k_puts_the_scratch_first_only_when_used() {
    let c = decode(&chart("", &[("a", &[(1, 0, 0), (7, 240, 0), (8, 480, 0)])]));
    assert_eq!(
        (
            c.key_count,
            notes(&c).iter().map(|n| n.1).collect::<Vec<_>>()
        ),
        (8, vec![1, 7, 0])
    );
    let c = decode(&chart("", &[("a", &[(1, 0, 0), (7, 240, 0)])]));
    assert_eq!(
        (
            c.key_count,
            notes(&c).iter().map(|n| n.1).collect::<Vec<_>>()
        ),
        (7, vec![0, 6])
    );
}

#[test]
fn beat_5k_drops_keys_6_and_7_and_the_default_hint_is_beat_7k() {
    let (c, r) = report(&chart(
        r#","mode_hint":"beat-5k""#,
        &[("a", &[(5, 0, 0), (6, 240, 0), (8, 480, 0)])],
    ));
    assert_eq!((c.key_count, r.notes_outside_layout), (6, 1));
    assert_eq!(notes(&c).iter().map(|n| n.1).collect::<Vec<_>>(), [5, 0]);
    let (c, r) = report(&chart(r#","mode_hint":"mystery""#, &[("a", &[(7, 0, 0)])]));
    assert_eq!((c.key_count, r.unknown_mode_hint), (7, true));
}

#[test]
fn double_play_layouts() {
    let hint = r#","mode_hint":"beat-14k""#;
    let c = decode(&chart(
        hint,
        &[(
            "a",
            &[
                (1, 0, 0),
                (8, 0, 0),
                (9, 240, 0),
                (15, 240, 0),
                (16, 480, 0),
            ],
        )],
    ));
    assert_eq!(c.key_count, 16);
    let mut got: Vec<(i64, u8)> = c.notes.iter().map(|n| (n.time_us, n.column)).collect();
    got.sort_unstable();
    assert_eq!(
        got,
        [
            (0, 0),
            (0, 1),
            (250_000 * 2, 8),
            (500_000, 14),
            (1_000_000, 15)
        ]
    );
    // no scratch at all: 14 columns, P2 key 1 follows P1 key 7
    let c = decode(&chart(hint, &[("a", &[(1, 0, 0), (9, 240, 0)])]));
    assert_eq!(
        (
            c.key_count,
            notes(&c).iter().map(|n| n.1).collect::<Vec<_>>()
        ),
        (14, vec![0, 7])
    );
    // beat-10k: 5 keys per side, scratch of side 2 at x=16
    let c = decode(&chart(
        r#","mode_hint":"beat-10k""#,
        &[(
            "a",
            &[
                (5, 0, 0),
                (9, 240, 0),
                (13, 480, 0),
                (14, 720, 0),
                (16, 960, 0),
            ],
        )],
    ));
    assert_eq!(c.key_count, 12);
    assert_eq!(
        notes(&c).iter().map(|n| n.1).collect::<Vec<_>>(),
        [5, 6, 10, 11]
    );
}

#[test]
fn flat_layouts() {
    let c = decode(&chart(
        r#","mode_hint":"popn-9k""#,
        &[("a", &[(1, 0, 0), (9, 240, 0), (10, 480, 0)])],
    ));
    assert_eq!(
        (
            c.key_count,
            notes(&c).iter().map(|n| n.1).collect::<Vec<_>>()
        ),
        (9, vec![0, 8])
    );
    assert_eq!(
        decode(&chart(r#","mode_hint":"popn-5k""#, &[("a", &[(5, 0, 0)])])).key_count,
        5
    );
    assert_eq!(
        decode(&chart(
            r#","mode_hint":"generic-6keys""#,
            &[("a", &[(6, 0, 0)])]
        ))
        .key_count,
        6
    );
    assert_eq!(
        decode(&chart(
            r#","mode_hint":"keyboard-24k""#,
            &[("a", &[(26, 0, 0)])]
        ))
        .key_count,
        26
    );
}

#[test]
fn zero_null_and_missing_x_are_background_notes() {
    let text = r#"{"version":"1.0.0","info":{"init_bpm":120},"sound_channels":[
        {"name":"a","notes":[{"x":null,"y":0},{"y":240},{"x":0,"y":480},{"x":1,"y":720}]}]}"#;
    let (c, r) = report(text);
    assert_eq!(c.notes.len(), 1);
    assert_eq!((r.bgm_notes_skipped, r.single_audio), (3, false));
    assert_eq!(c.metadata.audio_file, "");
}

#[test]
fn one_lonely_background_note_is_the_audio_and_the_origin() {
    let (c, r) = report(&chart(
        "",
        &[
            ("song.ogg", &[(0, 480, 0)]),
            ("k.wav", &[(1, 0, 0), (2, 960, 0)]),
        ],
    ));
    assert!(r.single_audio);
    assert_eq!(c.metadata.audio_file, "song.ogg");
    assert_eq!(
        notes(&c).iter().map(|n| n.0).collect::<Vec<_>>(),
        [-1_000_000, 1_000_000]
    );
    assert_eq!(c.timing_points[0].time_us(), -1_000_000);
}

#[test]
fn a_background_note_sharing_a_channel_with_playable_notes_is_not_the_audio() {
    let (c, r) = report(&chart("", &[("mix.ogg", &[(0, 0, 0), (1, 240, 0)])]));
    assert!(!r.single_audio);
    assert_eq!(c.metadata.audio_file, "");
    // a background note at the pulse of a playable note of its channel is dropped, not counted
    let (_, r) = report(&chart("", &[("mix.ogg", &[(0, 240, 0), (1, 240, 0)])]));
    assert_eq!(r.bgm_notes_skipped, 0);
}

#[test]
fn keysounds_are_deduplicated_and_normalised() {
    let c = decode(&chart(
        "",
        &[
            ("a\\\\k.wav", &[(1, 0, 0)]),
            ("b.wav", &[(2, 240, 0)]),
            ("a/k.wav", &[(3, 480, 0)]),
        ],
    ));
    let files: Vec<&str> = c.hitsounds.iter().map(|h| h.file.as_str()).collect();
    assert_eq!(files, ["a/k.wav", "b.wav"]);
    assert_eq!(
        c.notes.iter().map(|n| n.hitsound_index).collect::<Vec<_>>(),
        [Some(0), Some(1), Some(0)]
    );
}

#[test]
fn layered_notes_fuse_into_the_first() {
    let (c, r) = report(&chart(
        "",
        &[("a.wav", &[(1, 0, 0)]), ("b.wav", &[(1, 0, 0)])],
    ));
    assert_eq!((c.notes.len(), r.overlapping_notes_dropped), (1, 1));
    assert_eq!(c.notes[0].hitsound_index, Some(0));
}

#[test]
fn notes_inside_a_hold_are_dropped_and_unsafe_names_refused() {
    let (c, r) = report(&chart(
        "",
        &[
            ("a.wav", &[(1, 0, 960), (1, 240, 0)]),
            ("../evil.wav", &[(2, 0, 0)]),
            ("/abs.wav", &[(3, 0, 0)]),
            ("C:/x.wav", &[(4, 0, 0)]),
        ],
    ));
    assert_eq!(
        (c.notes.len(), r.overlapping_notes_dropped, r.unsafe_names),
        (4, 1, 3)
    );
    assert_eq!(c.hitsounds.len(), 1);
}

#[test]
fn metadata_fields() {
    let (c, _) = report(&chart(
        r#","title":" T ","subtitle":"[S]","artist":"A","genre":"G","chart_name":"","level":7,"subartists":["music:M","chart: Me "],"back_image":"..\\x.png","eyecatch_image":"e.png""#,
        &[("a", &[(1, 0, 0)])],
    ));
    let m = &c.metadata;
    assert_eq!(
        (
            m.title.as_str(),
            m.artist.as_str(),
            m.creator.as_str(),
            m.difficulty_name.as_str()
        ),
        ("T", "A", "Me", "[S]")
    );
    assert_eq!(
        (
            m.difficulty_value,
            m.genre.as_deref(),
            m.background_file.as_deref()
        ),
        (Some(7.0), Some("G"), Some("e.png"))
    );
}

#[test]
fn invalid_files_are_errors() {
    let bad = |s: &str| assert!(BmsonDecoder::decode(s.as_bytes()).is_err(), "{s}");
    bad("");
    bad("not json");
    bad(r#"{"info":{"init_bpm":120},"sound_channels":[{"name":"a","notes":[{"x":1,"y":0}]}]}"#);
    bad(
        r#"{"version":"2.0.0","info":{"init_bpm":120},"sound_channels":[{"name":"a","notes":[{"x":1,"y":0}]}]}"#,
    );
    bad(r#"{"version":"1.0.0","info":{},"sound_channels":[{"name":"a","notes":[{"x":1,"y":0}]}]}"#);
    bad(
        r#"{"version":"1.0.0","info":{"init_bpm":0},"sound_channels":[{"name":"a","notes":[{"x":1,"y":0}]}]}"#,
    );
    bad(r#"{"version":"1.0.0","sound_channels":[]}"#);
    bad(
        r#"{"version":"1.0.0","info":{"init_bpm":120},"sound_channels":[{"name":"a","notes":[{"x":0,"y":0}]}]}"#,
    );
    bad(
        r#"{"version":"1.0.0","info":{"init_bpm":120,"mode_hint":"beat-5k"},"sound_channels":[{"name":"a","notes":[{"x":6,"y":0}]}]}"#,
    );
}

#[test]
fn hostile_numbers_do_not_panic() {
    let _ = BmsonDecoder::decode(br#"{"version":"1.0.0","info":{"init_bpm":1e-300},"stop_events":[{"y":0,"duration":9223372036854775807}],"sound_channels":[{"name":"a","notes":[{"x":1,"y":9223372036854775807,"l":9223372036854775807},{"x":1.5,"y":-4}]}]}"#);
}

#[test]
fn the_example_asset_decodes_as_worked_out_by_hand() {
    let data = rox_test_utils::get_test_asset("bms/example.bmson");
    let (c, r) = BmsonDecoder::decode_with_report(&data).unwrap();
    assert_eq!(
        (c.key_count, r.single_audio, r.overlapping_notes_dropped),
        (8, true, 1)
    );
    let m = &c.metadata;
    assert_eq!(
        (
            m.title.as_str(),
            m.difficulty_name.as_str(),
            m.creator.as_str(),
            m.audio_file.as_str(),
            m.background_file.as_deref()
        ),
        (
            "ROX bmson features",
            "HYPER",
            "Hand",
            "song.ogg",
            Some("bg/stage.png")
        )
    );
    // BGM at y=0: origin 0. One stop of one beat (0.5 s) at y=480 (1 s). 60 BPM from y=960 (2 s + 0.5 s stop = 2.5 s).
    let us = |s: f64| (s * 1e6).round() as i64;
    let mut expected = vec![
        (0, 1, 0),       // kick key 1; the snare on key 1 at the same pulse is layered and dropped
        (us(1.0), 0, 0), // scratch, at the start of the stop
        (us(2.0), 2, 0), // y=720: 1.5 s + 0.5 s of stop
        (us(2.5), 3, us(2.0)), // hold y=960..1440: 2 beats at 60 BPM
        (us(3.5), 4, 0), // y=1200: one beat after y=960
        (us(4.5), 7, 0), // y=1440
    ];
    expected.sort_by_key(|n| (n.0, n.1));
    let mut got = notes(&c);
    got.sort_by_key(|n| (n.0, n.1));
    assert_eq!(got, expected);
    assert_eq!(
        c.hitsounds
            .iter()
            .map(|h| h.file.as_str())
            .collect::<Vec<_>>(),
        ["kick.wav", "snare.wav"]
    );
}

#[test]
fn auto_decode_reads_bmson_and_refuses_to_write() {
    let data = rox_test_utils::get_test_asset("bms/example.bmson");
    let c = rox::formats::auto_decode_bytes("x.bmson", &data).unwrap();
    assert!(rox::formats::auto_encode(&c, std::env::temp_dir().join("rox-never.bmson")).is_err());
}

#[test]
fn the_audio_plan_slices_channels_like_beatoraja() {
    // vox.wav at 120 BPM: restart at 240 (0.5 s), continue at 360 (0.75 s, two notes), restart at 720 (1.5 s, a playable
    // and a background note: one event), continue at 840, 1200 (a background note), 1680.
    let notes: &[(i64, i64, i64)] = &[(1, 240, 0), (3, 360, 0), (7, 360, 0), (2, 720, 0), (0, 720, 0), (4, 840, 0), (3, 1200, 0), (0, 1680, 0)];
    let text = format!(
        r#"{{"version":"1.0.0","info":{{"init_bpm":120}},"sound_channels":[{{"name":"vox.wav","notes":[{}]}}]}}"#,
        notes
            .iter()
            .enumerate()
            .map(|(i, (x, y, l))| {
                // notes 2, 3 (index 1, 2), 5 (index 5), 6, 7 continue; the others restart
                let c = matches!(i, 1 | 2 | 5 | 6 | 7);
                format!(r#"{{"x":{x},"y":{y},"l":{l},"c":{c}}}"#)
            })
            .collect::<Vec<_>>()
            .join(",")
    );
    let (_, _, plan) = BmsonDecoder::decode_with_audio_plan(text.as_bytes()).unwrap();
    let got: Vec<(i64, i64, Option<i64>)> = plan.events.iter().map(|e| (e.time_us, e.from_us, e.len_us)).collect();
    // slices: 0.5 s from 0 for 0.25; 0.75 s from 0.25 for 0.75 (to the restart pulse); 1.5 s from 0 for 0.25; 1.75 s from 0.25 for 0.75;
    assert_eq!(
        got,
        [
            (500_000, 0, Some(250_000)),
            (750_000, 250_000, None),
            (1_500_000, 0, Some(250_000)),
            (1_750_000, 250_000, Some(750_000)),
            (2_500_000, 1_000_000, Some(1_000_000)),
            (3_500_000, 2_000_000, None),
        ]
    );
}

#[test]
fn the_audio_plan_ignores_stops_in_slice_lengths_and_unsafe_names() {
    let text = r#"{"version":"1.0.0","info":{"init_bpm":120},"stop_events":[{"y":240,"duration":240}],
        "sound_channels":[{"name":"a.wav","notes":[{"x":1,"y":0,"c":false},{"x":1,"y":480,"c":true}]},{"name":"../x.wav","notes":[{"x":2,"y":0}]}]}"#;
    let (chart, _, plan) = BmsonDecoder::decode_with_audio_plan(text.as_bytes()).unwrap();
    assert_eq!(plan.events.len(), 2);
    // 480 pulses = 1 s of music; the stop (0.5 s) is not part of the slice but delays the second note
    assert_eq!((plan.events[0].len_us, plan.events[1].time_us), (Some(1_000_000), 1_500_000));
    assert_eq!(chart.notes.len(), 3, "the note of the refused name is still a note, only silent");
}
