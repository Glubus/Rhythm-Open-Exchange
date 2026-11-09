//! Tests of the `.ssq` decoder. Every file here is built in memory with the chunk layout of the format notes (the tempo chunk of the
//! notes' example is used as a byte-for-byte test vector); no real file is used (Konami's data has no licence).

use rox::codec::Decoder;
use rox::model::{Note, NoteType, TimingPoint};

use super::{Difficulty, SsqDecoder, Style};

// ---------------------------------------------------------------------------------------------------- builders

const SINGLE_EXPERT: u16 = 0x0314;
const SINGLE_BASIC: u16 = 0x0114;
const DOUBLE_CHALLENGE: u16 = 0x0618;

/// A chunk: header and body padded with zeros to a multiple of 4.
fn chunk(kind: u16, param2: u16, param3: u16, body: &[u8]) -> Vec<u8> {
    let mut body = body.to_vec();
    while !body.len().is_multiple_of(4) {
        body.push(0);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&u32::try_from(12 + body.len()).unwrap().to_le_bytes());
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&param2.to_le_bytes());
    out.extend_from_slice(&param3.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&body);
    out
}

fn i32s(values: impl IntoIterator<Item = i32>) -> Vec<u8> {
    values.into_iter().flat_map(i32::to_le_bytes).collect()
}

/// Tempo chunk: offsets then clock values.
fn tempo(tps: u16, entries: &[(i32, i32)]) -> Vec<u8> {
    let mut body = i32s(entries.iter().map(|e| e.0));
    body.extend(i32s(entries.iter().map(|e| e.1)));
    chunk(1, tps, u16::try_from(entries.len()).unwrap(), &body)
}

/// The canonical events chunk.
fn events() -> Vec<u8> {
    let mut body = i32s([0, 0, 4096, 4096, 16384, 20480]);
    body.extend([1, 4, 2, 1, 2, 2, 2, 5, 2, 3, 2, 4]);
    chunk(2, 1, 6, &body)
}

/// Step chunk: rows `(tick, step byte)` and the freeze block, with the pads the notes describe.
fn steps(code: u16, rows: &[(i32, u8)], freezes: &[(u8, u8)]) -> Vec<u8> {
    let mut body = i32s(rows.iter().map(|r| r.0));
    body.extend(rows.iter().map(|r| r.1));
    if rows.len() % 2 == 1 {
        body.push(0);
    }
    for &(panels, freeze_kind) in freezes {
        body.extend([panels, freeze_kind]);
    }
    chunk(3, code, u16::try_from(rows.len()).unwrap(), &body)
}

/// Mine chunk: `(tick, panels, flags, reserved)`.
fn mines(code: u16, entries: &[(i32, u8, u8, u16)]) -> Vec<u8> {
    let mut body = Vec::new();
    for &(tick, panels, flags, reserved) in entries {
        body.extend(tick.to_le_bytes());
        body.extend([panels, flags]);
        body.extend(reserved.to_le_bytes());
    }
    chunk(20, code, u16::try_from(entries.len()).unwrap(), &body)
}

fn file(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut out: Vec<u8> = chunks.concat();
    out.extend([0, 0, 0, 0]);
    out
}

/// 120 BPM at 1000 ticks per second: 4096 ticks are 2 s.
fn tempo_120() -> Vec<u8> {
    tempo(1000, &[(0, 0), (4096, 2000)])
}

fn simple(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut all = vec![tempo_120(), events()];
    all.extend_from_slice(chunks);
    file(&all)
}

fn notes_of(chart: &rox::model::RoxChart) -> Vec<(i64, u8, NoteType)> {
    chart
        .notes
        .iter()
        .map(|n| (n.time_us, n.column, n.note_type))
        .collect()
}

// ---------------------------------------------------------------------------------------------------- tests

