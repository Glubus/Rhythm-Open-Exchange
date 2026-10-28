//! Tests of the `O2Jam` decoder. Every file here is built in memory (headers and packages as the format notes lay them
//! out, containers with the documented scrambles applied); no real file is used.

use rox::audio_plan::SoundEvent;
use rox::codec::Decoder;
use rox::error::RoxError;
use rox::model::{Note, TimingPoint};

use super::ojm::{AccXor, REARRANGE_TABLE, rearrange};
use super::{OjmContainer, OjmKind, OjnDecoder, OjnHeader, SampleFormat};

// ---------------------------------------------------------------------------------------------------- OJN builders

const EMPTY: [u8; 4] = [0; 4];

/// A note slot: sample `value`, volume/pan byte, type.
fn note(value: i16, volume_pan: u8, note_type: u8) -> [u8; 4] {
    let v = value.to_le_bytes();
    [v[0], v[1], volume_pan, note_type]
}

fn tap(value: i16) -> [u8; 4] {
    note(value, 0, 0)
}

fn float(value: f32) -> [u8; 4] {
    value.to_le_bytes()
}

/// One package: measure, channel, slots.
fn pkg(measure: i32, channel: i16, slots: &[[u8; 4]]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&measure.to_le_bytes());
    out.extend_from_slice(&channel.to_le_bytes());
    out.extend_from_slice(&i16::try_from(slots.len()).unwrap().to_le_bytes());
    for slot in slots {
        out.extend_from_slice(slot);
    }
    out
}

/// `n` empty slots with `(index, slot)` filled.
fn grid(n: usize, filled: &[(usize, [u8; 4])]) -> Vec<[u8; 4]> {
    let mut slots = vec![EMPTY; n];
    for &(i, slot) in filled {
        slots[i] = slot;
    }
    slots
}

fn put(out: &mut [u8], at: usize, bytes: &[u8]) {
    out[at..at + bytes.len()].copy_from_slice(bytes);
}

struct Ojn {
    bpm: f32,
    levels: [i16; 3],
    title: Vec<u8>,
    artist: Vec<u8>,
    noter: Vec<u8>,
    ojm: Vec<u8>,
    genre: i32,
    sections: [Vec<u8>; 3],
}

impl Ojn {
    fn new(sections: [Vec<u8>; 3]) -> Self {
        Self {
            bpm: 120.0,
            levels: [3, 12, 30],
            title: b"Song".to_vec(),
            artist: b"Artist".to_vec(),
            noter: b"Noter".to_vec(),
            ojm: b"o2ma1.ojm".to_vec(),
            genre: 2,
            sections,
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; 300];
        put(&mut out, 0, &1234i32.to_le_bytes());
        put(&mut out, 4, b"ojn\0");
        put(&mut out, 8, &2.9f32.to_le_bytes());
        put(&mut out, 12, &self.genre.to_le_bytes());
        put(&mut out, 16, &self.bpm.to_le_bytes());
        for (i, level) in self.levels.iter().enumerate() {
            put(&mut out, 20 + 2 * i, &level.to_le_bytes());
        }
        let text = |out: &mut Vec<u8>, at: usize, size: usize, bytes: &[u8]| {
            assert!(bytes.len() <= size);
            put(out, at, bytes);
        };
        text(&mut out, 108, 64, &self.title);
        text(&mut out, 172, 32, &self.artist);
        text(&mut out, 204, 32, &self.noter);
        text(&mut out, 236, 32, &self.ojm);
        let mut offset = 300usize;
        for (i, section) in self.sections.iter().enumerate() {
            put(
                &mut out,
                284 + 4 * i,
                &i32::try_from(offset).unwrap().to_le_bytes(),
            );
            offset += section.len();
        }
        put(&mut out, 296, &i32::try_from(offset).unwrap().to_le_bytes());
        for section in &self.sections {
            out.extend_from_slice(section);
        }
        out
    }
}

fn concat(parts: &[Vec<u8>]) -> Vec<u8> {
    parts.concat()
}

fn one_section(parts: &[Vec<u8>]) -> Ojn {
    Ojn::new([Vec::new(), concat(parts), Vec::new()])
}

fn times(notes: &[Note]) -> Vec<(i64, u8)> {
    notes.iter().map(|n| (n.time_us, n.column)).collect()
}

// ------------------------------------------------------------------------------------------------------ OJN tests

#[test]
fn the_header_is_read_at_the_documented_offsets() {
    let mut ojn = one_section(&[pkg(0, 2, &[tap(1)])]);
    ojn.title = b"My Title".to_vec();
    ojn.genre = 3;
    ojn.ojm = b"o2ma77.ojm".to_vec();
    let header = OjnHeader::parse(&ojn.bytes()).unwrap();
    assert_eq!(header.song_id, 1234);
    assert_eq!(header.level, [3, 12, 30]);
    assert_eq!(header.title, "My Title");
    assert_eq!(header.artist, "Artist");
    assert_eq!(header.noter, "Noter");
    assert_eq!(header.ojm_file, "o2ma77.ojm");
    assert_eq!(header.genre_name(), "Techno");
    assert!((header.bpm - 120.0).abs() < f32::EPSILON);
    assert_eq!(header.note_offset, [300, 300, 312]);
    assert_eq!(header.cover_offset, header.note_offset[2]);
}

