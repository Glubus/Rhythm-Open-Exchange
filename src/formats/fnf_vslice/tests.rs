//! Tests of the V-Slice decoder. Every file here is built in memory (JSON by `json!`, packages with the `zip` writer); no real
//! file is used.

use std::io::Write;

use rox::audio_plan::SoundEvent;
use rox::error::RoxError;
use rox::model::{Note, TimingPoint};
use serde_json::{Value, json};

use super::fnfc::{MAX_ENTRIES, MAX_ENTRY_BYTES, MAX_TOTAL_BYTES};
use super::{FnfcPackage, VSliceDecoder, is_vslice_chart_name, vslice_metadata_name};

// ------------------------------------------------------------------------------------------------------- builders

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

fn chart(notes: &Value) -> Value {
    json!({ "version": "2.0.0", "scrollSpeed": { "default": 1.0 }, "events": [], "notes": notes })
}

fn metadata(extra: &Value) -> Value {
    let mut base = json!({
        "version": "2.2.4",
        "songName": "Test Song",
        "artist": "Someone",
        "charter": "Me",
        "timeFormat": "ms",
        "timeChanges": [ { "t": 0, "bpm": 120 } ],
        "playData": {
            "difficulties": ["easy", "hard"],
            "characters": { "player": "bf", "girlfriend": "gf", "opponent": "dad", "instrumental": "" }
        }
    });
    for (key, value) in extra.as_object().unwrap() {
        base[key] = value.clone();
    }
    base
}

fn tap(t: f64, d: i64) -> Value {
    json!({ "t": t, "d": d })
}

fn simple_chart() -> Vec<u8> {
    bytes(&chart(&json!({ "hard": [tap(1000.0, 0), tap(1500.0, 3)] })))
}

fn err_text<T>(result: Result<T, RoxError>) -> String {
    match result {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.to_string(),
    }
}

fn vocals_of(plan: &rox::audio_plan::AudioPlan) -> Vec<(&str, i64, bool)> {
    plan.events
        .iter()
        .filter(|e| e.file.starts_with("Voices"))
        .map(|e| (e.file.as_str(), e.time_us, e.optional))
        .collect()
}

// ------------------------------------------------------------------------------------------------------- names

#[test]
fn chart_names() {
    assert_eq!(is_vslice_chart_name("bopeebo-chart.json"), Some("default"));
    assert_eq!(is_vslice_chart_name("Bopeebo-CHART.JSON"), Some("default"));
    assert_eq!(
        is_vslice_chart_name("bopeebo-chart-erect.json"),
        Some("erect")
    );
    assert_eq!(
        is_vslice_chart_name("dir/Bopeebo-Chart-Pico.json"),
        Some("Pico")
    );
    assert_eq!(
        is_vslice_chart_name("a-chart-b-chart-bf.json"),
        Some("bf"),
        "the last -chart- splits"
    );
    assert_eq!(
        is_vslice_chart_name("a-chart-b-chart.json"),
        Some("default")
    );
    for other in [
        "-chart.json",
        "chart.json",
        "bopeebo-metadata.json",
        "bopeebo-chart-.json",
        "bopeebo-chart.txt",
        "song.json",
        "bopeebo-chart.fnfc",
    ] {
        assert_eq!(is_vslice_chart_name(other), None, "{other}");
    }
    assert_eq!(
        vslice_metadata_name("Bopeebo-chart.json").as_deref(),
        Some("Bopeebo-metadata.json")
    );
    assert_eq!(
        vslice_metadata_name("bopeebo-chart-erect.json").as_deref(),
        Some("bopeebo-metadata-erect.json")
    );
    assert_eq!(vslice_metadata_name("x.json"), None);
}

// ------------------------------------------------------------------------------------------------------- chart

#[test]
fn player_strumline_only_with_holds_and_counted_extras() {
    let notes = json!({ "hard": [
        { "t": 1000, "d": 0 },
        { "t": 1500.4, "d": 1, "l": 400.25 },
        { "t": 2000, "d": 2, "k": "mine", "p": [{ "n": "x", "v": 1 }] },
        { "t": 2500, "d": 3, "k": "" },
        { "t": 1000, "d": 4 },
        { "t": 1200, "d": 7, "l": 500 },
        { "t": 1300, "d": -1 },
        { "t": -5, "d": 0 },
        { "t": 1e30, "d": 0 },
    ] });
    let mut file = chart(&notes);
    file["events"] =
        json!([{ "t": 0, "e": "FocusCamera", "v": { "char": 1 } }, { "t": 5, "e": "X" }]);
    file["scrollSpeed"] = json!({ "default": 1.0, "hard": 2.2 });
    let (c, report) =
        VSliceDecoder::decode_with_report(&bytes(&file), &bytes(&metadata(&json!({}))), 0).unwrap();
    assert_eq!(c.key_count, 4);
    assert_eq!(
        c.notes,
        vec![
            Note::tap(1_000_000, 0),
            Note::hold(1_500_400, 400_250, 1),
            Note::tap(2_000_000, 2),
            Note::tap(2_500_000, 3),
        ]
    );
    assert_eq!(report.difficulty, "hard");
    assert_eq!(report.other_strumline_notes, 2);
    assert_eq!(report.invalid_data_notes, 1);
    assert_eq!(report.notes_before_zero, 1);
    assert_eq!(report.notes_out_of_range, 1);
    assert_eq!(report.note_kinds.get("mine"), Some(&1));
    assert_eq!(report.note_kinds_ignored(), 1, "the empty kind is no kind");
    assert_eq!(report.events_ignored, 2);
    // The scroll speed is reported, never an SV point.
    assert_eq!(report.scroll_speed, Some(2.2));
    assert!(c.timing_points.iter().all(TimingPoint::is_bpm));
}