#[test]
fn single_taps_and_jumps_land_on_their_columns_and_times() {
    // A beat is 1024 ticks = 0.5 s at 120 BPM.
    let data = simple(&[steps(
        SINGLE_EXPERT,
        &[(0, 0x01), (1024, 0x02), (2048, 0x0C)],
        &[],
    )]);
    let chart = SsqDecoder::decode_variant(&data, 0).unwrap();
    assert_eq!(chart.key_count, 4);
    assert_eq!(chart.metadata.difficulty_name, "Expert");
    assert_eq!(chart.metadata.difficulty_value, None);
    assert_eq!(chart.metadata.audio_file, "");
    assert_eq!(chart.metadata.title, "");
    assert_eq!(
        notes_of(&chart),
        [
            (0, 0, NoteType::Tap),
            (500_000, 1, NoteType::Tap),
            (1_000_000, 2, NoteType::Tap),
            (1_000_000, 3, NoteType::Tap),
        ]
    );
    assert_eq!(chart.timing_points, [TimingPoint::bpm(0, 120.0)]);
    // Decoding through the `Decoder` trait reads the first chart too.
    assert_eq!(SsqDecoder::decode(&data).unwrap().notes.len(), 4);
}

#[test]
fn double_chart_has_eight_columns() {
    let data = simple(&[steps(DOUBLE_CHALLENGE, &[(0, 0x81), (1024, 0x18)], &[])]);
    let chart = SsqDecoder::decode_variant(&data, 0).unwrap();
    assert_eq!(chart.key_count, 8);
    assert_eq!(chart.metadata.difficulty_name, "Challenge");
    assert_eq!(
        notes_of(&chart),
        [
            (0, 0, NoteType::Tap),
            (0, 7, NoteType::Tap),
            (500_000, 3, NoteType::Tap),
            (500_000, 4, NoteType::Tap),
        ]
    );
}

#[test]
fn a_freeze_is_a_hold_from_its_head_to_the_zero_step() {
    // Left head at 0, freeze end at tick 2048 (1 s) for panel 0; the freeze block has one entry. N = 2 (even, no pad).
    let data = simple(&[steps(
        SINGLE_EXPERT,
        &[(0, 0x01), (2048, 0x00)],
        &[(0x01, 1)],
    )]);
    let (chart, report) = SsqDecoder::decode_with_report(&data, 0).unwrap();
    assert_eq!(
        notes_of(&chart),
        [(
            0,
            0,
            NoteType::Hold {
                duration_us: 1_000_000
            }
        )]
    );
    assert_eq!(report.freeze_ends_without_head, 0);
    assert_eq!(SsqDecoder::variants(&data).unwrap()[0].holds, 1);
}

#[test]
fn freeze_block_follows_the_odd_pad_and_two_heads_can_share_a_tail() {
    // N = 3 (odd: one pad byte before the freeze block): jump head left+down at 0, tap up at 1024, one tail ending both heads.
    let rows = [(0, 0x03), (1024, 0x04), (2048, 0x00)];
    let chunk_bytes = steps(SINGLE_EXPERT, &rows, &[(0x03, 1)]);
    assert_eq!(chunk_bytes.len(), 12 + 12 + 3 + 1 + 2 + 2); // header, ticks, steps, pad, freeze, trailing pad
    let chart = SsqDecoder::decode_variant(&simple(&[chunk_bytes]), 0).unwrap();
    assert_eq!(
        notes_of(&chart),
        [
            (
                0,
                0,
                NoteType::Hold {
                    duration_us: 1_000_000
                }
            ),
            (
                0,
                1,
                NoteType::Hold {
                    duration_us: 1_000_000
                }
            ),
            (500_000, 2, NoteType::Tap),
        ]
    );
}

#[test]
fn freeze_end_names_only_some_panels_of_the_head_row() {
    // A row hits left and right; only right is a freeze.
    let data = simple(&[steps(
        SINGLE_EXPERT,
        &[(0, 0x09), (1024, 0x00)],
        &[(0x08, 1)],
    )]);
    let chart = SsqDecoder::decode_variant(&data, 0).unwrap();
    assert_eq!(
        notes_of(&chart),
        [
            (0, 0, NoteType::Tap),
            (
                0,
                3,
                NoteType::Hold {
                    duration_us: 500_000
                }
            ),
        ]
    );
}