#[test]
fn an_unknown_genre_is_etc_and_korean_text_is_read_as_cp949() {
    let mut ojn = one_section(&[pkg(0, 2, &[tap(1)])]);
    ojn.genre = 99;
    ojn.title = vec![0xB0, 0xA1, 0xB3, 0xAA]; // 가나 in CP949
    let (chart, _) = OjnDecoder::decode_with_report(&ojn.bytes(), 0).unwrap();
    assert_eq!(chart.metadata.title, "가나");
    assert_eq!(chart.metadata.genre.as_deref(), Some("Etc"));
}

#[test]
fn notes_are_on_the_seven_columns_at_their_grid_times() {
    // 120 BPM: a beat is 500 ms. Measure 0 lane 1 at beats 0 and 2, lane 7 at beat 0, measure 1 lane 4 at beat 4+2.
    let ojn = one_section(&[
        pkg(1, 5, &grid(4, &[(2, tap(4))])),
        pkg(0, 2, &grid(4, &[(0, tap(1)), (2, tap(2))])),
        pkg(0, 8, &grid(8, &[(0, tap(3))])),
    ]);
    let chart = OjnDecoder::decode(&ojn.bytes()).unwrap();
    assert_eq!(chart.key_count, 7);
    assert_eq!(
        times(&chart.notes),
        [(0, 0), (0, 6), (1_000_000, 0), (3_000_000, 3)]
    );
    assert!(chart.notes.iter().all(|n| !n.is_hold()));
    assert_eq!(chart.timing_points, [TimingPoint::bpm(0, 120.0)]);
    assert_eq!(chart.metadata.audio_file, "");
    assert_eq!(chart.hitsounds.len(), 0);
    assert_eq!(chart.metadata.difficulty_name, "NX");
    assert_eq!(chart.metadata.difficulty_value, Some(12.0));
    assert_eq!(chart.metadata.creator, "Noter");
}

#[test]
fn a_long_note_runs_from_its_head_to_its_tail() {
    // Lane 2: head at slot 0, tail at slot 4 of 8 = beat 2 = 1 s.
    let ojn = one_section(&[pkg(
        0,
        3,
        &grid(8, &[(0, note(1, 0, 2)), (4, note(1, 0, 3))]),
    )]);
    let chart = OjnDecoder::decode(&ojn.bytes()).unwrap();
    assert_eq!(chart.notes, [Note::hold(0, 1_000_000, 1)]);
}

#[test]
fn a_bpm_event_changes_the_tempo_from_its_beat() {
    // Header 120. Measure 1 slot 0 (beat 4, 2 s): 240 BPM. A 240 at measure 2 repeats the tempo: dropped.
    // Note at measure 1 slot 2 of 4 = beat 6 = 2 s + 2 beats of 250 ms.
    let ojn = one_section(&[
        pkg(1, 1, &[float(240.0)]),
        pkg(2, 1, &[float(240.0)]),
        pkg(1, 2, &grid(4, &[(2, tap(1))])),
        pkg(0, 2, &grid(4, &[(0, tap(1))])),
    ]);
    let (chart, report) = OjnDecoder::decode_with_report(&ojn.bytes(), 0).unwrap();
    assert_eq!(times(&chart.notes), [(0, 0), (2_500_000, 0)]);
    assert_eq!(
        chart.timing_points,
        [
            TimingPoint::bpm(0, 120.0),
            TimingPoint::bpm(2_000_000, 240.0)
        ]
    );
    assert_eq!(report.repeated_bpm_events, 1);
}

#[test]
fn a_bpm_event_at_beat_zero_replaces_the_header_bpm_and_zero_or_negative_ones_are_ignored() {
    let ojn = one_section(&[
        pkg(0, 1, &[float(150.0)]),
        pkg(1, 1, &[float(0.0)]),
        pkg(2, 1, &[float(-5.0)]),
        pkg(0, 2, &grid(4, &[(2, tap(1))])),
    ]);
    let (chart, report) = OjnDecoder::decode_with_report(&ojn.bytes(), 0).unwrap();
    // beat 2 at 150 BPM = 0.8 s
    assert_eq!(times(&chart.notes), [(800_000, 0)]);
    assert_eq!(chart.timing_points, [TimingPoint::bpm(0, 150.0)]);
    assert_eq!(report.ignored_bpm_events, 1); // the 0 slot is an empty slot, only the negative one counts
}

#[test]
fn an_unusable_header_bpm_takes_the_first_bpm_event() {
    let mut ojn = one_section(&[
        pkg(0, 1, &[float(100.0)]),
        pkg(0, 2, &grid(2, &[(1, tap(1))])),
    ]);
    ojn.bpm = 0.0;
    let (chart, report) = OjnDecoder::decode_with_report(&ojn.bytes(), 0).unwrap();
    assert!(report.header_bpm_unusable);
    assert_eq!(times(&chart.notes), [(1_200_000, 0)]); // beat 2 at 100 BPM
    // with no event either: an error
    let mut none = one_section(&[pkg(0, 2, &[tap(1)])]);
    none.bpm = f32::NAN;
    assert!(OjnDecoder::decode(&none.bytes()).is_err());
}