#[test]
fn metadata_fields_land_in_the_chart() {
    let meta = metadata(&json!({
        "playData": {
            "ratings": { "hard": 7 }, "previewStart": 2000, "previewEnd": 12000,
            "characters": { "player": "bf", "opponent": "dad" }
        }
    }));
    let c = VSliceDecoder::decode_variant(&simple_chart(), &bytes(&meta), 0).unwrap();
    let m = &c.metadata;
    assert_eq!(
        (m.title.as_str(), m.artist.as_str(), m.creator.as_str()),
        ("Test Song", "Someone", "Me")
    );
    assert_eq!(m.difficulty_name, "Hard");
    assert_eq!(m.difficulty_value, Some(7.0));
    assert_eq!(m.audio_file, "", "the stems are in the plan, not the chart");
    assert_eq!(
        (m.audio_offset_us, m.preview_time_us, m.preview_duration_us),
        (0, 2_000_000, 10_000_000)
    );
    // Defaults when the metadata omits them.
    let bare = json!({ "version": "2.2.0", "timeChanges": [ { "t": 0, "bpm": 90 } ] });
    let c = VSliceDecoder::decode_variant(&simple_chart(), &bytes(&bare), 0).unwrap();
    assert_eq!(
        (
            c.metadata.title.as_str(),
            c.metadata.artist.as_str(),
            c.metadata.creator.as_str()
        ),
        ("Unknown", "Unknown", "")
    );
}

#[test]
fn variants_are_ordered_by_the_games_list_then_by_name() {
    let notes = json!({
        "zeta": [tap(0.0, 0)],
        "nightmare": [tap(0.0, 0)],
        "hard": [tap(0.0, 0)],
        "alpha": [tap(0.0, 1)],
        "easy": [tap(0.0, 0)],
        "erect": [tap(0.0, 0), { "t": 100, "d": 1, "l": 50 }],
        "normal": [tap(0.0, 0)],
        "opponent-only": [tap(0.0, 4)],
        "empty": [],
        "my-diff_two": [tap(0.0, 0)],
    });
    let meta = bytes(&metadata(
        &json!({ "playData": { "ratings": { "erect": 9 } } }),
    ));
    let variants = VSliceDecoder::variants(&bytes(&chart(&notes)), &meta).unwrap();
    let ids: Vec<&str> = variants.iter().map(|v| v.difficulty.as_str()).collect();
    assert_eq!(
        ids,
        [
            "easy",
            "normal",
            "hard",
            "erect",
            "nightmare",
            "alpha",
            "my-diff_two",
            "zeta"
        ]
    );
    assert!(
        variants
            .iter()
            .enumerate()
            .all(|(i, v)| v.index == i && v.key_count == 4)
    );
    let erect = &variants[3];
    assert_eq!(
        (erect.name.as_str(), erect.notes, erect.holds, erect.rating),
        ("Erect", 2, 1, Some(9.0))
    );
    assert_eq!(erect.duration_us, 150_000);
    assert_eq!(variants[6].name, "My Diff Two");
    // `decode_variant` uses the same numbering.
    let c = VSliceDecoder::decode_variant(&bytes(&chart(&notes)), &meta, 5).unwrap();
    assert_eq!(
        (c.metadata.difficulty_name.as_str(), c.notes[0].column),
        ("Alpha", 1)
    );
    assert!(
        err_text(VSliceDecoder::decode_variant(
            &bytes(&chart(&notes)),
            &meta,
            8
        ))
        .contains("8 playable")
    );
}

#[test]
fn a_chart_with_no_player_note_has_no_variant() {
    let file = bytes(&chart(&json!({ "hard": [tap(0.0, 4)], "easy": [] })));
    let meta = bytes(&metadata(&json!({})));
    assert_eq!(VSliceDecoder::variants(&file, &meta).unwrap(), []);
    assert!(VSliceDecoder::decode_variant(&file, &meta, 0).is_err());
}

#[test]
fn overlapping_and_duplicate_notes_are_dropped() {
    let notes = json!({ "hard": [
        { "t": 1000, "d": 0, "l": 1000 },
        tap(1500.0, 0),
        tap(1000.0, 0),
        tap(2000.0, 0),
        tap(1500.0, 1),
    ] });
    let (c, report) =
        VSliceDecoder::decode_with_report(&bytes(&chart(&notes)), &bytes(&metadata(&json!({}))), 0)
            .unwrap();
    assert_eq!(report.overlapping_notes_dropped, 2);
    assert_eq!(
        c.notes,
        vec![
            Note::hold(1_000_000, 1_000_000, 0),
            Note::tap(1_500_000, 1),
            Note::tap(2_000_000, 0)
        ]
    );
}

#[test]
fn tiny_holds_become_taps() {
    let notes = json!({ "hard": [{ "t": 0, "d": 0, "l": 0.0001 }, { "t": 10, "d": 1, "l": -5 }] });
    let (c, report) =
        VSliceDecoder::decode_with_report(&bytes(&chart(&notes)), &bytes(&metadata(&json!({}))), 0)
            .unwrap();
    assert_eq!(c.notes, vec![Note::tap(0, 0), Note::tap(10_000, 1)]);
    assert_eq!(report.empty_holds_as_taps, 1);
}