#[test]
fn freeze_oddities_are_ignored_and_counted() {
    // kind 2 ignored; an end on a panel with no head; an end at the head's own time.
    let data = simple(&[steps(
        SINGLE_EXPERT,
        &[
            (0, 0x01),
            (0, 0x00),
            (1024, 0x00),
            (2048, 0x00),
            (3072, 0x01),
        ],
        &[(0x01, 1), (0x02, 1), (0x01, 2)],
    )]);
    let (chart, report) = SsqDecoder::decode_with_report(&data, 0).unwrap();
    // First 0x00 (tick 0): panel 0 ends at its own head's time: stays a tap. Second: panel 1 has no head. Third: kind 2.
    assert_eq!(
        notes_of(&chart),
        [(0, 0, NoteType::Tap), (1_500_000, 0, NoteType::Tap)]
    );
    assert_eq!(report.empty_freezes_as_taps, 1);
    assert_eq!(report.freeze_ends_without_head, 1);
    assert_eq!(report.ignored_freeze_kinds, 1);
}

#[test]
fn shock_arrows_are_mines_on_their_side() {
    let single = simple(&[steps(
        SINGLE_EXPERT,
        &[(0, 0x0F), (1024, 0xFF), (2048, 0x01)],
        &[],
    )]);
    let (chart, report) = SsqDecoder::decode_with_report(&single, 0).unwrap();
    assert_eq!(report.shock_arrows, 2);
    assert_eq!(chart.notes.iter().filter(|n| n.is_mine()).count(), 8);
    assert!(chart.notes.iter().all(|n| n.column < 4));
    assert_eq!(chart.notes[0].time_us, 0);

    let double = simple(&[steps(
        DOUBLE_CHALLENGE,
        &[(0, 0x0F), (1024, 0xF0), (2048, 0xFF)],
        &[],
    )]);
    let chart = SsqDecoder::decode_variant(&double, 0).unwrap();
    let columns = |time: i64| -> Vec<u8> {
        chart
            .notes
            .iter()
            .filter(|n| n.time_us == time && n.is_mine())
            .map(|n| n.column)
            .collect()
    };
    assert_eq!(columns(0), [0, 1, 2, 3]);
    assert_eq!(columns(500_000), [4, 5, 6, 7]);
    assert_eq!(columns(1_000_000), [0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(SsqDecoder::variants(&double).unwrap()[0].mines, 16);
}

#[test]
fn panels_the_style_lacks_are_dropped() {
    // 0x13 in a single chart: left, down and a fifth panel that does not exist; 0x30 leaves nothing.
    let data = simple(&[steps(
        SINGLE_EXPERT,
        &[(0, 0x13), (1024, 0x30), (2048, 0x01)],
        &[],
    )]);
    let (chart, report) = SsqDecoder::decode_with_report(&data, 0).unwrap();
    assert_eq!(chart.notes.len(), 3);
    assert_eq!(report.panels_outside_style, 2);
}

#[test]
fn a_stop_delays_the_notes_after_it_and_is_hit_when_it_begins() {
    // 120 BPM for a measure, a one-second stop at tick 4096 (equal offsets), then 240 BPM (4096 ticks in 1 s).
    let tempo = tempo(1000, &[(0, 0), (4096, 2000), (4096, 3000), (8192, 4000)]);
    let data = file(&[
        tempo,
        steps(
            SINGLE_EXPERT,
            &[(4096, 0x01), (6144, 0x01), (8192, 0x01)],
            &[],
        ),
    ]);
    let chart = SsqDecoder::decode_variant(&data, 0).unwrap();
    assert_eq!(
        notes_of(&chart),
        [
            (2_000_000, 0, NoteType::Tap),
            (3_500_000, 0, NoteType::Tap),
            (4_000_000, 0, NoteType::Tap),
        ]
    );
    assert_eq!(
        chart.timing_points,
        [
            TimingPoint::bpm(0, 120.0),
            TimingPoint::bpm(3_000_000, 240.0)
        ]
    );
}

#[test]
fn ticks_before_and_after_the_tempo_entries_extend_the_nearest_bpm() {
    // Legacy origin: the first entry is at tick -4096, 3 s into the audio. A measure takes 2 s.
    let tempo = tempo(1000, &[(-4096, 3000), (0, 5000), (4096, 6000)]);
    let data = file(&[
        tempo,
        steps(SINGLE_EXPERT, &[(0, 0x01), (2048, 0x01), (8192, 0x01)], &[]),
    ]);
    let chart = SsqDecoder::decode_variant(&data, 0).unwrap();
    // Tick 0 is 5 s, tick 2048 is 5.5 s, tick 8192 extends the last segment (1 s per measure): 7 s.
    assert_eq!(
        chart.notes.iter().map(|n| n.time_us).collect::<Vec<_>>(),
        [5_000_000, 5_500_000, 7_000_000]
    );
    let bpms: Vec<f32> = chart
        .timing_points
        .iter()
        .filter_map(TimingPoint::bpm_value)
        .collect();
    assert_eq!(bpms, [120.0, 240.0]);
    // The first BPM sits no later than the first note.
    assert!(chart.timing_points[0].time_us() <= chart.notes[0].time_us);
}

/// The tempo chunk of the format notes' example (skogaby's `ssq_format.md` section 3.5): TPS 150, nine entries, two stops.
fn documented_tempo() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(84u32.to_le_bytes());
    bytes.extend([1, 0, 0x96, 0, 9, 0, 0, 0]);
    for offset in [
        0, 4096, 73_728, 73_728, 272_384, 370_688, 370_688, 372_736, 684_032,
    ] {
        bytes.extend(i32::to_le_bytes(offset));
    }
    for data in [1, 94, 1689, 1829, 6376, 10_876, 10_970, 11_064, 18_189] {
        bytes.extend(i32::to_le_bytes(data));
    }
    assert_eq!(bytes.len(), 84);
    bytes
}

#[test]
fn the_documented_tempo_chunk_gives_its_bpm_and_stops() {
    let data = file(&[
        documented_tempo(),
        steps(
            SINGLE_BASIC,
            &[(0, 0x01), (73_728, 0x01), (73_729, 0x01), (684_032, 0x01)],
            &[],
        ),
    ]);
    let (chart, report) = SsqDecoder::decode_with_report(&data, 0).unwrap();
    assert_eq!(report.ticks_per_second, 150);
    // Tick 0: tempo_data[0] / 150 s. Tick 73728 begins the 0.933 s stop: 1689 / 150 = 11.26 s. One tick after it: after the stop
    // (1829 / 150 s) plus the next segment's 1 tick. The last entry: 18189 / 150 = 121.26 s.
    let times: Vec<i64> = chart.notes.iter().map(|n| n.time_us).collect();
    assert_eq!(times[0], 6667);
    assert_eq!(times[1], 11_260_000);
    assert_eq!(times[3], 121_260_000);
    assert!((12_193_480..=12_193_490).contains(&times[2]), "{times:?}");
    let bpms: Vec<f32> = chart
        .timing_points
        .iter()
        .filter_map(TimingPoint::bpm_value)
        .collect();
    // 240 x 150 x 4096 / (4096 x 93) = 387.1; 240 x 150 x 69632 / (4096 x 1595) = 383.7 (the notes' value).
    assert!((bpms[0] - 387.096_77).abs() < 1e-3, "{bpms:?}");
    assert!((bpms[1] - 383.7).abs() < 0.1, "{bpms:?}");
    // The stops are in the clock: the second segment after a stop starts at 1829 / 150 s.
    assert!(
        chart
            .timing_points
            .iter()
            .any(|p| p.time_us() == 12_193_333)
    );
}

#[test]
fn mine_chunk_adds_mines_and_refuses_bad_entries() {
    let data = simple(&[
        steps(SINGLE_EXPERT, &[(0, 0x01), (2048, 0x00)], &[(0x01, 1)]),
        steps(SINGLE_BASIC, &[(0, 0x01)], &[]),
        mines(
            SINGLE_EXPERT,
            &[
                (3072, 0x05, 0, 0), // two panels, after the freeze
                (1024, 0x02, 0, 0), // other column during the freeze: kept
                (1024, 0x01, 0, 0), // inside the freeze of column 0: dropped
                (512, 0x0F, 0, 0),  // shock pattern: refused
                (512, 0x00, 0, 0),  // no panel
                (512, 0x10, 0, 0),  // panel a single chart lacks
                (512, 0x02, 1, 0),  // flags
                (512, 0x02, 0, 7),  // reserved
                (-1, 0x02, 0, 0),   // negative tick
            ],
        ),
        // A mine chunk of a code with no step chunk is ignored.
        mines(0x0618, &[(0, 0x01, 0, 0)]),
    ]);
    let (chart, report) = SsqDecoder::decode_with_report(&data, 0).unwrap();
    assert_eq!(report.mines_from_chunk, 3);
    assert_eq!(report.invalid_mine_entries, 6);
    assert_eq!(report.overlapping_notes_dropped, 1);
    assert_eq!(
        notes_of(&chart),
        [
            (
                0,
                0,
                NoteType::Hold {
                    duration_us: 1_000_000
                }
            ),
            (500_000, 1, NoteType::Mine),
            (1_500_000, 0, NoteType::Mine),
            (1_500_000, 2, NoteType::Mine),
        ]
    );
    // The basic chart has no mine chunk.
    assert!(
        SsqDecoder::decode_variant(&data, 1)
            .unwrap()
            .notes
            .iter()
            .all(|n| !n.is_mine())
    );
}

#[test]
fn variants_are_the_playable_step_chunks_in_file_order_and_the_rest_is_reported() {
    let data = simple(&[
        steps(0x0314, &[(0, 0x01)], &[]),
        steps(0x0514, &[(0, 0x01)], &[]),    // slot 5: not accepted
        steps(0x011A, &[(0, 0x01)], &[]),    // another style
        steps(0x0314, &[(1024, 0x02)], &[]), // duplicate code: the first is the chart
        steps(0x0214, &[(0, 0x00)], &[(0x01, 1)]), // no playable note
        chunk(9, 0x0114, 1, &[0; 8]),        // Hudson / metadata
        steps(0x0418, &[(0, 0x11)], &[]),
        steps(0x0614, &[(0, 0x04)], &[]),
    ]);
    let variants = SsqDecoder::variants(&data).unwrap();
    let names: Vec<String> = variants.iter().map(super::Variant::name).collect();
    assert_eq!(
        names,
        ["Single Expert", "Double Beginner", "Single Challenge"]
    );
    assert_eq!(
        variants
            .iter()
            .map(|v| (v.index, v.code, v.key_count))
            .collect::<Vec<_>>(),
        [(0, 0x0314, 4), (1, 0x0418, 8), (2, 0x0614, 4)]
    );
    assert_eq!(
        (variants[0].style, variants[0].difficulty),
        (Style::Single, Difficulty::Expert)
    );
    // The duplicate did not replace the first.
    assert_eq!(
        SsqDecoder::decode_variant(&data, 0).unwrap().notes[0].time_us,
        0
    );
    let (_, report) = SsqDecoder::decode_with_report(&data, 2).unwrap();
    let skipped = report.skipped.join("\n");
    for expected in [
        "0x0514",
        "0x011a",
        "second chunk for code 0x0314",
        "no playable note",
        "type 9",
    ] {
        assert!(skipped.contains(expected), "{expected}: {skipped}");
    }
    assert!(SsqDecoder::decode_variant(&data, 3).is_err());
}

#[test]
fn decode_with_title_sets_the_title_only() {
    let data = simple(&[steps(SINGLE_EXPERT, &[(0, 0x01)], &[])]);
    let chart = SsqDecoder::decode_with_title(&data, 0, "a01_all").unwrap();
    assert_eq!(chart.metadata.title, "a01_all");
    assert_eq!(chart.metadata.audio_file, "");
    assert_eq!(chart.metadata.artist, "");
}

#[test]
fn decode_file_uses_the_file_stem() {
    let dir = std::env::temp_dir().join(format!("rox_ssq_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("abc12_all.ssq");
    std::fs::write(&path, simple(&[steps(SINGLE_EXPERT, &[(0, 0x01)], &[])])).unwrap();
    let chart = SsqDecoder::decode_file(&path, 0).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(chart.metadata.title, "abc12_all");
}

#[test]
fn builder_and_parser_round_trip_on_generated_charts() {
    // 120 BPM: 128 ticks are 62.5 ms exactly. A xorshift picks the rows; shocks are left out (their bytes are not taps).
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for (code, key_count) in [(SINGLE_EXPERT, 4u8), (DOUBLE_CHALLENGE, 8)] {
        let panel_mask: u64 = if key_count == 8 { 0xFF } else { 0x0F };
        let mut rows = Vec::new();
        let mut expected = Vec::new();
        for row in 0..200i32 {
            let tick = row * 128;
            let mut byte = u8::try_from(next() & panel_mask).unwrap();
            if byte == 0 || matches!(byte, 0x0F | 0xF0 | 0xFF) {
                byte = 0x01;
            }
            rows.push((tick, byte));
            for column in 0..key_count {
                if byte & (1 << column) != 0 {
                    expected.push(Note::tap(i64::from(tick) * 62_500 / 128, column));
                }
            }
        }
        let chart = SsqDecoder::decode_variant(&simple(&[steps(code, &rows, &[])]), 0).unwrap();
        assert_eq!(chart.key_count, key_count);
        assert_eq!(chart.notes, expected);
    }
}

#[test]
fn bad_tempo_chunks_are_errors() {
    let steps = steps(SINGLE_EXPERT, &[(0, 0x01)], &[]);
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("no tempo chunk", file(std::slice::from_ref(&steps))),
        (
            "zero TPS",
            file(&[tempo(0, &[(0, 0), (4096, 2000)]), steps.clone()]),
        ),
        ("no entries", file(&[tempo(1000, &[]), steps.clone()])),
        ("one entry", file(&[tempo(1000, &[(0, 0)]), steps.clone()])),
        (
            "offsets backwards",
            file(&[tempo(1000, &[(4096, 0), (0, 2000)]), steps.clone()]),
        ),
        (
            "clock backwards",
            file(&[tempo(1000, &[(0, 2000), (4096, 0)]), steps.clone()]),
        ),
        (
            "negative stop",
            file(&[tempo(1000, &[(0, 0), (0, -5), (4096, 2000)]), steps.clone()]),
        ),
        (
            "no BPM at all",
            file(&[tempo(1000, &[(0, 7), (4096, 7)]), steps.clone()]),
        ),
        (
            "count above the body",
            file(&[
                chunk(1, 1000, 500, &i32s([0, 4096, 0, 2000])),
                steps.clone(),
            ]),
        ),
    ];
    for (name, data) in cases {
        assert!(SsqDecoder::variants(&data).is_err(), "{name}");
        assert!(SsqDecoder::decode_variant(&data, 0).is_err(), "{name}");
    }
}

#[test]
fn hostile_step_chunks_are_skipped_not_trusted() {
    // 65535 entries announced, a few bytes present; the other chart is still read.
    let huge = chunk(3, SINGLE_EXPERT, 65_535, &i32s([0, 1024]));
    // Freeze ends announced by the zero bytes but missing from the block.
    let missing = steps(0x0414, &[(0, 0x00), (1024, 0x00)], &[]);
    let data = simple(&[huge, missing, steps(SINGLE_BASIC, &[(0, 0x01)], &[])]);
    let variants = SsqDecoder::variants(&data).unwrap();
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0].code, SINGLE_BASIC);
    let (_, report) = SsqDecoder::decode_with_report(&data, 0).unwrap();
    assert_eq!(report.skipped.len(), 2, "{:?}", report.skipped);
}