#[test]
fn a_measure_fraction_shortens_the_measure_without_compressing_its_notes() {
    // Measure 0 is 0.75 of 4 beats = 3 beats; its slots stay on a 4-beat grid (slot 2 of 4 = beat 2 = 1 s); measure 1
    // starts at beat 3 = 1.5 s. A BPM change at measure 1 slot 0 is at 1.5 s too.
    let ojn = one_section(&[
        pkg(0, 0, &[float(0.75)]),
        pkg(0, 2, &grid(4, &[(2, tap(1)), (3, tap(2))])),
        pkg(1, 4, &grid(4, &[(0, tap(3))])),
        pkg(1, 1, &[float(240.0)]),
        pkg(1, 3, &grid(4, &[(2, tap(4))])),
    ]);
    let chart = OjnDecoder::decode(&ojn.bytes()).unwrap();
    assert_eq!(
        times(&chart.notes),
        [
            (1_000_000, 0),
            (1_500_000, 0),
            (1_500_000, 2),
            (2_000_000, 1)
        ]
    );
}

#[test]
fn a_measure_fraction_that_is_not_positive_is_ignored() {
    let ojn = one_section(&[
        pkg(0, 0, &[float(-1.0)]),
        pkg(1, 2, &grid(4, &[(0, tap(1))])),
    ]);
    let (chart, report) = OjnDecoder::decode_with_report(&ojn.bytes(), 0).unwrap();
    assert_eq!(times(&chart.notes), [(2_000_000, 0)]);
    assert_eq!(report.ignored_measure_fractions, 1);
}

#[test]
fn broken_long_notes_follow_the_o2jam_policy() {
    // Lane 3 (column 2), 16 slots of 125 ms: head 0, tap 2 (inside), head 4 (inside), tail 8, tail 10 (no head),
    // head 12 (never closed). Sample values 1..=4 on the sounding events, 9 on the tails.
    let ojn = one_section(&[pkg(
        0,
        4,
        &grid(
            16,
            &[
                (0, note(1, 0, 2)),
                (2, note(2, 0, 0)),
                (4, note(3, 0, 2)),
                (8, note(9, 0, 3)),
                (10, note(9, 0, 3)),
                (12, note(4, 0, 2)),
            ],
        ),
    )]);
    let (chart, report, plan) = OjnDecoder::decode_with_audio_plan(&ojn.bytes(), 0).unwrap();
    assert_eq!(
        chart.notes,
        [Note::hold(0, 1_000_000, 2), Note::tap(1_500_000, 2)]
    );
    assert_eq!(report.notes_inside_holds_to_bgm, 2);
    assert_eq!(report.stray_hold_ends, 1);
    assert_eq!(report.unclosed_holds_as_taps, 1);
    assert_eq!(report.lane_events, 6);
    let sounds: Vec<(i64, Option<u32>)> =
        plan.events.iter().map(|e| (e.time_us, e.index)).collect();
    assert_eq!(
        sounds,
        [
            (0, Some(0)),
            (250_000, Some(1)),
            (500_000, Some(2)),
            (1_500_000, Some(3))
        ]
    );
}

#[test]
fn a_long_note_whose_tail_is_not_after_its_head_is_a_tap() {
    // Head and tail in the same slot position (two packages): empty hold.
    let ojn = one_section(&[pkg(0, 2, &[note(1, 0, 2)]), pkg(0, 2, &[note(1, 0, 3)])]);
    let (chart, report) = OjnDecoder::decode_with_report(&ojn.bytes(), 0).unwrap();
    assert_eq!(chart.notes, [Note::tap(0, 0)]);
    assert_eq!(report.empty_holds_as_taps, 1);
}

#[test]
fn two_notes_at_one_place_keep_one_chart_note_and_both_sounds() {
    let ojn = one_section(&[pkg(0, 2, &[tap(1)]), pkg(0, 2, &[tap(2)])]);
    let (chart, report, plan) = OjnDecoder::decode_with_audio_plan(&ojn.bytes(), 0).unwrap();
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(report.overlapping_notes_dropped, 1);
    assert_eq!(plan.events.len(), 2);
}

#[test]
fn type_one_is_a_normal_note_and_counted() {
    let ojn = one_section(&[pkg(0, 2, &[note(1, 0, 1)])]);
    let (chart, report) = OjnDecoder::decode_with_report(&ojn.bytes(), 0).unwrap();
    assert_eq!(chart.notes, [Note::tap(0, 0)]);
    assert_eq!(report.type_one_notes, 1);
}

#[test]
fn the_plan_has_every_note_and_background_sound_on_the_ojm() {
    // 120 BPM. Lane 1: value 3 (id 2) at 0; type 4 (Ogg) value 2 (id 1001) at beat 2 with volume nibble 8 (50 %).
    // Background channel 9: value 5 (id 4) at beat 1 (500 ms), volume nibble 15 (94 %); channel 12: value 7 (id 6)
    // at beat 3, type 4 (id 1006); a long-note tail there (type 3) makes no sound. Volume nibble 0 = unchanged.
    let ojn = one_section(&[
        pkg(0, 2, &grid(4, &[(0, tap(3)), (2, note(2, 0x80, 4))])),
        pkg(0, 9, &grid(4, &[(1, note(5, 0xF3, 0)), (2, note(8, 0, 3))])),
        pkg(0, 12, &grid(4, &[(3, note(7, 0, 4))])),
    ]);
    let (chart, report, plan) = OjnDecoder::decode_with_audio_plan(&ojn.bytes(), 0).unwrap();
    assert_eq!(chart.notes.len(), 2);
    assert_eq!(report.background_hold_ends_removed, 1);
    assert_eq!(report.ojm_file.as_deref(), Some("o2ma1.ojm"));
    let mut expected = vec![
        SoundEvent::in_container(0, "o2ma1.ojm", 2),
        SoundEvent::in_container(500_000, "o2ma1.ojm", 4),
        SoundEvent::in_container(1_000_000, "o2ma1.ojm", 1001),
        SoundEvent::in_container(1_500_000, "o2ma1.ojm", 1006),
    ];
    expected[1].volume = Some(94);
    expected[2].volume = Some(50);
    assert_eq!(plan.events, expected);
    assert_eq!(plan.files(), ["o2ma1.ojm"]);
}