// ------------------------------------------------------------------------------------------------------- tempo

fn bpm_points(changes: &Value) -> Result<Vec<(i64, f32, u8)>, String> {
    let meta = bytes(&metadata(&json!({ "timeChanges": changes })));
    VSliceDecoder::decode_variant(&simple_chart(), &meta, 0)
        .map(|c| {
            c.timing_points
                .iter()
                .map(|p| match p {
                    TimingPoint::Bpm {
                        time_us,
                        bpm,
                        signature,
                    } => (*time_us, *bpm, *signature),
                    TimingPoint::Sv { .. } => panic!("no SV expected"),
                })
                .collect()
        })
        .map_err(|e| e.to_string())
}

#[test]
fn time_changes_become_bpm_points() {
    let points = bpm_points(&json!([
        { "t": 0, "bpm": 100 },
        { "t": 4000.5, "bpm": 150, "n": 3, "d": 4, "b": 7, "bt": [4, 4] },
        { "t": 8000, "n": 6 },
        { "t": 9000, "n": 6 },
        { "t": 12000, "bpm": 150, "n": 4 },
    ]))
    .unwrap();
    assert_eq!(
        points,
        vec![
            (0, 100.0, 4),
            (4_000_500, 150.0, 3),
            (8_000_000, 150.0, 6),
            (12_000_000, 150.0, 4),
        ],
        "an absent bpm inherits, an absent n is 4, a repeat is dropped"
    );
}

#[test]
fn time_changes_are_sorted_clamped_and_deduplicated_like_the_game() {
    let meta = bytes(&metadata(&json!({ "timeChanges": [
        { "t": 5000, "bpm": 200 },
        { "t": -10, "bpm": 90 },
        { "t": 0, "bpm": 100, "d": 8 },
    ] })));
    let (c, _) = VSliceDecoder::decode_with_report(&simple_chart(), &meta, 0).unwrap();
    let (_, report) = VSliceDecoder::decode_with_report(&simple_chart(), &meta, 0).unwrap();
    assert_eq!(
        c.timing_points,
        vec![
            TimingPoint::bpm(0, 100.0),
            TimingPoint::bpm(5_000_000, 200.0)
        ]
    );
    assert!(report.unsorted_time_changes);
    assert_eq!(report.negative_time_changes_clamped, 1);
    assert_eq!(
        report.same_time_changes_replaced, 1,
        "the later of two entries at 0 wins"
    );
    assert_eq!(report.non_quarter_denominators, 1);
}

#[test]
fn bad_tempo_maps_are_errors() {
    for (changes, needle) in [
        (json!([]), "empty"),
        (json!([{ "t": 100, "bpm": 120 }]), "not 0"),
        (json!([{ "t": 0 }]), "no bpm"),
        (json!([{ "t": 0, "bpm": 0 }]), "bpm 0"),
        (json!([{ "t": 0, "bpm": -3 }]), "bpm -3"),
        (
            json!([{ "t": 0, "bpm": 120 }, { "t": 5, "bpm": 1e9 }]),
            "bpm",
        ),
        (json!([{ "t": 0, "bpm": 120, "n": 0 }]), "numerator"),
        (json!([{ "t": 0, "bpm": 120, "n": 300 }]), "numerator"),
        (json!([{ "t": 1e30, "bpm": 120 }]), "out of range"),
    ] {
        let error = bpm_points(&changes).unwrap_err();
        assert!(error.contains(needle), "{changes}: {error}");
    }
    let missing = bytes(&json!({ "version": "2.2.0", "songName": "x" }));
    assert!(err_text(VSliceDecoder::variants(&simple_chart(), &missing)).contains("empty"));
}

// ------------------------------------------------------------------------------------------------------- versions

#[test]
fn versions_and_time_format_are_checked() {
    let good_meta = bytes(&metadata(&json!({})));
    for version in ["2.0.0", "2.0.9", "2.0.0-rc.1"] {
        let mut file = chart(&json!({ "hard": [tap(0.0, 0)] }));
        file["version"] = json!(version);
        assert!(
            VSliceDecoder::variants(&bytes(&file), &good_meta).is_ok(),
            "{version}"
        );
    }
    for version in ["1.0.0", "2.1.0", "3.0.0", "2.0", "x", ""] {
        let mut file = chart(&json!({ "hard": [tap(0.0, 0)] }));
        file["version"] = json!(version);
        let error = err_text(VSliceDecoder::variants(&bytes(&file), &good_meta));
        assert!(
            error.contains("chart version") && error.contains(version),
            "{error}"
        );
    }
    let mut file = chart(&json!({}));
    file.as_object_mut().unwrap().remove("version");
    assert!(err_text(VSliceDecoder::variants(&bytes(&file), &good_meta)).contains("no `version`"));

    for version in ["2.1.0", "2.3.0", "1.0.0"] {
        let meta = bytes(&metadata(&json!({ "version": version })));
        let error = err_text(VSliceDecoder::variants(&simple_chart(), &meta));
        assert!(error.contains("metadata version"), "{error}");
    }
    for format in ["ticks", "float", "ms2"] {
        let meta = bytes(&metadata(&json!({ "timeFormat": format })));
        let error = err_text(VSliceDecoder::variants(&simple_chart(), &meta));
        assert!(
            error.contains(format) && error.contains("timeFormat"),
            "{error}"
        );
    }
    // No timeFormat at all is `ms`.
    let mut meta = metadata(&json!({}));
    meta.as_object_mut().unwrap().remove("timeFormat");
    assert!(VSliceDecoder::variants(&simple_chart(), &bytes(&meta)).is_ok());
}