#[test]
fn bad_chunk_lengths_end_the_walk_and_keep_what_was_read() {
    let good = simple(&[steps(SINGLE_EXPERT, &[(0, 0x01)], &[])]);
    let body_end = good.len() - 4; // the terminator
    for tail in [
        vec![0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0], // length past the file
        vec![5, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0], // not a multiple of 4
        vec![8, 0, 0, 0, 3, 0, 0, 0],             // under 12
        vec![0xAA, 0xBB],                         // less than a length
        vec![0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF], // padding after the terminator
    ] {
        let mut data = good[..body_end].to_vec();
        data.extend(&tail);
        assert_eq!(SsqDecoder::variants(&data).unwrap().len(), 1, "{tail:?}");
    }
    // A chunk whose param2 is the sentinel ends the lookup: the step chunk after it is not found.
    let mut data = good[..body_end].to_vec();
    data.extend(chunk(2, 0xFFFF, 0, &[]));
    data.extend(steps(SINGLE_BASIC, &[(0, 0x01)], &[]));
    data.extend([0, 0, 0, 0]);
    let variants = SsqDecoder::variants(&data).unwrap();
    assert_eq!(variants.len(), 1);
    let (_, report) = SsqDecoder::decode_with_report(&data, 0).unwrap();
    assert!(
        report.skipped.iter().any(|s| s.contains("0xFFFF")),
        "{:?}",
        report.skipped
    );
}