#[test]
fn the_ojm_name_is_made_safe_and_an_unsafe_one_leaves_the_plan_empty() {
    let body = [pkg(0, 2, &[tap(1)])];
    let mut ojn = one_section(&body);
    ojn.ojm = b"songs\\o2ma1.ojm".to_vec();
    let (_, _, plan) = OjnDecoder::decode_with_audio_plan(&ojn.bytes(), 0).unwrap();
    assert_eq!(plan.events[0].file, "songs/o2ma1.ojm");
    for bad in [&b"..\\evil.ojm"[..], b"/etc/x.ojm", b"C:\\x.ojm"] {
        let mut ojn = one_section(&body);
        ojn.ojm = bad.to_vec();
        let (chart, report, plan) = OjnDecoder::decode_with_audio_plan(&ojn.bytes(), 0).unwrap();
        assert_eq!(chart.notes.len(), 1);
        assert!(plan.is_empty());
        assert_eq!(report.unsafe_names, 1);
        assert_eq!(report.ojm_file, None);
    }
    let mut empty = one_section(&body);
    empty.ojm = Vec::new();
    let (_, report, plan) = OjnDecoder::decode_with_audio_plan(&empty.bytes(), 0).unwrap();
    assert!(plan.is_empty());
    assert_eq!(report.unsafe_names, 0);
}

#[test]
fn the_three_difficulties_and_the_empty_one_that_is_skipped() {
    let easy = pkg(0, 2, &[tap(1)]);
    let normal = concat(&[pkg(0, 2, &[tap(1)]), pkg(0, 3, &[tap(2)])]);
    let hard = concat(&[
        pkg(0, 2, &[tap(1)]),
        pkg(0, 3, &[tap(2)]),
        pkg(0, 4, &[tap(3)]),
    ]);
    let all = Ojn::new([easy.clone(), normal.clone(), hard.clone()]).bytes();
    let variants = OjnDecoder::variants(&all).unwrap();
    assert_eq!(variants.len(), 3);
    assert_eq!(
        variants
            .iter()
            .map(|v| (v.index, v.source_index, v.notes, v.level))
            .collect::<Vec<_>>(),
        [(0, 0, 1, 3), (1, 1, 2, 12), (2, 2, 3, 30)]
    );
    assert_eq!(variants[0].name.code(), "EX");
    assert_eq!(variants[1].name.code(), "NX");
    assert_eq!(variants[2].name.code(), "HX");
    assert!(variants.iter().all(|v| v.key_count == 7 && v.holds == 0));

    // Easy empty: the other two are variants 0 and 1, and keep their place in the file.
    let skipped = Ojn::new([Vec::new(), normal, hard]).bytes();
    let variants = OjnDecoder::variants(&skipped).unwrap();
    assert_eq!(
        variants
            .iter()
            .map(|v| (v.index, v.source_index, v.notes))
            .collect::<Vec<_>>(),
        [(0, 1, 2), (1, 2, 3)]
    );
    assert_eq!(OjnDecoder::decode(&skipped).unwrap().notes.len(), 2);
    let hard_chart = OjnDecoder::decode_variant(&skipped, 1).unwrap();
    assert_eq!(hard_chart.notes.len(), 3);
    assert_eq!(hard_chart.metadata.difficulty_name, "HX");
    assert_eq!(hard_chart.metadata.difficulty_value, Some(30.0));
    assert!(OjnDecoder::decode_variant(&skipped, 2).is_err());

    // A section with packages but no note (only a BPM event) is empty too.
    let tempo_only = Ojn::new([pkg(0, 1, &[float(150.0)]), Vec::new(), Vec::new()]).bytes();
    assert_eq!(OjnDecoder::variants(&tempo_only).unwrap().len(), 0);
    assert!(OjnDecoder::decode(&tempo_only).is_err());
}

#[test]
fn variants_count_long_notes_and_duration() {
    let ojn = one_section(&[
        pkg(0, 3, &grid(8, &[(0, note(1, 0, 2)), (4, note(1, 0, 3))])),
        pkg(0, 2, &[tap(1)]),
    ]);
    let variants = OjnDecoder::variants(&ojn.bytes()).unwrap();
    assert_eq!(variants.len(), 1);
    assert_eq!((variants[0].notes, variants[0].holds), (2, 1));
    assert_eq!(variants[0].duration_us, 1_000_000);
}

#[test]
fn files_that_are_not_ojn_are_errors() {
    assert!(OjnDecoder::decode(&[]).is_err());
    assert!(OjnDecoder::decode(&[0u8; 299]).is_err());
    assert!(OjnDecoder::decode(&[0u8; 400]).is_err());
    let mut encrypted = vec![0u8; 400];
    encrypted[..3].copy_from_slice(b"new");
    let error = OjnDecoder::decode(&encrypted).unwrap_err();
    assert!(matches!(error, RoxError::InvalidFormat(message) if message.contains("encrypted")));
}