#[test]
fn hostile_json_is_an_error_not_a_panic() {
    let meta = bytes(&metadata(&json!({})));
    let hostile: Vec<Vec<u8>> = vec![
        b"".to_vec(),
        b"[]".to_vec(),
        b"null".to_vec(),
        b"{".to_vec(),
        br#"{"version":"2.0.0","notes":[]}"#.to_vec(),
        br#"{"version":"2.0.0","notes":{"hard":{"t":1}}}"#.to_vec(),
        br#"{"version":"2.0.0","notes":{"hard":[{"t":"x","d":0}]}}"#.to_vec(),
        br#"{"version":"2.0.0","notes":{"hard":[{"t":1,"d":1e30}]}}"#.to_vec(),
        br#"{"version":"2.0.0","notes":{"hard":[{"t":1,"d":0,"l":"a"}]}}"#.to_vec(),
        br#"{"version":"2.0.0","scrollSpeed":{"hard":"fast"},"notes":{}}"#.to_vec(),
        b"{\"version\":\"2.0.0\",\"notes\":{\"hard\":[{\"t\":1,\"d\":0}]}} trailing".to_vec(),
        vec![b'['; 100_000],
        vec![0xff, 0xfe, 0x00],
    ];
    for file in &hostile {
        assert!(
            VSliceDecoder::variants(file, &meta).is_err(),
            "{:?}",
            String::from_utf8_lossy(&file[..file.len().min(40)])
        );
        let _ = VSliceDecoder::variants(&simple_chart(), file);
    }
    for file in [
        b"{}".as_slice(),
        b"[1]",
        br#"{"version":2}"#,
        br#"{"version":"2.2.0","timeChanges":[{"t":0}]}"#,
        br#"{"version":"2.2.0","timeChanges":"x"}"#,
        br#"{"version":"2.2.0","timeChanges":[{"t":0,"bpm":120}],"offsets":[]}"#,
    ] {
        assert!(VSliceDecoder::variants(&simple_chart(), file).is_err());
    }
    // A file above the size limit is refused before parsing.
    let huge = vec![b' '; super::data::MAX_JSON_BYTES + 1];
    assert!(err_text(VSliceDecoder::variants(&huge, &meta)).contains("bytes"));
}

#[test]
fn infinite_looking_numbers_do_not_overflow() {
    // 1.7e308 parses as a finite f64; it must be refused as out of range, never wrapped.
    let notes = json!({ "hard": [tap(1.7e308, 0), { "t": 5, "d": 0, "l": 1.7e308 }, tap(7.0, 1)] });
    let (c, report) =
        VSliceDecoder::decode_with_report(&bytes(&chart(&notes)), &bytes(&metadata(&json!({}))), 0)
            .unwrap();
    assert_eq!(c.notes, vec![Note::tap(7000, 1)]);
    assert_eq!(report.notes_out_of_range, 2);
}

// ------------------------------------------------------------------------------------------------------- audio plan

fn plan_of(meta: &Value, variation: &str) -> rox::audio_plan::AudioPlan {
    VSliceDecoder::decode_with_audio_plan(&simple_chart(), &bytes(meta), 0, variation)
        .unwrap()
        .2
}

#[test]
fn default_plan_has_the_instrumental_and_optional_vocals() {
    let plan = plan_of(&metadata(&json!({})), "default");
    assert_eq!(
        plan.events,
        vec![
            SoundEvent::whole(0, "Inst.ogg"),
            SoundEvent::optional_whole(0, "Voices-bf.ogg"),
            SoundEvent::optional_whole(0, "Voices-dad.ogg"),
        ]
    );
    assert!(!plan.events[0].optional && plan.events[1..].iter().all(|e| e.optional));
    assert!(
        plan.events
            .iter()
            .all(|e| e.from_us == 0 && e.len_us.is_none() && e.index.is_none())
    );
    // The empty variation is the default one.
    assert_eq!(plan_of(&metadata(&json!({})), ""), plan);
}

#[test]
fn variation_and_instrumental_names() {
    let meta = metadata(&json!({ "playData": { "characters": {
        "player": "bf", "opponent": "pico", "instrumental": "erect",
    } } }));
    let plan = plan_of(&meta, "erect");
    assert_eq!(
        plan.files(),
        [
            "Inst-erect.ogg",
            "Voices-bf-erect.ogg",
            "Voices-pico-erect.ogg"
        ]
    );
}

#[test]
fn vocal_lists_replace_the_characters() {
    let meta = metadata(&json!({ "playData": { "characters": {
        "player": "bf", "opponent": "dad",
        "playerVocals": ["bf", "bf-car"], "opponentVocals": ["dad", "bf"],
    } } }));
    let plan = plan_of(&meta, "default");
    assert_eq!(
        plan.files(),
        [
            "Inst.ogg",
            "Voices-bf-car.ogg",
            "Voices-bf.ogg",
            "Voices-dad.ogg"
        ],
        "a file named twice is one event"
    );
}