#[test]
fn truncating_a_valid_file_at_every_length_never_panics() {
    let data = simple(&[
        steps(
            SINGLE_EXPERT,
            &[(0, 0x01), (2048, 0x00), (3072, 0x0F)],
            &[(0x01, 1)],
        ),
        steps(DOUBLE_CHALLENGE, &[(0, 0x81)], &[]),
        mines(SINGLE_EXPERT, &[(1024, 0x02, 0, 0)]),
    ]);
    for len in 0..=data.len() {
        let prefix = &data[..len];
        let _ = SsqDecoder::variants(prefix);
        let _ = SsqDecoder::decode_with_report(prefix, 0);
        let _ = SsqDecoder::decode_variant(prefix, 1);
    }
}

#[test]
fn garbage_never_panics_nor_allocates_by_its_counts() {
    let mut state = 0x1234_5678_9ABC_DEF1u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let valid = simple(&[steps(SINGLE_EXPERT, &[(0, 0x01)], &[])]);
    for round in 0..300 {
        let mut data = valid.clone();
        // Flip a few bytes of a valid file, or send random bytes after a valid header.
        for _ in 0..=(round % 5) {
            let at = usize::try_from(next() % data.len() as u64).unwrap();
            data[at] = (next() & 0xFF) as u8;
        }
        let _ = SsqDecoder::variants(&data);
        let _ = SsqDecoder::decode_with_report(&data, 0);
        let random: Vec<u8> = (0..(next() % 200)).map(|_| (next() & 0xFF) as u8).collect();
        let _ = SsqDecoder::variants(&random);
    }
}