// ------------------------------------------------------------------------------------------------ hostile OJN input

#[test]
fn every_prefix_of_an_ojn_is_an_error_or_a_partial_chart_never_a_panic() {
    let ojn = Ojn::new([
        concat(&[
            pkg(0, 2, &grid(8, &[(0, tap(1))])),
            pkg(0, 1, &[float(150.0)]),
        ]),
        concat(&[
            pkg(0, 3, &grid(4, &[(0, note(1, 0, 2)), (2, note(1, 0, 3))])),
            pkg(1, 9, &grid(4, &[(1, tap(2))])),
        ]),
        Vec::new(),
    ])
    .bytes();
    for len in 0..=ojn.len() {
        let _ = OjnDecoder::variants(&ojn[..len]);
        let _ = OjnDecoder::decode_with_audio_plan(&ojn[..len], 0);
    }
}

#[test]
fn a_section_cut_inside_a_package_keeps_the_packages_before_it() {
    let mut section = concat(&[
        pkg(0, 2, &grid(4, &[(0, tap(1))])),
        pkg(0, 3, &grid(4, &[(0, tap(2))])),
    ]);
    section.truncate(section.len() - 6);
    let (chart, report) =
        OjnDecoder::decode_with_report(&one_section(&[section]).bytes(), 0).unwrap();
    assert_eq!(chart.notes.len(), 1);
    assert!(report.truncated_section);
}

#[test]
fn huge_counts_and_bad_offsets_cost_nothing() {
    // A package that claims 32767 slots in a few bytes.
    let mut huge = Vec::new();
    huge.extend_from_slice(&0i32.to_le_bytes());
    huge.extend_from_slice(&2i16.to_le_bytes());
    huge.extend_from_slice(&i16::MAX.to_le_bytes());
    huge.extend_from_slice(&tap(1));
    let ojn = one_section(&[huge]).bytes();
    assert_eq!(OjnDecoder::variants(&ojn).unwrap().len(), 0);

    // Offsets that point outside the file or backwards.
    let base = one_section(&[pkg(0, 2, &[tap(1)])]).bytes();
    for bad in [i32::MIN, -1, 0, 299, 301, 100_000, i32::MAX] {
        for slot in 0..3 {
            let mut data = base.clone();
            put(&mut data, 284 + 4 * slot, &bad.to_le_bytes());
            let _ = OjnDecoder::variants(&data);
            let mut data = base.clone();
            put(&mut data, 296, &bad.to_le_bytes());
            let _ = OjnDecoder::variants(&data);
        }
    }
}

#[test]
fn events_beyond_the_range_are_counted_and_skipped() {
    let slow = one_section(&[
        pkg(0, 1, &[float(1.0e-30)]),
        pkg(0, 2, &grid(2, &[(1, tap(1))])),
        pkg(0, 3, &[tap(1)]),
        pkg(i32::MAX, 2, &[tap(1)]),
        pkg(-1, 2, &[tap(1)]),
    ]);
    let (chart, report) = OjnDecoder::decode_with_report(&slow.bytes(), 0).unwrap();
    assert_eq!(chart.notes, [Note::tap(0, 1)]);
    assert_eq!(report.events_out_of_range, 2);
    assert_eq!(report.negative_measures, 1);
}

#[test]
fn a_negative_sample_value_is_skipped() {
    let ojn = one_section(&[pkg(0, 2, &[note(-4, 0, 0)]), pkg(0, 3, &[tap(1)])]);
    let (chart, report) = OjnDecoder::decode_with_report(&ojn.bytes(), 0).unwrap();
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(report.negative_sample_values, 1);
}

// -------------------------------------------------------------------------------------------------- auto dispatch