#[test]
fn offsets_delay_the_stems_by_their_sign() {
    let meta = metadata(&json!({ "offsets": {
        "instrumental": 250.5, "vocals": { "bf": 10, "dad": -40.25, "nobody": 99 },
        "altInstrumentals": { "erect": 7000 }, "altVocals": { "erect": { "bf": 5 } },
    } }));
    let plan = plan_of(&meta, "default");
    assert_eq!(
        plan.events
            .iter()
            .map(|e| (e.file.as_str(), e.time_us))
            .collect::<Vec<_>>(),
        [
            ("Voices-dad.ogg", 210_250),
            ("Inst.ogg", 250_500),
            ("Voices-bf.ogg", 260_500),
        ],
        "vocals sit on top of the instrumental offset"
    );
    // The chart's own times do not move.
    let (c, _, _) =
        VSliceDecoder::decode_with_audio_plan(&simple_chart(), &bytes(&meta), 0, "default")
            .unwrap();
    assert_eq!(c.notes[0].time_us, 1_000_000);
    assert_eq!(c.metadata.audio_offset_us, 0);

    // A negative instrumental offset starts the stem before the chart's 0.
    let early = metadata(&json!({ "offsets": { "instrumental": -5000 } }));
    assert_eq!(plan_of(&early, "default").events[0].time_us, -5_000_000);

    // An alternate instrumental reads its own offsets (`getInstrumentalOffset(id)` / `getVocalOffset(char, id)`).
    let alt = metadata(&json!({
        "playData": { "characters": { "player": "bf", "opponent": "dad", "instrumental": "erect" } },
        "offsets": {
            "instrumental": 1, "vocals": { "bf": 50 },
            "altInstrumentals": { "erect": 7000 }, "altVocals": { "erect": { "bf": 5 } },
        },
    }));
    let plan = plan_of(&alt, "default");
    assert_eq!(
        plan.events
            .iter()
            .map(|e| (e.file.as_str(), e.time_us))
            .collect::<Vec<_>>(),
        [
            ("Inst-erect.ogg", 7_000_000),
            ("Voices-dad.ogg", 7_000_000),
            ("Voices-bf.ogg", 7_005_000)
        ]
    );
    // An alt instrumental with no offset of its own falls back to `instrumental`.
    let fallback = metadata(&json!({
        "playData": { "characters": { "player": "bf", "opponent": "dad", "instrumental": "x" } },
        "offsets": { "instrumental": 30 },
    }));
    assert_eq!(plan_of(&fallback, "default").events[0].time_us, 30_000);

    let wild = metadata(&json!({ "offsets": { "instrumental": 1e30 } }));
    let error = err_text(VSliceDecoder::decode_with_audio_plan(
        &simple_chart(),
        &bytes(&wild),
        0,
        "default",
    ));
    assert!(error.contains("instrumental offset"), "{error}");
}

#[test]
fn unsafe_names_are_refused() {
    let meta = metadata(&json!({ "playData": { "characters": {
        "player": "../bf", "opponent": "a\\b",
        "playerVocals": ["ok", "x/y", ""],
    } } }));
    let (_, report, plan) =
        VSliceDecoder::decode_with_audio_plan(&simple_chart(), &bytes(&meta), 0, "default")
            .unwrap();
    assert_eq!(plan.files(), ["Inst.ogg", "Voices-ok.ogg"]);
    assert_eq!(report.unsafe_names, 3, "x/y, the empty id and the opponent");

    let bad_inst =
        metadata(&json!({ "playData": { "characters": { "instrumental": "../../etc" } } }));
    let error = err_text(VSliceDecoder::decode_with_audio_plan(
        &simple_chart(),
        &bytes(&bad_inst),
        0,
        "default",
    ));
    assert!(error.contains("file-name part"), "{error}");
    for variation in ["../x", "a/b", "a\\b"] {
        assert!(
            VSliceDecoder::decode_with_audio_plan(
                &simple_chart(),
                &bytes(&metadata(&json!({}))),
                0,
                variation
            )
            .is_err()
        );
    }
    // Every event file name is relative and has no `..`.
    for event in &plan.events {
        assert!(!event.file.starts_with('/') && !event.file.contains(".."));
    }
}