#[test]
fn extreme_tempo_slopes_do_not_overflow() {
    // One tick-second per 2^31 ticks: notes far out are skipped, never wrapped.
    let data = file(&[
        tempo(1, &[(0, 0), (i32::MAX, 1)]),
        steps(SINGLE_EXPERT, &[(0, 0x01), (i32::MAX, 0x01)], &[]),
    ]);
    let (chart, report) = SsqDecoder::decode_with_report(&data, 0).unwrap();
    assert!(
        chart
            .notes
            .iter()
            .all(|n| n.time_us.abs() <= 10_000_000_000_000)
    );
    assert_eq!(report.notes_out_of_range + chart.notes.len(), 2);
    // A clock jump of 2^31 seconds-ticks at TPS 1 in a stop: still no panic.
    let data = file(&[
        tempo(1, &[(0, 0), (4096, 1), (4096, i32::MAX), (8192, i32::MAX)]),
        steps(
            SINGLE_EXPERT,
            &[(0, 0x01), (8192, 0x01), (i32::MAX, 0x01)],
            &[],
        ),
    ]);
    let _ = SsqDecoder::variants(&data);
}

#[test]
fn negative_ticks_are_skipped_but_still_use_their_freeze_entry() {
    // The first zero step has a negative tick and eats the first freeze entry (kind 1, panel 0); the second ends the real head.
    let data = simple(&[steps(
        SINGLE_EXPERT,
        &[(-1024, 0x00), (0, 0x02), (1024, 0x00)],
        &[(0x01, 1), (0x02, 1)],
    )]);
    let (chart, report) = SsqDecoder::decode_with_report(&data, 0).unwrap();
    assert_eq!(report.negative_ticks, 1);
    assert_eq!(
        notes_of(&chart),
        [(
            0,
            1,
            NoteType::Hold {
                duration_us: 500_000
            }
        )]
    );
}