#[test]
fn auto_decode_variant_reads_the_nth_playable_difficulty_of_an_ojn_file() {
    use crate::formats::auto::{auto_decode, auto_decode_bytes, auto_decode_variant};
    let normal = pkg(0, 2, &[tap(1)]);
    let hard = concat(&[pkg(0, 2, &[tap(1)]), pkg(0, 3, &[tap(2)])]);
    let bytes = Ojn::new([Vec::new(), normal, hard]).bytes();
    let dir = std::env::temp_dir().join(format!("rox-o2jam-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("o2ma1.OJN");
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(auto_decode(&path).unwrap().notes.len(), 1);
    assert_eq!(auto_decode_variant(&path, 0).unwrap().notes.len(), 1);
    assert_eq!(auto_decode_variant(&path, 1).unwrap().notes.len(), 2);
    assert_eq!(
        auto_decode_variant(&path, 1)
            .unwrap()
            .metadata
            .difficulty_name,
        "HX"
    );
    assert!(auto_decode_variant(&path, 2).is_err());
    assert_eq!(auto_decode_bytes("x.ojn", &bytes).unwrap().notes.len(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

// ---------------------------------------------------------------------------------------------------- OJM builders

fn name32(name: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..name.len()].copy_from_slice(name.as_bytes());
    out
}

/// An M30 file; entries are `(codec, ref, payload)`; the payload is `XOR`ed with the mask of `flag` as stored.
fn m30(flag: i32, entries: &[(i16, i16, &[u8])]) -> Vec<u8> {
    let mask: Option<&[u8; 4]> = match flag {
        16 => Some(b"nami"),
        32 => Some(b"0412"),
        _ => None,
    };
    let mut out = Vec::new();
    out.extend_from_slice(b"M30\0");
    out.extend_from_slice(&1i32.to_le_bytes());
    out.extend_from_slice(&flag.to_le_bytes());
    out.extend_from_slice(&i32::try_from(entries.len()).unwrap().to_le_bytes());
    out.extend_from_slice(&28i32.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    for &(codec, reference, payload) in entries {
        out.extend_from_slice(&name32("sample"));
        out.extend_from_slice(&i32::try_from(payload.len()).unwrap().to_le_bytes());
        out.extend_from_slice(&codec.to_le_bytes());
        out.extend_from_slice(&2i16.to_le_bytes());
        out.extend_from_slice(&0i32.to_le_bytes());
        out.extend_from_slice(&reference.to_le_bytes());
        out.extend_from_slice(&0i16.to_le_bytes());
        out.extend_from_slice(&0i32.to_le_bytes());
        let mut payload = payload.to_vec();
        if let Some(mask) = mask {
            for group in payload.as_chunks_mut::<4>().0 {
                for (byte, key) in group.iter_mut().zip(mask) {
                    *byte ^= key;
                }
            }
        }
        out.extend_from_slice(&payload);
    }
    out
}

/// The "acc xor" in the encoding direction: `decode` of the result gives `plain` back.
fn acc_encode(state: &mut AccXor, plain: &[u8]) -> Vec<u8> {
    plain
        .iter()
        .map(|&byte| {
            let encoded = if (state.key << state.counter) & 0x80 != 0 {
                !byte
            } else {
                byte
            };
            state.counter += 1;
            if state.counter > 7 {
                state.counter = 0;
                state.key = encoded;
            }
            encoded
        })
        .collect()
}

/// The inverse of `rearrange`.
fn unrearrange(plain: &[u8]) -> Vec<u8> {
    let len = plain.len();
    let block = len / 17;
    let key = 17 * (len % 17);
    let mut encoded = plain.to_vec();
    for b in 0..17 {
        let from = block * usize::from(REARRANGE_TABLE[key + b]);
        encoded[block * b..block * (b + 1)].copy_from_slice(&plain[from..from + block]);
    }
    encoded
}

struct Wav<'a> {
    channels: i16,
    rate: i32,
    bits: i16,
    pcm: &'a [u8],
}

/// An OMC (`scrambled`) or OJM file: WAV slots (`None` = empty slot) then Ogg slots.
fn omc(
    signature: [u8; 4],
    scrambled: bool,
    wavs: &[Option<Wav<'_>>],
    oggs: &[Option<&[u8]>],
) -> Vec<u8> {
    let mut body = Vec::new();
    let mut acc = AccXor::new();
    for slot in wavs {
        body.extend_from_slice(&name32("wavslot"));
        let (channels, rate, bits, size) = slot
            .as_ref()
            .map_or((0, 0, 0, 0), |w| (w.channels, w.rate, w.bits, w.pcm.len()));
        body.extend_from_slice(&1i16.to_le_bytes());
        body.extend_from_slice(&channels.to_le_bytes());
        body.extend_from_slice(&rate.to_le_bytes());
        let block = channels * bits / 8;
        body.extend_from_slice(&(rate * i32::from(block)).to_le_bytes());
        body.extend_from_slice(&block.to_le_bytes());
        body.extend_from_slice(&bits.to_le_bytes());
        body.extend_from_slice(&0i32.to_le_bytes());
        body.extend_from_slice(&i32::try_from(size).unwrap().to_le_bytes());
        if let Some(wav) = slot {
            if scrambled {
                body.extend_from_slice(&unrearrange(&acc_encode(&mut acc, wav.pcm)));
            } else {
                body.extend_from_slice(wav.pcm);
            }
        }
    }
    let ogg_start = 20 + body.len();
    for slot in oggs {
        body.extend_from_slice(&name32("oggslot"));
        let data = slot.unwrap_or(&[]);
        body.extend_from_slice(&i32::try_from(data.len()).unwrap().to_le_bytes());
        body.extend_from_slice(data);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&signature);
    out.extend_from_slice(&i16::try_from(wavs.len()).unwrap().to_le_bytes());
    out.extend_from_slice(&i16::try_from(oggs.len()).unwrap().to_le_bytes());
    out.extend_from_slice(&20i32.to_le_bytes());
    out.extend_from_slice(&i32::try_from(ogg_start).unwrap().to_le_bytes());
    out.extend_from_slice(&i32::try_from(20 + body.len()).unwrap().to_le_bytes());
    out.extend_from_slice(&body);
    out
}

// ------------------------------------------------------------------------------------------------------ OJM tests

#[test]
fn rearrange_matches_a_hand_computed_vector() {
    // 17 bytes: block size 1, key 0, the table starts 10 0E 02 09 04 00 07 01 06 08 0F 0A 05 0C 03 0D 0B, and block
    // `b` goes to the place the table gives: place[16]=0, [14]=1, [2]=2, [9]=3, [4]=4, [0]=5, [7]=6, [1]=7, [6]=8,
    // [8]=9, [15]=10, [10]=11, [5]=12, [12]=13, [3]=14, [13]=15, [11]=16.
    let encoded: Vec<u8> = (0..17).collect();
    assert_eq!(
        rearrange(&encoded),
        [5, 7, 2, 14, 4, 12, 8, 6, 9, 3, 11, 16, 13, 15, 1, 10, 0]
    );
    // Bytes after the 17 blocks stay: 19 bytes = block size 1, 2 left over, key 17 x 2.
    let encoded: Vec<u8> = (0..19).collect();
    let plain = rearrange(&encoded);
    assert_eq!(&plain[17..], [17, 18]);
    let mut sorted = plain.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, (0..19).collect::<Vec<u8>>());
}

#[test]
fn the_acc_xor_matches_a_hand_computed_vector() {
    // The key starts at 0xFF: the first 8 bytes are complemented. The 8th input byte (7) is the key of the next
    // group: its bits 7 - counter decide: 7 = 0b0000_0111, so counters 0..=4 keep the byte, 5..=7 complement it.
    let mut bytes = vec![0, 1, 2, 3, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0];
    AccXor::new().decode(&mut bytes);
    assert_eq!(
        bytes,
        [
            0xFF, 0xFE, 0xFD, 0xFC, 0xFB, 0xFA, 0xF9, 0xF8, 0, 0, 0, 0, 0, 0xFF, 0xFF, 0xFF
        ]
    );
}

#[test]
fn an_m30_gives_ogg_samples_with_the_two_id_ranges() {
    let file = m30(
        0,
        &[(5, 7, b"OggS-key"), (0, 3, b"OggS-bgm"), (2, 1, b"skipped")],
    );
    let ojm = OjmContainer::parse(&file).unwrap();
    assert_eq!(ojm.kind(), OjmKind::M30);
    assert_eq!(ojm.samples().map(|s| s.id).collect::<Vec<_>>(), [7, 1003]);
    assert_eq!(ojm.sample(7).unwrap().data, b"OggS-key");
    assert_eq!(ojm.sample(7).unwrap().format, SampleFormat::Ogg);
    assert_eq!(ojm.sample(1003).unwrap().data, b"OggS-bgm");
    assert_eq!(ojm.sample(3), None);
    assert_eq!(ojm.report().unknown_codec_samples, 1);
    assert!(!ojm.report().truncated);
}

#[test]
fn an_m30_scramble_is_undone_on_whole_groups_of_four_bytes() {
    let payload: Vec<u8> = (0..11).collect(); // 2 groups + 3 bytes that are not `XOR`ed
    for flag in [0, 16, 32] {
        let ojm = OjmContainer::parse(&m30(flag, &[(5, 1, &payload)])).unwrap();
        assert_eq!(ojm.sample(1).unwrap().data, payload, "flag {flag}");
    }
    // The stored bytes really differ for a flag, and the trailing bytes do not.
    let stored = m30(16, &[(5, 1, &payload)]);
    assert_eq!(stored[28 + 52], payload[0] ^ b'n');
    assert_eq!(stored[28 + 52 + 10], payload[10]);
    assert!(OjmContainer::parse(&m30(8, &[(5, 1, &payload)])).is_err());
    assert!(OjmContainer::parse(&m30(1, &[(5, 1, &payload)])).is_err());
}

#[test]
fn an_m30_stops_at_the_end_of_its_bytes_whatever_the_header_count_says() {
    let mut file = m30(0, &[(5, 1, b"a"), (5, 2, b"b")]);
    put(&mut file, 12, &9i32.to_le_bytes()); // claims 9 samples
    let ojm = OjmContainer::parse(&file).unwrap();
    assert_eq!(ojm.len(), 2);
    put(&mut file, 12, &1i32.to_le_bytes()); // claims 1: the real number is read
    assert_eq!(OjmContainer::parse(&file).unwrap().len(), 2);
}

fn pcm(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| u8::try_from((i * 7 + usize::from(seed)) % 251).unwrap())
        .collect()
}

#[test]
fn an_omc_is_unscrambled_and_its_wav_gets_a_riff_header() {
    // Two WAV samples in one file (the "acc" state runs across them), an empty slot that takes id 2, one more, then
    // Ogg slots with an empty one.
    let (a, b, c) = (pcm(40, 1), pcm(100, 2), pcm(35, 3));
    let file = omc(
        *b"OMC\0",
        true,
        &[
            Some(Wav {
                channels: 2,
                rate: 44_100,
                bits: 16,
                pcm: &a,
            }),
            Some(Wav {
                channels: 1,
                rate: 22_050,
                bits: 16,
                pcm: &b,
            }),
            None,
            Some(Wav {
                channels: 1,
                rate: 8_000,
                bits: 8,
                pcm: &c,
            }),
        ],
        &[Some(b"OggS-one"), None, Some(b"OggS-three")],
    );
    let ojm = OjmContainer::parse(&file).unwrap();
    assert_eq!(ojm.kind(), OjmKind::Omc);
    assert_eq!(
        ojm.samples().map(|s| s.id).collect::<Vec<_>>(),
        [0, 1, 3, 1000, 1002]
    );
    let wav = ojm.sample(0).unwrap();
    assert_eq!(wav.format, SampleFormat::Wav);
    assert_eq!(wav.data.len(), 44 + 40);
    assert_eq!(&wav.data[..4], b"RIFF");
    assert_eq!(
        u32::from_le_bytes(wav.data[4..8].try_into().unwrap()),
        36 + 40
    );
    assert_eq!(&wav.data[8..16], b"WAVEfmt ");
    assert_eq!(u32::from_le_bytes(wav.data[16..20].try_into().unwrap()), 16);
    assert_eq!(u16::from_le_bytes(wav.data[20..22].try_into().unwrap()), 1); // PCM
    assert_eq!(u16::from_le_bytes(wav.data[22..24].try_into().unwrap()), 2); // channels
    assert_eq!(
        u32::from_le_bytes(wav.data[24..28].try_into().unwrap()),
        44_100
    );
    assert_eq!(
        u32::from_le_bytes(wav.data[28..32].try_into().unwrap()),
        44_100 * 4
    );
    assert_eq!(u16::from_le_bytes(wav.data[32..34].try_into().unwrap()), 4);
    assert_eq!(u16::from_le_bytes(wav.data[34..36].try_into().unwrap()), 16);
    assert_eq!(&wav.data[36..40], b"data");
    assert_eq!(u32::from_le_bytes(wav.data[40..44].try_into().unwrap()), 40);
    assert_eq!(&wav.data[44..], &a[..]);
    assert_eq!(&ojm.sample(1).unwrap().data[44..], &b[..]);
    // 35 bytes: odd length, a pad byte follows the data without being counted.
    let odd = ojm.sample(3).unwrap();
    assert_eq!(odd.data.len(), 44 + 35 + 1);
    assert_eq!(u32::from_le_bytes(odd.data[40..44].try_into().unwrap()), 35);
    assert_eq!(&odd.data[44..79], &c[..]);
    assert_eq!(ojm.sample(1000).unwrap().data, b"OggS-one");
    assert_eq!(ojm.sample(1002).unwrap().format, SampleFormat::Ogg);
    assert_eq!(ojm.sample(2), None);
    assert_eq!(ojm.sample(1001), None);
}

#[test]
fn an_ojm_keeps_its_wav_payload_as_it_is() {
    let a = pcm(64, 9);
    let file = omc(
        *b"OJM\0",
        false,
        &[Some(Wav {
            channels: 1,
            rate: 44_100,
            bits: 16,
            pcm: &a,
        })],
        &[Some(b"OggS")],
    );
    let ojm = OjmContainer::parse(&file).unwrap();
    assert_eq!(ojm.kind(), OjmKind::Ojm);
    assert_eq!(&ojm.sample(0).unwrap().data[44..], &a[..]);
    assert_eq!(ojm.sample(1000).unwrap().data, b"OggS");
    // The same bytes with the OMC signature are scrambled the other way and do not come out equal.
    let mut as_omc = file.clone();
    as_omc[..4].copy_from_slice(b"OMC\0");
    assert_ne!(
        &OjmContainer::parse(&as_omc)
            .unwrap()
            .sample(0)
            .unwrap()
            .data[44..],
        &a[..]
    );
}

#[test]
fn an_omc_with_a_size_that_is_not_a_multiple_of_seventeen_round_trips() {
    for len in [17usize, 34, 35, 52, 100, 101, 255, 300] {
        let a = pcm(len, 5);
        let file = omc(
            *b"OMC\0",
            true,
            &[Some(Wav {
                channels: 1,
                rate: 8_000,
                bits: 8,
                pcm: &a,
            })],
            &[],
        );
        let ojm = OjmContainer::parse(&file).unwrap();
        assert_eq!(
            &ojm.sample(0).unwrap().data[44..44 + len],
            &a[..],
            "len {len}"
        );
    }
}

#[test]
fn the_first_sample_wins_on_a_duplicate_id() {
    let file = m30(0, &[(5, 1, b"first"), (5, 1, b"second")]);
    let ojm = OjmContainer::parse(&file).unwrap();
    assert_eq!(ojm.sample(1).unwrap().data, b"first");
    assert_eq!(ojm.report().duplicate_ids, 1);
}

#[test]
fn a_container_that_is_not_one_is_an_error() {
    assert!(OjmContainer::parse(b"").is_err());
    assert!(OjmContainer::parse(b"RIFF....").is_err());
    assert!(OjmContainer::parse(b"M30\0abc").is_err());
    assert!(OjmContainer::parse(b"OMC\0abc").is_err());
    assert!(OjmContainer::parse(b"OJM\0").is_err());
}

#[test]
fn every_prefix_of_a_container_and_huge_sizes_are_safe() {
    let a = pcm(50, 1);
    let files = [
        m30(16, &[(5, 1, b"OggS-key-sample"), (0, 2, b"OggS-bgm")]),
        omc(
            *b"OMC\0",
            true,
            &[
                Some(Wav {
                    channels: 1,
                    rate: 8_000,
                    bits: 8,
                    pcm: &a,
                }),
                None,
            ],
            &[Some(b"OggSabc")],
        ),
        omc(
            *b"OJM\0",
            false,
            &[Some(Wav {
                channels: 1,
                rate: 8_000,
                bits: 8,
                pcm: &a,
            })],
            &[Some(b"OggSabc")],
        ),
    ];
    for file in &files {
        for len in 0..=file.len() {
            let _ = OjmContainer::parse(&file[..len]);
        }
        // Every 4-byte field set to a huge or negative value in turn.
        for at in (0..file.len().saturating_sub(4)).step_by(2) {
            for value in [i32::MAX, i32::MIN, -1, 0x7FFF_0000] {
                let mut data = file.clone();
                put(&mut data, at, &value.to_le_bytes());
                let _ = OjmContainer::parse(&data);
            }
        }
    }
    let mut cut = m30(0, &[(5, 1, b"abcdefgh")]);
    put(&mut cut, 28 + 32, &i32::MAX.to_le_bytes());
    let ojm = OjmContainer::parse(&cut).unwrap();
    assert!(ojm.is_empty());
    assert!(ojm.report().truncated);
}