#[test]
fn plan_in_a_folder_follows_the_games_search() {
    let meta = bytes(&metadata(&json!({ "playData": { "characters": {
        "player": "bf-car", "opponent": "dad-pixel", "instrumental": "",
    } } })));
    let run = |files: &[&str], variation: &str| {
        let present = |name: &str| files.contains(&name);
        let (_, _, plan) = VSliceDecoder::decode_with_audio_plan_in(
            &simple_chart(),
            &meta,
            0,
            variation,
            &present,
        )
        .unwrap();
        vocals_of(&plan)
            .iter()
            .map(|v| v.0.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        run(&["Voices-bf-car.ogg", "Voices-dad.ogg"], "default"),
        ["Voices-bf-car.ogg", "Voices-dad.ogg"]
    );
    assert_eq!(
        run(&["Voices-bf.ogg", "Voices-dad.ogg"], "default"),
        ["Voices-bf.ogg", "Voices-dad.ogg"],
        "bf-car shortens to bf"
    );
    assert_eq!(run(&["Voices-bf.ogg"], "default"), ["Voices-bf.ogg"]);
    assert_eq!(
        run(&["Voices.ogg"], "default"),
        ["Voices.ogg"],
        "the plain file is the last resort"
    );
    assert_eq!(
        run(&["Voices.ogg", "Voices-bf.ogg"], "default"),
        ["Voices-bf.ogg"]
    );
    assert_eq!(
        run(&["Voices-bf-erect.ogg", "Voices-bf.ogg"], "erect"),
        ["Voices-bf-erect.ogg"]
    );
    assert_eq!(
        run(&["Voices-bf.ogg"], "erect"),
        Vec::<String>::new(),
        "the game's second pass keeps the suffix while shortening `bf-car`, so `Voices-bf.ogg` is not found (Song.hx quirk)"
    );
    assert_eq!(run(&["Voices-erect.ogg"], "erect"), ["Voices-erect.ogg"]);
    assert_eq!(run(&[], "default"), Vec::<String>::new());

    // The listed ids are used only when every one of them exists.
    let listed = bytes(&metadata(&json!({ "playData": { "characters": {
        "player": "bf", "opponent": "dad", "playerVocals": ["bf", "bf-alt"],
    } } })));
    let run = |files: &[&str]| {
        let present = |name: &str| files.contains(&name);
        let (_, _, plan) = VSliceDecoder::decode_with_audio_plan_in(
            &simple_chart(),
            &listed,
            0,
            "default",
            &present,
        )
        .unwrap();
        vocals_of(&plan)
            .iter()
            .map(|v| v.0.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        run(&["Voices-bf.ogg", "Voices-bf-alt.ogg"]),
        ["Voices-bf.ogg", "Voices-bf-alt.ogg"]
    );
    assert_eq!(run(&["Voices-bf.ogg"]), ["Voices-bf.ogg"]);
}

// ------------------------------------------------------------------------------------------------------- .fnfc

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, data) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn package(extra: &[(&str, &[u8])]) -> Vec<u8> {
    let manifest = bytes(&json!({ "version": "1.0.0", "songId": "song" }));
    let chart_default = bytes(&chart(
        &json!({ "easy": [tap(0.0, 0)], "hard": [tap(1000.0, 0), tap(1500.0, 2)] }),
    ));
    let meta_default = bytes(&metadata(&json!({ "offsets": { "instrumental": 100 } })));
    let chart_erect = bytes(&chart(&json!({ "erect": [tap(500.0, 3)] })));
    let meta_erect = bytes(&metadata(
        &json!({ "playData": { "characters": { "player": "bf", "opponent": "dad", "instrumental": "erect" } } }),
    ));
    let mut entries: Vec<(&str, &[u8])> = vec![
        ("manifest.json", &manifest),
        ("song-metadata.json", &meta_default),
        ("song-chart.json", &chart_default),
        ("Inst.ogg", b"inst"),
        ("Voices-bf.ogg", b"bf voices"),
        ("song-metadata-erect.json", &meta_erect),
        ("song-chart-erect.json", &chart_erect),
        ("Inst-erect.ogg", b"inst erect"),
    ];
    entries.extend_from_slice(extra);
    zip_of(&entries)
}

#[test]
fn fnfc_open_lists_entries_in_archive_order() {
    let mut package = FnfcPackage::open(package(&[])).unwrap();
    assert_eq!(package.song_id(), "song");
    assert_eq!(
        package.entries(),
        [
            "manifest.json",
            "song-metadata.json",
            "song-chart.json",
            "Inst.ogg",
            "Voices-bf.ogg",
            "song-metadata-erect.json",
            "song-chart-erect.json",
            "Inst-erect.ogg"
        ]
    );
    assert_eq!(package.entry_index("Inst.ogg"), Some(3));
    assert_eq!(package.entry_index("inst.ogg"), None, "case matters");
    assert_eq!(package.variations(), ["default", "erect"]);
    assert_eq!(package.read_entry(4).unwrap(), b"bf voices");
    assert!(package.read_entry(99).is_err());
    assert!(package.chart("default").is_ok() && package.chart("").is_ok());
    assert!(package.metadata("erect").is_ok());
    assert!(err_text(package.chart("pico")).contains("song-chart-pico.json"));
}

#[test]
fn fnfc_decodes_variants_and_plans_with_entry_indexes() {
    let data = package(&[]);
    let variants = VSliceDecoder::fnfc_variants(&data, "default").unwrap();
    assert_eq!(
        variants
            .iter()
            .map(|v| v.difficulty.as_str())
            .collect::<Vec<_>>(),
        ["easy", "hard"]
    );
    let c = VSliceDecoder::decode_fnfc_variant(&data, "default", 1).unwrap();
    assert_eq!(
        (c.notes.len(), c.metadata.difficulty_name.as_str()),
        (2, "Hard")
    );
    let c = VSliceDecoder::decode_fnfc_variant(&data, "erect", 0).unwrap();
    assert_eq!((c.notes[0].column, c.notes[0].time_us), (3, 500_000));
    assert!(VSliceDecoder::decode_fnfc_variant(&data, "pico", 0).is_err());

    let (_, _, plan) =
        VSliceDecoder::decode_fnfc_with_audio_plan(&data, "mysong.fnfc", "default", 0).unwrap();
    assert_eq!(
        plan.events,
        vec![
            SoundEvent {
                optional: false,
                ..SoundEvent::in_container(100_000, "mysong.fnfc", 3)
            },
            SoundEvent {
                optional: true,
                ..SoundEvent::in_container(100_000, "mysong.fnfc", 4)
            },
        ],
        "Voices-dad.ogg is absent: skipped, not an error"
    );
    let (_, _, plan) =
        VSliceDecoder::decode_fnfc_with_audio_plan(&data, "mysong.fnfc", "erect", 0).unwrap();
    assert_eq!(plan.events.len(), 2);
    assert_eq!((plan.events[0].index, plan.events[0].time_us), (Some(7), 0));
    // `Voices-bf-erect.ogg` is absent: the game then looks for `Voices-bf.ogg` (entry 4).
    assert_eq!(
        (plan.events[1].index, plan.events[1].optional),
        (Some(4), true)
    );

    assert!(VSliceDecoder::decode_fnfc_with_audio_plan(&data, "../x.fnfc", "default", 0).is_err());
    assert!(VSliceDecoder::decode_fnfc_with_audio_plan(&data, "/x.fnfc", "default", 0).is_err());
}

#[test]
fn fnfc_without_an_instrumental_has_no_plan_but_decodes() {
    let manifest = bytes(&json!({ "songId": "s" }));
    let data = zip_of(&[
        ("manifest.json", &manifest),
        ("s-chart.json", &simple_chart()),
        ("s-metadata.json", &bytes(&metadata(&json!({})))),
    ]);
    assert!(VSliceDecoder::decode_fnfc_variant(&data, "default", 0).is_ok());
    let error = err_text(VSliceDecoder::decode_fnfc_with_audio_plan(
        &data, "s.fnfc", "default", 0,
    ));
    assert!(error.contains("no Inst.ogg"), "{error}");
}

#[test]
fn fnfc_structural_errors() {
    assert!(FnfcPackage::open(Vec::new()).is_err());
    assert!(FnfcPackage::open(b"PK\x03\x04 not really".to_vec()).is_err());
    assert!(FnfcPackage::open(vec![0u8; 4096]).is_err());
    let data = zip_of(&[("a.txt", b"x")]);
    assert!(err_text(FnfcPackage::open(data)).contains("manifest.json"));
    for manifest in [
        json!({}),
        json!({ "songId": "" }),
        json!({ "songId": "../x" }),
        json!({ "songId": "a/b" }),
        json!({ "songId": "a\\b" }),
    ] {
        let data = zip_of(&[("manifest.json", &bytes(&manifest))]);
        assert!(
            err_text(FnfcPackage::open(data)).contains("songId"),
            "{manifest}"
        );
    }
    let data = zip_of(&[("manifest.json", b"not json")]);
    assert!(FnfcPackage::open(data).is_err());
    // A manifest that is a directory-like or missing piece.
    let manifest = bytes(&json!({ "songId": "s" }));
    let data = zip_of(&[
        ("manifest.json", &manifest),
        ("s-chart.json", &simple_chart()),
    ]);
    let mut package = FnfcPackage::open(data).unwrap();
    assert!(err_text(package.metadata("default")).contains("s-metadata.json"));
}

#[test]
fn fnfc_unsafe_entry_names_refuse_the_package() {
    let manifest = bytes(&json!({ "songId": "s" }));
    for name in [
        "../evil.ogg",
        "/abs.ogg",
        "a/../../b.ogg",
        "C:/x.ogg",
        "a\\b.ogg",
        "ok/../../x",
    ] {
        let data = zip_of(&[("manifest.json", &manifest), (name, b"x")]);
        let error = err_text(FnfcPackage::open(data));
        assert!(error.contains("unsafe name"), "{name}: {error}");
    }
    // Sub-folders are fine, they are simply never looked up.
    let data = zip_of(&[
        ("manifest.json", &manifest),
        ("__MACOSX/._s", b"x"),
        ("dir/", b""),
    ]);
    let package = FnfcPackage::open(data).unwrap();
    assert_eq!(package.entries().len(), 3);
}

#[test]
fn fnfc_limits_entry_count() {
    let manifest = bytes(&json!({ "songId": "s" }));
    let names: Vec<String> = (0..MAX_ENTRIES).map(|i| format!("f{i}")).collect();
    let mut entries: Vec<(&str, &[u8])> = vec![("manifest.json", &manifest)];
    for name in names.iter().take(MAX_ENTRIES - 1) {
        entries.push((name, b""));
    }
    assert_eq!(entries.len(), MAX_ENTRIES);
    assert!(
        FnfcPackage::open(zip_of(&entries)).is_ok(),
        "exactly the limit is fine"
    );
    entries.push((&names[MAX_ENTRIES - 1], b""));
    let error = err_text(FnfcPackage::open(zip_of(&entries)));
    assert!(error.contains("entries"), "{error}");
}

/// Sets the declared uncompressed size of the first central-directory record to `declared`.
fn lie_about_size(mut zip: Vec<u8>, entry: usize, declared: u32) -> Vec<u8> {
    let signature = [0x50, 0x4b, 0x01, 0x02];
    let mut at = 0;
    let mut seen = 0;
    while let Some(found) = zip[at..].windows(4).position(|w| w == signature) {
        at += found;
        if seen == entry {
            zip[at + 24..at + 28].copy_from_slice(&declared.to_le_bytes());
            return zip;
        }
        seen += 1;
        at += 4;
    }
    panic!("no central directory record {entry}");
}

#[test]
fn fnfc_refuses_declared_zip_bombs() {
    let manifest = bytes(&json!({ "songId": "s" }));
    let base = zip_of(&[
        ("manifest.json", &manifest),
        ("s-chart.json", b"{}"),
        ("big.ogg", &[7u8; 64]),
    ]);
    // Total declared size above the limit.
    let total = u32::try_from(MAX_TOTAL_BYTES - 1).unwrap();
    let bomb = lie_about_size(base.clone(), 2, total);
    let error = err_text(FnfcPackage::open(bomb));
    assert!(error.contains("unpacks to"), "{error}");
    // One entry above the per-entry limit (but the total is fine): `read_entry` refuses it.
    let declared = u32::try_from(MAX_ENTRY_BYTES + 1).unwrap();
    let bomb = lie_about_size(base.clone(), 2, declared);
    match FnfcPackage::open(bomb) {
        Ok(mut package) => assert!(err_text(package.read_entry(2)).contains("allowed")),
        Err(e) => assert!(e.to_string().contains("fnfc"), "{e}"),
    }
    // A JSON entry above the JSON limit is refused by its declared size.
    let declared = u32::try_from(super::data::MAX_JSON_BYTES + 1).unwrap();
    let bomb = lie_about_size(base, 1, declared);
    if let Ok(mut package) = FnfcPackage::open(bomb) {
        assert!(package.chart("default").is_err());
    }
}

#[test]
fn fnfc_refuses_a_real_inflating_bomb() {
    // 70 MiB of zeros deflate to ~70 KiB: a real bomb for the JSON limit.
    let manifest = bytes(&json!({ "songId": "s" }));
    let zeros = vec![0u8; super::data::MAX_JSON_BYTES + 1024 * 1024];
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer.start_file("manifest.json", deflated).unwrap();
    writer.write_all(&manifest).unwrap();
    writer.start_file("s-chart.json", deflated).unwrap();
    writer.write_all(&zeros).unwrap();
    let data = writer.finish().unwrap().into_inner();
    assert!(
        data.len() < 1_000_000,
        "it really is a bomb: {} bytes",
        data.len()
    );
    let mut package = FnfcPackage::open(data.clone()).unwrap();
    assert!(err_text(package.chart("default")).contains("allowed"));
    // The declared size lies low: the read still stops at the limit and fails (CRC / size mismatch), never allocating the bomb.
    let lie = lie_about_size(data, 1, 10);
    if let Ok(mut package) = FnfcPackage::open(lie) {
        assert!(package.chart("default").is_err());
    }
}

// ------------------------------------------------------------------------------------------------------- auto

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("rox-vslice-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, data: &[u8]) -> std::path::PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, data).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn auto_decode_routes_vslice_charts_and_packages() {
    use crate::formats::auto::{
        auto_decode, auto_decode_bytes, auto_decode_variant,
        detect::{DetectedFormat, detect},
    };
    let dir = TempDir::new("auto");
    let chart_path = dir.write(
        "song-chart.json",
        &bytes(&chart(
            &json!({ "easy": [tap(0.0, 0)], "hard": [tap(1000.0, 1)] }),
        )),
    );
    // No metadata yet: a clear error naming the file.
    let error = err_text(auto_decode_variant(&chart_path, 0));
    assert!(
        error.contains("song-metadata.json") && error.contains("V-Slice"),
        "{error}"
    );
    let error = err_text(auto_decode(&chart_path));
    assert!(error.contains("song-metadata.json"), "{error}");

    dir.write("song-metadata.json", &bytes(&metadata(&json!({}))));
    let c = auto_decode_variant(&chart_path, 1).unwrap();
    assert_eq!(
        (c.metadata.difficulty_name.as_str(), c.notes[0].column),
        ("Hard", 1)
    );
    assert_eq!(
        auto_decode(&chart_path).unwrap().metadata.difficulty_name,
        "Easy"
    );
    assert!(auto_decode_variant(&chart_path, 2).is_err());

    // A variation reads its own metadata.
    let erect = dir.write(
        "song-chart-erect.json",
        &bytes(&chart(&json!({ "erect": [tap(0.0, 2)] }))),
    );
    assert!(err_text(auto_decode_variant(&erect, 0)).contains("song-metadata-erect.json"));
    dir.write(
        "song-metadata-erect.json",
        &bytes(&metadata(&json!({ "songName": "Erect Song" }))),
    );
    assert_eq!(
        auto_decode_variant(&erect, 0).unwrap().metadata.title,
        "Erect Song"
    );

    // A package, by extension; a single default-variation chart per index.
    assert_eq!(
        detect(std::path::Path::new("x.FNFC")).unwrap(),
        DetectedFormat::Fnfc
    );
    let fnfc = dir.write("song.fnfc", &package(&[]));
    assert_eq!(auto_decode(&fnfc).unwrap().metadata.difficulty_name, "Easy");
    assert_eq!(
        auto_decode_variant(&fnfc, 1)
            .unwrap()
            .metadata
            .difficulty_name,
        "Hard"
    );
    assert_eq!(
        auto_decode_bytes("x.fnfc", &package(&[]))
            .unwrap()
            .notes
            .len(),
        1
    );
    assert!(auto_decode_variant(&fnfc, 2).is_err());
}

#[test]
fn other_json_files_are_still_read_as_before() {
    use crate::formats::auto::{auto_decode, auto_decode_variant};
    let dir = TempDir::new("legacy");
    let legacy = bytes(&json!({ "song": { "song": "L", "bpm": 120, "notes": [
        { "mustHitSection": true, "sectionNotes": [[0, 0, 0], [500, 1, 0]] }
    ] } }));
    // A plain name, and a legacy chart that happens to be named like a V-Slice one: both stay legacy.
    for name in ["song.json", "other-chart.json", "other-chart-hard.json"] {
        let path = dir.write(name, &legacy);
        assert_eq!(auto_decode(&path).unwrap().notes.len(), 2, "{name}");
        assert_eq!(
            auto_decode_variant(&path, 0).unwrap().notes.len(),
            2,
            "{name}"
        );
        assert!(auto_decode_variant(&path, 1).is_err(), "{name}");
    }
}