#[test]
fn auto_decode_routes_an_ssq_by_its_path_and_lists_every_variant() {
    use crate::formats::auto::{
        auto_decode, auto_decode_all, auto_decode_bytes, auto_decode_variant,
    };
    let dir = std::env::temp_dir().join(format!("rox_ssq_auto_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("a01_all.ssq");
    let data = simple(&[
        steps(SINGLE_EXPERT, &[(0, 0x01)], &[]),
        steps(DOUBLE_CHALLENGE, &[(0, 0x81), (1024, 0x01)], &[]),
    ]);
    std::fs::write(&path, &data).unwrap();
    let first = auto_decode(&path).unwrap();
    let second = auto_decode_variant(&path, 1).unwrap();
    let all = auto_decode_all(&path).unwrap().unwrap();
    let hinted = auto_decode_bytes("x/a01_all.ssq", &data).unwrap();
    let missing = auto_decode_variant(&path, 2);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(
        (first.key_count, first.metadata.title.as_str()),
        (4, "a01_all")
    );
    assert_eq!((second.key_count, second.notes.len()), (8, 3));
    assert_eq!(hinted.metadata.title, "a01_all");
    assert_eq!(
        all.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
        ["Single Expert", "Double Challenge"]
    );
    assert_eq!(all[1].chart.as_ref().unwrap().notes, second.notes);
    assert!(missing.is_err());
}

// ---------------------------------------------------------------------------------------------------- DDR STEP

/// A STEP chunk: `{i32 length, body}` with the given `i32` words.
fn step_chunk(words: &[i32]) -> Vec<u8> {
    let mut out = i32::try_from(4 + 4 * words.len()).unwrap().to_le_bytes().to_vec();
    out.extend(words.iter().flat_map(|word| word.to_le_bytes()));
    out
}

#[test]
fn a_step_file_is_one_chart_per_chunk_with_its_panels_choosing_the_layout() {
    use super::{StepDecoder, looks_like_step};
    // Tempo: tick 0 at 0 s, tick 8192 (two measures) at 4 s in 1/75 s: 120 BPM, a measure is 2 s.
    let tempo = step_chunk(&[0, 0, 8192, 300]);
    // A single chart is written for both players (each nibble a panel): left then right, Maniac (code 0x0200).
    let single = step_chunk(&[0x0200, 0, 0x0001_0001, 2048, 0x1000_1000, 3072, -1]);
    // A Solo-mix 3-panel chart (up-left, down, up-right: panel bits 4, 1, 6): Basic.
    let solo3 = step_chunk(&[0x0000, 0, 0x0001_0000, 1024, 0x0000_0010, 2048, 0x0100_0000]);
    // A couple chart (code 1, second player's panels): skipped.
    let couple = step_chunk(&[0x0001, 0, 0x0010_0001]);
    let file = [tempo, single, solo3, couple, vec![0; 4]].concat();
    assert!(looks_like_step(&file));
    assert!(!looks_like_step(b"#TITLE:pydance step;\n"));
    let variants = StepDecoder::variants(&file).unwrap();
    let names: Vec<(String, u8)> = variants.iter().map(|v| (v.name(), v.key_count)).collect();
    assert_eq!(names, vec![("Single Maniac".to_string(), 4), ("Solo 3 Basic".to_string(), 3)]);
    let notes = |index| -> Vec<(i64, u8)> {
        StepDecoder::decode_with_title(&file, index, "t").unwrap().notes.iter().map(|n| (n.time_us, n.column)).collect()
    };
    assert_eq!(notes(0), vec![(0, 0), (1_000_000, 3)]);
    assert_eq!(notes(1), vec![(0, 0), (500_000, 1), (1_000_000, 2)]);
    let chart = StepDecoder::decode_with_title(&file, 0, "t").unwrap();
    assert_eq!(chart.timing_points, vec![TimingPoint::bpm(0, 120.0)]);
}
