//! Tests of the `.dtx` decoder. Every file here is written by hand from the documented syntax (the community charts have no
//! licence and are not in the repository); the expected times are computed by hand at 120 BPM, where a measure lasts 2 s.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap
)]

use rox::audio_plan::AudioPlan;
use rox::codec::Decoder;
use rox::error::RoxError;
use rox::model::RoxChart;

use super::channels::{BLUE, GREEN, OPEN, PINK, RED, Role, YELLOW, classify};
use super::parser::real;
use super::{DtxDecoder, DtxReport, Part};

/// One measure at 120 BPM.
const M: i64 = 2_000_000;

fn file(lines: &[&str]) -> Vec<u8> {
    lines.join("\n").into_bytes()
}

/// A drum chart at 120 BPM with `lines` after the header.
fn drums(lines: &[&str]) -> Vec<u8> {
    file(&[&["#TITLE: T", "#BPM: 120"][..], lines].concat())
}

fn decode(lines: &[&str]) -> (RoxChart, DtxReport, AudioPlan) {
    DtxDecoder::decode_with_audio_plan(&drums(lines), 0).unwrap()
}

fn decode_part(lines: &[&str], part: Part) -> (RoxChart, DtxReport, AudioPlan) {
    let data = drums(lines);
    let index = DtxDecoder::variants(&data)
        .unwrap()
        .iter()
        .position(|v| v.part == part)
        .unwrap();
    DtxDecoder::decode_with_audio_plan(&data, index).unwrap()
}

/// (time, column) of every note, in chart order.
fn notes(chart: &RoxChart) -> Vec<(i64, u8)> {
    chart.notes.iter().map(|n| (n.time_us, n.column)).collect()
}

fn events(plan: &AudioPlan) -> Vec<(i64, &str, Option<u8>)> {
    plan.events
        .iter()
        .map(|e| (e.time_us, e.file.as_str(), e.volume))
        .collect()
}

// ------------------------------------------------------------------------------------------------------- text and lines

#[test]
fn a_header_is_read_in_every_form() {
    for form in [
        "#TITLE: A",
        "#TITLE:A",
        "#TITLE A",
        "#title: A",
        "#TiTlE:\tA",
        "#TITLEA",
        "  #TITLE: A ; a comment",
        "#TITLE:   A   ;",
    ] {
        let data = file(&[form, "#00112: 01"]);
        let chart = DtxDecoder::decode_variant(&data, 0).unwrap();
        assert_eq!(chart.metadata.title, "A", "{form:?}");
    }
    // A command glued to its parameter gives the glued text in place of the parameter that follows (the game's rule).
    let chart = DtxDecoder::decode_variant(&file(&["#TITLEglued: lost", "#00112: 01"]), 0).unwrap();
    assert_eq!(chart.metadata.title, "glued");
    // Comments: a line that does not start with `#`, and `;` inside a line.
    let data = file(&[
        "; #TITLE: no",
        "TITLE: no",
        "#ARTIST: B ; #TITLE: no",
        "#GENRE: Metal",
        "#00112: 01",
    ]);
    let chart = DtxDecoder::decode_variant(&data, 0).unwrap();
    assert_eq!(
        (
            chart.metadata.title.as_str(),
            chart.metadata.artist.as_str()
        ),
        ("", "B")
    );
    assert_eq!(chart.metadata.genre.as_deref(), Some("Metal"));
    // The last header wins.
    let chart = DtxDecoder::decode_variant(&file(&["#TITLE: one", "#TITLE: two", "#00112: 01"]), 0)
        .unwrap();
    assert_eq!(chart.metadata.title, "two");
}

#[test]
fn line_endings_and_a_byte_order_mark() {
    for data in [
        b"#TITLE: A\r\n#00112: 01\r\n".to_vec(),
        b"#TITLE: A\r#00112: 01\r".to_vec(),
        b"\xEF\xBB\xBF#TITLE: A\n#00112: 01".to_vec(),
        b"#TITLE: A\n\n\n   \n#00112: 01\n\n".to_vec(),
    ] {
        let (chart, report) = DtxDecoder::decode_with_report(&data, 0).unwrap();
        assert_eq!(chart.metadata.title, "A");
        assert_eq!(notes(&chart), [(M, 0)]);
        assert_eq!(report.charset, "UTF-8");
    }
}

#[test]
fn a_data_line_takes_an_optional_space_and_any_case_of_channel() {
    for form in [
        "#00112: 01",
        "#00112:01",
        "#00112 01",
        "#00112:\t01",
        "#00112:   01 ; tail",
        "  #00112: 01",
    ] {
        let (chart, _, _) = decode(&[form]);
        assert_eq!(notes(&chart), [(M, 0)], "{form:?}");
    }
    // A hexadecimal channel in lower case: 1a is the left cymbal.
    let (chart, report, _) = decode(&["#0011a: 01", "#00212: 01"]);
    assert_eq!(notes(&chart), [(M, 0), (2 * M, 1)]);
    assert_eq!(report.layout, "Drums 2 columns: LC SD");
}

#[test]
fn measure_numbers_are_base_36_in_the_hundreds() {
    // `A00` is 1000, `B00` 1100, `Z99` 3599; lower case is the same digit.
    let (chart, _, _) = decode(&["#00012: 01", "#A0012: 01", "#b0012: 01", "#Z9912: 01"]);
    assert_eq!(
        notes(&chart),
        [
            (0, 0),
            (2_000 * 1_000_000, 0),
            (2_200 * 1_000_000, 0),
            (7_198 * 1_000_000, 0)
        ]
    );
    // `#0A012` is not a data line (the second character must be a decimal digit), `#1000012` neither (not five characters).
    let (chart, report, _) = decode(&["#00012: 01", "#0A012: 01", "#100012: 01"]);
    assert_eq!(notes(&chart), [(0, 0)]);
    assert_eq!(report.unknown_chips, 0);
}

#[test]
fn data_lines_with_odd_lengths_underscores_and_bad_characters() {
    // The odd last character is dropped and counted; `_` is ignored (so `0_1_00_01` is three slots: 01 00 01, at ticks 0 and 256).
    let (chart, report, _) = decode(&[
        "#00012: 010",
        "#00112: 0_1_00_01",
        "#00212: 01!1",
        "#00312: 0é",
    ]);
    assert_eq!(notes(&chart), [(0, 0), (M, 0), (M + 1_333_333, 0)]);
    assert_eq!(report.odd_data_lines, 1);
    assert_eq!(report.invalid_data_lines, 2);
    // An empty parameter is nothing.
    let (chart, report, _) = decode(&["#00012:", "#00112: ;", "#00212: 01"]);
    assert_eq!(notes(&chart), [(2 * M, 0)]);
    assert_eq!(report.invalid_data_lines, 0);
}

#[test]
fn an_object_is_placed_by_integer_division_of_the_384_ticks() {
    // Five slots: ticks 0, 76, 153, 230, 307 (384 * i / 5, rounded down), not the exact fifths of the measure.
    let (chart, _, _) = decode(&["#00012: 0101010101"]);
    let expected: Vec<i64> = [0i64, 76, 153, 230, 307]
        .iter()
        .map(|tick| ((*tick as f64) / 384.0 * 4.0 * 0.5 * 1_000_000.0).round() as i64)
        .collect();
    assert_eq!(
        chart.notes.iter().map(|n| n.time_us).collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        expected[1], 395_833,
        "tick 76 is 0.3958 s, the exact fifth would be 0.4 s"
    );
    assert_eq!(expected[4], 1_598_958);
    // Quarters and eighths are exact.
    let (chart, _, _) = decode(&["#00012: 01000100"]);
    assert_eq!(
        chart.notes.iter().map(|n| n.time_us).collect::<Vec<_>>(),
        [0, M / 2]
    );
}

#[test]
fn lines_of_one_channel_and_measure_are_not_merged() {
    // Three lines for the same lane and measure: three chips; two at one time in one column keep one note, the other still sounds.
    let (chart, report, plan) = decode(&[
        "#WAV01: a.wav",
        "#WAV02: b.wav",
        "#00012: 0100",
        "#00012: 0200",
        "#00012: 0002",
    ]);
    assert_eq!(notes(&chart), [(0, 0), (M / 2, 0)]);
    assert_eq!(report.overlapping_notes_dropped, 1);
    assert_eq!(plan.events.len(), 3);
}

// -------------------------------------------------------------------------------------------------------------- time

#[test]
fn a_measure_length_holds_until_the_next_channel_02_line() {
    // Measure 1 is half as long and so are the measures after it, until measure 4 says 1 again. The comma is a decimal point.
    let (chart, _, _) = decode(&[
        "#00102: 0,5",
        "#00402: 1",
        "#00012: 01",
        "#00112: 01",
        "#00212: 01",
        "#00312: 01",
        "#00412: 01",
        "#00512: 01",
        "#00212: 0001",
    ]);
    assert_eq!(
        chart.notes.iter().map(|n| n.time_us).collect::<Vec<_>>(),
        [
            0, 2_000_000, 3_000_000, 3_500_000, 4_000_000, 5_000_000, 7_000_000
        ]
    );
}

#[test]
fn a_bad_measure_length_is_counted_and_ignored() {
    for bad in ["0", "-1", "abc", "", "0,0"] {
        let line = format!("#00102: {bad}");
        let (chart, report, _) = decode(&[line.as_str(), "#00212: 01"]);
        assert_eq!(notes(&chart), [(2 * M, 0)], "{bad:?}");
        assert_eq!(report.ignored_bar_lengths, 1, "{bad:?}");
    }
}

#[test]
fn the_tempo_comes_from_bpm_the_base_bpm_and_the_two_change_channels() {
    // Initial tempo: `#BPM` (the first `#BPM00`), 120 when there is none, never plus `#BASEBPM`.
    let at = |lines: &[&str]| -> i64 { notes_of(&file(lines))[0].0 };
    assert_eq!(at(&["#BPM: 60", "#00112: 01"]), 2 * M);
    assert_eq!(at(&["#00112: 01"]), M);
    assert_eq!(at(&["#BASEBPM: 60", "#BPM: 120", "#00112: 01"]), M);
    assert_eq!(
        at(&["#BPM00: 60", "#BPM: 240", "#00112: 01"]),
        2 * M,
        "the first one wins"
    );
    assert_eq!(at(&["#BPM: 0", "#00112: 01"]), M, "not positive: 120");
    assert_eq!(at(&["#BPM: -5", "#00112: 01"]), M);
    assert_eq!(at(&["#BPM: 60,5", "#00112: 01"]), 3_966_942); // 4 * 60 / 60.5 s
    assert_eq!(
        at(&["#BPM: 1e999", "#00112: 01"]),
        240_000_000,
        "the number ends at the `e`: 1 BPM"
    );

    // Channel 08: the table value, plus #BASEBPM unless it is #BPM00.
    let (chart, report) =
        DtxDecoder::decode_with_report(&drums(&["#BPM01: 240", "#00108: 01", "#00212: 01"]), 0)
            .unwrap();
    assert_eq!(bpms(&chart), [(0, 120.0), (M, 240.0)]);
    assert_eq!(
        notes(&chart),
        [(M + M / 2, 0)],
        "measure 2 starts 2 s + 1 s"
    );
    assert_eq!(report.ignored_bpm_changes, 0);
    let (chart, _) = DtxDecoder::decode_with_report(
        &drums(&["#BASEBPM: 60", "#BPM01: 120", "#00108: 01", "#00212: 01"]),
        0,
    )
    .unwrap();
    assert_eq!(bpms(&chart), [(0, 120.0), (M, 180.0)]);
    assert_eq!(
        notes(&chart)[0].0,
        M + 4 * 60 * 1_000_000 / 180,
        "4 beats at 180 BPM"
    );

    // Channel 03: hexadecimal, plus #BASEBPM.
    let (chart, _) =
        DtxDecoder::decode_with_report(&drums(&["#BASEBPM: 60", "#00103: 78", "#00212: 01"]), 0)
            .unwrap();
    assert_eq!(bpms(&chart), [(0, 120.0), (M, 180.0)]);
    assert_eq!(notes(&chart), [(M + 1_333_333, 0)]);
    let (chart, _) =
        DtxDecoder::decode_with_report(&drums(&["#00103: 3c", "#00212: 01"]), 0).unwrap();
    assert_eq!(bpms(&chart), [(0, 120.0), (M, 60.0)]);

    // At one position channel 08 comes after channel 03: it wins. A change to the same BPM is no new point.
    let (chart, _) = DtxDecoder::decode_with_report(
        &drums(&["#BPM01: 240", "#00103: 78", "#00108: 01", "#00212: 01"]),
        0,
    )
    .unwrap();
    assert_eq!(bpms(&chart), [(0, 120.0), (M, 240.0)]);
    let (chart, _) =
        DtxDecoder::decode_with_report(&drums(&["#00103: 78", "#00212: 01"]), 0).unwrap();
    assert_eq!(bpms(&chart), [(0, 120.0)]);

    // A change in the middle of a measure: 120 BPM for the first half (1 s), then 240.
    let (chart, _) =
        DtxDecoder::decode_with_report(&drums(&["#BPM01: 240", "#00008: 0001", "#00112: 01"]), 0)
            .unwrap();
    assert_eq!(bpms(&chart), [(0, 120.0), (1_000_000, 240.0)]);
    assert_eq!(
        notes(&chart),
        [(1_500_000, 0)],
        "1 s at 120, then 2 beats at 240"
    );
}

fn notes_of(data: &[u8]) -> Vec<(i64, u8)> {
    notes(&DtxDecoder::decode_variant(data, 0).unwrap())
}

fn bpms(chart: &RoxChart) -> Vec<(i64, f32)> {
    chart
        .timing_points
        .iter()
        .filter_map(|t| t.bpm_value().map(|b| (t.time_us(), b)))
        .collect()
}

#[test]
fn unusable_tempo_changes_are_counted() {
    // Channel 08 object with no `#BPMzz`, channel 03 pair that is not hexadecimal: ignored, counted.
    let (chart, report) = DtxDecoder::decode_with_report(
        &drums(&["#00108: 05", "#00203: ZZ", "#00303: 0G", "#00412: 01"]),
        0,
    )
    .unwrap();
    assert_eq!(bpms(&chart), [(0, 120.0)]);
    assert_eq!(report.ignored_bpm_changes, 3);
    // A `#BPMzz` that is not positive is not defined.
    let (_, report) = DtxDecoder::decode_with_report(
        &drums(&["#BPM01: -3", "#BPM02: 0", "#00108: 0102", "#00412: 01"]),
        0,
    )
    .unwrap();
    assert_eq!(report.ignored_bpm_changes, 2);
}

#[test]
fn tables_can_be_used_before_they_are_defined_and_defined_again() {
    // A chip before every definition takes the first one; after one, the latest one before it.
    let (_, _, plan) = decode(&[
        "#00012: 01",
        "#WAV01: first.wav",
        "#00112: 01",
        "#WAV01: second.wav",
        "#00212: 01",
    ]);
    assert_eq!(
        events(&plan),
        [
            (0, "first.wav", None),
            (M, "first.wav", None),
            (2 * M, "second.wav", None)
        ]
    );
    // The same for the BPM table: m1 uses BPM01 = 240, then it is redefined to 60 for the chip of m2.
    let (chart, _) = DtxDecoder::decode_with_report(
        &drums(&[
            "#00108: 01",
            "#BPM01: 240",
            "#BPM01: 60",
            "#00208: 01",
            "#00312: 01",
        ]),
        0,
    )
    .unwrap();
    assert_eq!(
        bpms(&chart),
        [(0, 120.0), (2_000_000, 240.0), (3_000_000, 60.0)]
    );
}

#[test]
fn a_volume_applies_to_the_wav_it_precedes_and_to_the_ones_after_it() {
    let (_, _, plan) = decode(&[
        "#WAV01: a.wav",
        "#00012: 01",
        "#VOLUME01: 40",
        "#WAV01: b.wav",
        "#00112: 01",
        "#VOLUME01: 10",
        "#WAV01: f.wav",
        "#00212: 01",
        "#WAV02: c.wav",
        "#WAVVOL02: 150",
        "#00312: 02",
        "#WAV03: d.wav",
        "#VOLUME03: -5",
        "#00412: 03",
        "#WAV04: e.wav",
        "#00512: 04",
    ]);
    // a.wav precedes the first #VOLUME01: it takes it (40); b.wav follows it (40); f.wav follows the second one (10); c.wav is clamped
    // to 100 (no gain), d.wav to 0 (a volume line below the wav defines it), e.wav has none.
    assert_eq!(
        events(&plan),
        [
            (0, "a.wav", Some(40)),
            (M, "b.wav", Some(40)),
            (2 * M, "f.wav", Some(10)),
            (3 * M, "c.wav", None),
            (4 * M, "d.wav", Some(0)),
            (5 * M, "e.wav", None),
        ]
    );
}

// ------------------------------------------------------------------------------------------------------------ drums

#[test]
fn every_drum_channel_has_its_column_and_the_left_pedal_folds_onto_the_bass_drum() {
    // Channel 11+k at measure k.
    let lines: Vec<String> = (0..12)
        .map(|k| format!("#{k:03}{:02X}: 01", 0x11 + k))
        .collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let (chart, report, _) = decode(&refs);
    assert_eq!(chart.key_count, 9);
    assert_eq!(report.layout, "Drums 9 columns: LC HH SD HT BD LT FT CY RD");
    // HH SD BD HT LT CY FT HHO RD LC LP LBD
    let columns = [1u8, 2, 4, 3, 5, 7, 6, 1, 8, 0, 4, 4];
    assert_eq!(
        notes(&chart),
        columns
            .iter()
            .enumerate()
            .map(|(k, c)| (k as i64 * M, *c))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        (
            report.hi_hat_open_notes,
            report.left_pedal_notes_folded,
            report.overlapping_notes_dropped
        ),
        (1, 2, 0)
    );
    assert_eq!(chart.metadata.difficulty_name, "Drums");
}

#[test]
fn a_drum_column_exists_only_when_the_chart_uses_it() {
    let (chart, report, _) = decode(&["#00011: 01", "#00112: 01", "#00213: 01", "#00316: 01"]);
    assert_eq!(
        (chart.key_count, report.layout.as_str()),
        (4, "Drums 4 columns: HH SD BD CY")
    );
    assert_eq!(notes(&chart), [(0, 0), (M, 1), (2 * M, 2), (3 * M, 3)]);
    let (chart, _, _) = decode(&["#00012: 01", "#00113: 01"]);
    assert_eq!(
        chart.key_count, 2,
        "fewer than the 4 the catalog needs: the library refuses it, the decoder does not"
    );
}

#[test]
fn both_pedals_at_one_time_are_one_note_and_both_sound() {
    let (chart, report, plan) = decode(&[
        "#WAV01: a.wav",
        "#WAV02: b.wav",
        "#00013: 01",
        "#0001C: 02",
        "#0011B: 02",
    ]);
    assert_eq!(notes(&chart), [(0, 0), (M, 0)]);
    assert_eq!(report.overlapping_notes_dropped, 1);
    assert_eq!(report.left_pedal_notes_folded, 2);
    assert_eq!(
        events(&plan),
        [(0, "a.wav", None), (0, "b.wav", None), (M, "b.wav", None)]
    );
    // The first note keeps its sound.
    assert_eq!(
        chart
            .hitsounds
            .iter()
            .map(|h| h.file.as_str())
            .collect::<Vec<_>>(),
        ["a.wav", "b.wav"]
    );
    assert_eq!(chart.notes[0].hitsound_index, Some(0));
}

// ------------------------------------------------------------------------------------------------- guitar and bass

/// The channels of the format notes with the chord each one names, from `EChannel.cs` (`R`, `G`, `B`, `Y`, `P`, `x` = not held).
const GUITAR: [(u8, &str); 31] = [
    (0x21, "xxBxx"),
    (0x22, "xGxxx"),
    (0x23, "xGBxx"),
    (0x24, "Rxxxx"),
    (0x25, "RxBxx"),
    (0x26, "RGxxx"),
    (0x27, "RGBxx"),
    (0x93, "xxxYx"),
    (0x94, "xxBYx"),
    (0x95, "xGxYx"),
    (0x96, "xGBYx"),
    (0x97, "RxxYx"),
    (0x98, "RxBYx"),
    (0x99, "RGxYx"),
    (0x9A, "RGBYx"),
    (0x9B, "xxxxP"),
    (0x9C, "xxBxP"),
    (0x9D, "xGxxP"),
    (0x9E, "xGBxP"),
    (0x9F, "RxxxP"),
    (0xA9, "RxBxP"),
    (0xAA, "RGxxP"),
    (0xAB, "RGBxP"),
    (0xAC, "xxxYP"),
    (0xAD, "xxBYP"),
    (0xAE, "xGxYP"),
    (0xAF, "xGBYP"),
    (0xD0, "RxxYP"),
    (0xD1, "RxBYP"),
    (0xD2, "RGxYP"),
    (0xD3, "RGBYP"),
];

const BASS: [(u8, &str); 31] = [
    (0xA1, "xxBxx"),
    (0xA2, "xGxxx"),
    (0xA3, "xGBxx"),
    (0xA4, "Rxxxx"),
    (0xA5, "RxBxx"),
    (0xA6, "RGxxx"),
    (0xA7, "RGBxx"),
    (0xC5, "xxxYx"),
    (0xC6, "xxBYx"),
    (0xC8, "xGxYx"),
    (0xC9, "xGBYx"),
    (0xCA, "RxxYx"),
    (0xCB, "RxBYx"),
    (0xCC, "RGxYx"),
    (0xCD, "RGBYx"),
    (0xCE, "xxxxP"),
    (0xCF, "xxBxP"),
    (0xDA, "xGxxP"),
    (0xDB, "xGBxP"),
    (0xDC, "RxxxP"),
    (0xDD, "RxBxP"),
    (0xDE, "RGxxP"),
    (0xDF, "RGBxP"),
    (0xE1, "xxxYP"),
    (0xE2, "xxBYP"),
    (0xE3, "xGxYP"),
    (0xE4, "xGBYP"),
    (0xE5, "RxxYP"),
    (0xE6, "RxBYP"),
    (0xE7, "RGxYP"),
    (0xE8, "RGBYP"),
];

fn mask(pattern: &str) -> u8 {
    pattern
        .chars()
        .zip([RED, GREEN, BLUE, YELLOW, PINK])
        .filter(|(c, _)| *c != 'x')
        .fold(0, |acc, (_, bit)| acc | bit)
}

#[test]
fn the_guitar_and_bass_channels_name_their_chords() {
    for (channel, pattern) in GUITAR {
        assert_eq!(
            classify(channel),
            Role::Fret {
                part: Part::Guitar,
                buttons: mask(pattern)
            },
            "{channel:02X}"
        );
    }
    for (channel, pattern) in BASS {
        assert_eq!(
            classify(channel),
            Role::Fret {
                part: Part::Bass,
                buttons: mask(pattern)
            },
            "{channel:02X}"
        );
    }
    assert_eq!(
        classify(0x20),
        Role::Fret {
            part: Part::Guitar,
            buttons: OPEN
        }
    );
    assert_eq!(
        classify(0xA0),
        Role::Fret {
            part: Part::Bass,
            buttons: OPEN
        }
    );
    // Nothing else is a fret chip, the 31 + open of each instrument are all different, and a chord is never empty.
    let frets = (0..=255u8)
        .filter(|c| matches!(classify(*c), Role::Fret { .. }))
        .count();
    assert_eq!(frets, 64);
    let all: std::collections::HashSet<_> = GUITAR
        .iter()
        .chain(BASS.iter())
        .map(|(_, p)| mask(p))
        .collect();
    assert_eq!(all.len(), 31);
}

fn rox_channel(lines: &[&str]) -> RoxChart {
    DtxDecoder::decode_variant(&file(&[&["#BPM: 120"][..], lines].concat()), 0).unwrap()
}

#[test]
fn a_guitar_chart_has_an_open_column_and_three_buttons() {
    // Open, red, a red-green chord, blue.
    let chart = rox_channel(&["#00020: 01", "#00124: 01", "#00226: 01", "#00321: 01"]);
    assert_eq!(chart.key_count, 4);
    assert_eq!(chart.metadata.difficulty_name, "Guitar");
    assert_eq!(
        notes(&chart),
        [(0, 0), (M, 1), (2 * M, 1), (2 * M, 2), (3 * M, 3)]
    );
}

#[test]
fn yellow_or_pink_makes_it_a_five_button_chart_of_six_columns() {
    let (chart, report, _) = decode(&["#00020: 01", "#00193: 01", "#0029B: 01", "#003D3: 01"]);
    assert_eq!(
        (chart.key_count, report.layout.as_str()),
        (6, "Guitar 6 columns: Open R G B Y P")
    );
    // Y alone; P alone; RGBYP.
    assert_eq!(
        notes(&chart),
        [
            (0, 0),
            (M, 4),
            (2 * M, 5),
            (3 * M, 1),
            (3 * M, 2),
            (3 * M, 3),
            (3 * M, 4),
            (3 * M, 5)
        ]
    );
    // The bass has its own channels and its own chart in the same file.
    let data = file(&["#BPM: 120", "#00024: 01", "#001A4: 01", "#002E8: 01"]);
    let variants = DtxDecoder::variants(&data).unwrap();
    assert_eq!(
        variants
            .iter()
            .map(|v| (v.part, v.key_count, v.notes))
            .collect::<Vec<_>>(),
        [(Part::Guitar, 4, 1), (Part::Bass, 6, 6)]
    );
    let bass = DtxDecoder::decode_variant(&data, 1).unwrap();
    assert_eq!(bass.metadata.difficulty_name, "Bass");
    assert_eq!(notes(&bass)[0], (M, 1));
}

#[test]
fn holds_are_paired_as_the_game_pairs_them() {
    // A marker on a button chip starts a hold, the next marker ends it: red held from measure 0 to measure 1.
    let chart = rox_channel(&["#00024: 01", "#0002C: 01", "#0012C: 01"]);
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(
        (
            chart.notes[0].time_us,
            chart.notes[0].duration_us(),
            chart.notes[0].column
        ),
        (0, M, 1)
    );
    // A chord held is one hold per button.
    let chart = rox_channel(&["#00027: 01", "#0002C: 01", "#0012C: 01"]);
    assert_eq!(
        chart
            .notes
            .iter()
            .map(|n| (n.column, n.duration_us()))
            .collect::<Vec<_>>(),
        [(1, M), (2, M), (3, M)]
    );
    // Two holds one after the other.
    let chart = rox_channel(&[
        "#00024: 01",
        "#00222: 01",
        "#0002C: 01",
        "#0012C: 01",
        "#0022C: 01",
        "#0032C: 01",
    ]);
    assert_eq!(
        chart
            .notes
            .iter()
            .map(|n| (n.time_us, n.column, n.duration_us()))
            .collect::<Vec<_>>(),
        [(0, 1, M), (2 * M, 2, M)]
    );
    // The bass uses channel 2D.
    let data = file(&["#BPM: 120", "#000A4: 01", "#0002D: 01", "#0012D: 01"]);
    let bass = DtxDecoder::decode_variant(&data, 0).unwrap();
    assert_eq!(bass.metadata.difficulty_name, "Bass");
    assert_eq!(bass.notes.len(), 1);
    assert_eq!(bass.notes[0].duration_us(), M);
}

#[test]
fn a_broken_hold_is_dropped_and_counted() {
    let report_of = |lines: &[&str]| -> (usize, usize, usize) {
        let (chart, report, _) = decode(&[&["#00020: 01"][..], lines].concat());
        (
            chart.notes.iter().filter(|n| n.is_hold()).count(),
            report.hold_markers_dropped,
            chart.notes.len(),
        )
    };
    // No chip at the marker: both markers are unused.
    assert_eq!(report_of(&["#0012C: 01", "#0022C: 01"]), (0, 2, 1));
    // Only an open chip there: it is not a button chip.
    assert_eq!(
        report_of(&["#0012C: 01", "#0022C: 01", "#00120: 01"]),
        (0, 2, 2)
    );
    // A chip between the start and the end: no hold, and the chip stays a tap.
    assert_eq!(
        report_of(&["#00124: 01", "#0012C: 01", "#00222: 01", "#0032C: 01"]),
        (0, 2, 3)
    );
    // A chip exactly at the end breaks it too.
    assert_eq!(
        report_of(&["#00124: 01", "#0012C: 01", "#00222: 01", "#0022C: 01"]),
        (0, 2, 3)
    );
    // The end of a pair that was broken is used up: the next marker starts a new pair (here on a red chip, then closed).
    assert_eq!(
        report_of(&[
            "#00124: 01",
            "#0012C: 01",
            "#00222: 01",
            "#0022C: 01",
            "#00324: 01",
            "#0032C: 01",
            "#0042C: 01"
        ]),
        (1, 2, 4)
    );
    // A lone marker at the end of the file.
    assert_eq!(report_of(&["#00124: 01", "#0012C: 01"]), (0, 1, 2));
}

// -------------------------------------------------------------------------------------- dropped chips and the counters

#[test]
fn what_the_decoder_drops_is_counted_by_name() {
    let (chart, report, plan) = decode(&[
        "#WAV01: a.wav",
        "#00012: 01",
        "#00031: 0101", // hidden drums (2)
        "#000B1: 01",   // blank hit
        "#00028: 01",   // guitar wailing
        "#0002F: 01",   // wailing sound
        "#000A8: 01",   // bass wailing
        "#0001F: 01",   // fill-in
        "#00053: 01",   // fill-in
        "#00004: 01",   // BGA
        "#00054: 01",   // movie
        "#00050: 01",   // bar line
        "#0004F: 01",   // bonus
        "#00052: 01",   // MIDI chorus
        "#0000A: 01",   // reserved
        "#0001D: 01",   // not a channel of the table
        "#000FF: 0101", // neither
    ]);
    assert_eq!(notes(&chart), [(0, 0)]);
    assert_eq!(report.hidden_chips, 2);
    assert_eq!(report.blank_hit_sounds, 1);
    assert_eq!(report.wailing_chips, 3);
    assert_eq!(report.fill_in_chips, 2);
    assert_eq!(report.visual_chips, 3);
    assert_eq!(report.inert_chips, 3);
    assert_eq!(report.unknown_chips, 3);
    assert!(plan.events.len() == 1, "only the snare sounds");
}

#[test]
fn the_chips_of_the_other_instruments_are_counted_not_played() {
    let lines = [
        "#WAV01: a.wav",
        "#WAV02: b.wav",
        "#00012: 01",
        "#00024: 02",
        "#000A4: 01",
    ];
    let (drum, drum_report, drum_plan) = decode_part(&lines, Part::Drums);
    assert_eq!((drum.key_count, drum_report.other_instrument_chips), (1, 2));
    assert_eq!(events(&drum_plan), [(0, "a.wav", None)]);
    let (guitar, guitar_report, guitar_plan) = decode_part(&lines, Part::Guitar);
    assert_eq!(
        (guitar.key_count, guitar_report.other_instrument_chips),
        (4, 2)
    );
    assert_eq!(events(&guitar_plan), [(0, "b.wav", None)]);
    let (_, bass_report, bass_plan) = decode_part(&lines, Part::Bass);
    assert_eq!(bass_report.other_instrument_chips, 2);
    assert_eq!(events(&bass_plan), [(0, "a.wav", None)]);
}

// ------------------------------------------------------------------------------------------------------------ sounds

#[test]
fn the_audio_plan_holds_the_background_sounds_and_every_chip_of_the_instrument() {
    let (chart, report, plan) = decode(&[
        "#WAV01: bgm.ogg",
        "#WAV02: snare.wav",
        "#WAV03: kick.wav",
        "#VOLUME02: 50",
        "#WAVVOL03: 80",
        "#00101: 01", // background sound at measure 1
        "#00112: 02", // snare
        "#0021B: 03", // left pedal: folded onto the bass drum, still a sound
        "#00061: 01", // an effect channel (SE01), played by itself
        "#00031: 02", // hidden: no sound
        "#000B2: 02", // blank hit: no sound
        "#00024: 02", // guitar chip: not in the drum plan
    ]);
    assert_eq!(
        events(&plan),
        [
            (0, "bgm.ogg", None),
            (M, "bgm.ogg", None),
            (M, "snare.wav", Some(50)),
            (2 * M, "kick.wav", Some(80))
        ]
    );
    assert_eq!(report.background_sounds, 2);
    assert!(!report.single_audio);
    assert_eq!(chart.metadata.audio_file, "");
    // The notes carry their sound in the hitsound table.
    assert_eq!(
        chart
            .notes
            .iter()
            .map(|n| n.hitsound_index)
            .collect::<Vec<_>>(),
        [Some(0), Some(1)]
    );
    assert_eq!(chart.hitsounds[0].volume, Some(50));
    assert_eq!(chart.hitsounds[1].volume, Some(80));
    // The effect channels are all background sounds.
    for channel in [0x61, 0x69, 0x70, 0x79, 0x80, 0x89, 0x90, 0x92] {
        let line = format!("#001{channel:02X}: 01");
        let (_, report, plan) = decode(&["#WAV01: fx.wav", "#00012: 09", &line]);
        assert_eq!(
            (report.background_sounds, plan.events.len()),
            (1, 1),
            "{channel:02X}"
        );
    }
    for channel in [0x6A, 0x6F, 0x7A, 0x7F, 0x8A, 0x8F, 0x60, 0x93] {
        let line = format!("#001{channel:02X}: 01");
        let (_, report, _) = decode(&["#WAV01: fx.wav", "#00012: 09", &line]);
        assert_eq!(report.background_sounds, 0, "{channel:02X}");
    }
}

#[test]
fn a_chart_with_one_background_sound_and_no_other_sound_uses_it_as_its_audio() {
    // One BGM at measure 2 (4 s), a snare at measure 3 whose `#WAV02` is not defined: the audio file is the BGM and time 0 is its start.
    let (chart, report, plan) = decode(&["#WAV01: bgm.ogg", "#00201: 01", "#00312: 02"]);
    assert!(report.single_audio);
    assert_eq!(chart.metadata.audio_file, "bgm.ogg");
    assert_eq!(notes(&chart), [(M, 0)], "6 s - 4 s");
    assert_eq!(bpms(&chart), [(-2 * M, 120.0)]);
    assert_eq!(events(&plan), [(0, "bgm.ogg", None)]);
}

#[test]
fn a_song_with_samples_is_rendered_not_played_from_its_backing_track() {
    // The usual DTX: a backing track on channel 01 and one sample per drum chip. The BGM alone is not the audio.
    let (chart, report, plan) = decode(&[
        "#WAV01: bgm.ogg",
        "#WAV02: snare.wav",
        "#00001: 01",
        "#00012: 02",
    ]);
    assert!(!report.single_audio);
    assert_eq!(chart.metadata.audio_file, "");
    assert_eq!(
        events(&plan),
        [(0, "bgm.ogg", None), (0, "snare.wav", None)]
    );
    assert_eq!(notes(&chart), [(0, 0)], "no shift");
    // Two background sounds are never the audio, a BGM chip whose `#WAV` is not defined is no sound, a chart with none has none.
    let (chart, report, _) = decode(&["#WAV01: a.ogg", "#00001: 01", "#00101: 01", "#00212: 09"]);
    assert_eq!(
        (
            chart.metadata.audio_file.as_str(),
            report.background_sounds,
            report.single_audio
        ),
        ("", 2, false)
    );
    let (chart, report, plan) = decode(&["#00001: 05", "#00112: 09"]);
    assert_eq!(
        (chart.metadata.audio_file.as_str(), report.background_sounds),
        ("", 0)
    );
    assert!(plan.is_empty());
    // A BGM with an effect sound is two background sounds.
    let (_, report, _) = decode(&["#WAV01: a.ogg", "#00001: 01", "#00061: 01", "#00112: 09"]);
    assert_eq!((report.background_sounds, report.single_audio), (2, false));
}

#[test]
fn file_names_are_made_safe_and_follow_path() {
    let (_, report, plan) = decode(&[
        "#PATH: wav/",
        "#PATH_WAV: D:\\Games\\DTX\\",
        "#WAV01: a.wav",
        "#WAV02: ..\\x.wav",
        "#WAV03: C:\\x.wav",
        "#WAV04: /abs.wav",
        "#WAV05: sub\\b.wav",
        "#00012: 0102030405",
    ]);
    assert_eq!(
        events(&plan),
        [(0, "wav/a.wav", None), (1_598_958, "wav/sub/b.wav", None)]
    );
    assert_eq!(plan.files(), ["wav/a.wav", "wav/sub/b.wav"]);
    assert_eq!(report.unsafe_names, 3);
    assert!(report.path_wav_ignored);
    // Without `#PATH` the names are as written, `\` becomes `/`.
    let (_, _, plan) = decode(&["#WAV01: one\\two.wav", "#00012: 01"]);
    assert_eq!(plan.files(), ["one/two.wav"]);
    // The images the chart names are made safe too.
    let (chart, report, _) = decode(&[
        "#BACKGROUND: ..\\bg.png",
        "#STAGEFILE: stage.png",
        "#00012: 01",
    ]);
    assert_eq!(chart.metadata.background_file.as_deref(), Some("stage.png"));
    assert_eq!(report.unsafe_names, 1);
}

#[test]
fn the_background_image_follows_the_games_order() {
    let bg = |lines: &[&str], part: Part| -> Option<String> {
        let (chart, _, _) = decode_part(&[&["#00012: 01", "#00024: 01"][..], lines].concat(), part);
        chart.metadata.background_file.map(|n| n.to_string())
    };
    assert_eq!(bg(&[], Part::Drums), None);
    assert_eq!(
        bg(&["#PREIMAGE: pre.jpg"], Part::Drums).as_deref(),
        Some("pre.jpg")
    );
    assert_eq!(
        bg(
            &["#PREIMAGE: pre.jpg", "#STAGEFILE: stage.png"],
            Part::Drums
        )
        .as_deref(),
        Some("stage.png")
    );
    assert_eq!(
        bg(
            &[
                "#STAGEFILE: stage.png",
                "#WALL: wall.png",
                "#BACKGROUND_GR: gr.png"
            ],
            Part::Drums
        )
        .as_deref(),
        Some("wall.png")
    );
    assert_eq!(
        bg(
            &["#BACKGROUND: bg.png", "#BACKGROUND_GR: gr.png"],
            Part::Drums
        )
        .as_deref(),
        Some("bg.png")
    );
    assert_eq!(
        bg(
            &["#BACKGROUND: bg.png", "#BACKGROUND_GR: gr.png"],
            Part::Guitar
        )
        .as_deref(),
        Some("gr.png")
    );
    assert_eq!(
        bg(&["#BACKGROUND_GR: gr.png"], Part::Drums).as_deref(),
        Some("gr.png")
    );
}

// -------------------------------------------------------------------------------------------------- control flow

#[test]
fn random_and_if_are_reproducible() {
    let (chart, report, _) = decode(&[
        "#IF 1",
        "#00012: 01",
        "#ENDIF",
        "#RANDOM 5",
        "#IF 2",
        "#00112: 01",
        "#ENDIF",
        "#IF 1",
        "#00212: 01",
        "#IF 1",
        "#00312: 01",
        "#ENDIF",
        "#ENDIF",
        "#IF 9",
        "#IF 1",
        "#00412: 01",
        "#ENDIF",
        "#ENDIF",
        "#ENDIF",
        "#00512: 01",
    ]);
    // No #RANDOM yet (0) skips `#IF 1`; after `#RANDOM 5` the draw is 1; a nested block of a skipped block is skipped; a stray #ENDIF is ignored.
    assert_eq!(notes(&chart), [(2 * M, 0), (3 * M, 0), (5 * M, 0)]);
    assert_eq!(report.branches_skipped, 3);
    // An unreadable `#IF` is 1.
    let (chart, _, _) = decode(&[
        "#RANDOM 2",
        "#IF x",
        "#00012: 01",
        "#ENDIF",
        "#RANDOM2",
        "#IF 1",
        "#00112: 01",
        "#ENDIF",
    ]);
    assert_eq!(notes(&chart), [(0, 0), (M, 0)]);
}

// ------------------------------------------------------------------------------------------------------------ levels

#[test]
fn the_level_is_shown_as_the_game_shows_it() {
    let level = |lines: &[&str]| -> Option<f32> {
        let (chart, _, _) = decode(&[&["#00012: 01"][..], lines].concat());
        chart.metadata.difficulty_value
    };
    assert_eq!(level(&["#DLEVEL: 76"]), Some(7.6));
    assert_eq!(level(&["#PLAYLEVEL: 50"]), Some(5.0));
    assert_eq!(level(&["#DLEVEL: 576"]), Some(5.7 + 0.06));
    assert_eq!(level(&["#DLEVEL: 576", "#DLVDEC: 4"]), Some(5.7 + 0.04));
    assert_eq!(level(&["#DLEVEL: 40", "#DLVDEC: 5"]), Some(4.0 + 0.05));
    assert_eq!(level(&["#DLEVEL: 9999"]), Some(10.0), "clamped to 1000");
    assert_eq!(level(&["#DLEVEL: 0"]), None);
    assert_eq!(level(&["#DLEVEL: abc"]), None);
    assert_eq!(level(&[]), None);
    assert_eq!(level(&["#DLEVEL: -3"]), None);
    // One level per instrument.
    let data = file(&[
        "#DLEVEL: 70",
        "#GLEVEL: 35",
        "#BLEVEL: 0",
        "#00012: 01",
        "#00024: 01",
        "#000A4: 01",
    ]);
    let variants = DtxDecoder::variants(&data).unwrap();
    assert_eq!(
        variants.iter().map(|v| v.level).collect::<Vec<_>>(),
        [Some(7.0), Some(3.5), None]
    );
}

// ------------------------------------------------------------------------------------------------- charset

#[test]
fn the_text_is_read_as_utf_8_utf_16_or_shift_jis() {
    // "テスト" in Shift_JIS.
    let mut data = b"#TITLE: ".to_vec();
    data.extend_from_slice(&[0x83, 0x65, 0x83, 0x58, 0x83, 0x67]);
    data.extend_from_slice(b"\n#00112: 01\n");
    let (chart, report) = DtxDecoder::decode_with_report(&data, 0).unwrap();
    assert_eq!(report.charset, "Shift_JIS");
    assert_eq!(chart.metadata.title, "テスト");
    // The same title in UTF-8.
    let data = "#TITLE: テスト\n#00112: 01\n".as_bytes();
    let (chart, report) = DtxDecoder::decode_with_report(data, 0).unwrap();
    assert_eq!(
        (report.charset, chart.metadata.title.as_str()),
        ("UTF-8", "テスト")
    );
    // UTF-16 with a byte order mark (some community files are).
    let mut data = vec![0xFF, 0xFE];
    for unit in "#TITLE: テスト\n#00112: 01\n".encode_utf16() {
        data.extend_from_slice(&unit.to_le_bytes());
    }
    let (chart, report) = DtxDecoder::decode_with_report(&data, 0).unwrap();
    assert_eq!(
        (report.charset, chart.metadata.title.as_str()),
        ("UTF-16LE", "テスト")
    );
}

// ------------------------------------------------------------------------------------------------------ ignored

#[test]
fn the_playback_speed_of_the_viewer_is_counted_and_changes_nothing() {
    let (chart, report, _) = decode(&["#DTXVPLAYSPEED: 2.0", "#00112: 01"]);
    assert!(report.dtxv_play_speed_ignored);
    assert_eq!(notes(&chart), [(M, 0)]);
    let (_, report, _) = decode(&["#DTXVPLAYSPEED: 0", "#00112: 01"]);
    assert!(!report.dtxv_play_speed_ignored);
}

#[test]
fn real_numbers_take_a_dot_or_a_comma() {
    for (text, expected) in [
        ("1.5", Some(1.5)),
        ("1,5", Some(1.5)),
        ("173", Some(173.0)),
        ("1.234,5", Some(1234.5)),
        ("1 000.5", Some(1000.5)),
        ("2.5abc", Some(2.5)),
        (".5", Some(0.5)),
        ("1e5", Some(1.0)),
        ("-1", None),
        ("+1", None),
        ("", None),
        (".", None),
        ("abc", None),
    ] {
        assert_eq!(real(text), expected, "{text:?}");
    }
    assert_eq!(real(&"9".repeat(400)), None, "infinite");
}

// ------------------------------------------------------------------------------------------------ variants and routing

fn three_instruments() -> Vec<u8> {
    file(&[
        "#TITLE: Band",
        "#BPM: 120",
        "#DLEVEL: 70",
        "#00012: 01",
        "#00113: 01",
        "#00024: 01",
        "#000A4: 01",
        "#001A4: 01",
    ])
}

#[test]
fn each_instrument_with_chips_is_a_variant_numbered_from_zero() {
    let data = three_instruments();
    let variants = DtxDecoder::variants(&data).unwrap();
    assert_eq!(
        variants
            .iter()
            .map(|v| (v.index, v.part, v.key_count, v.notes, v.holds))
            .collect::<Vec<_>>(),
        [
            (0, Part::Drums, 2, 2, 0),
            (1, Part::Guitar, 4, 1, 0),
            (2, Part::Bass, 4, 2, 0)
        ]
    );
    assert_eq!(variants[0].duration_us, M);
    assert_eq!(variants[0].level, Some(7.0));
    for (index, name) in ["Drums", "Guitar", "Bass"].iter().enumerate() {
        let chart = DtxDecoder::decode_variant(&data, index).unwrap();
        assert_eq!(chart.metadata.difficulty_name, *name);
        assert_eq!(chart.metadata.title, "Band");
    }
    assert!(matches!(
        DtxDecoder::decode_variant(&data, 3),
        Err(RoxError::InvalidFormat(_))
    ));
    // `decode_all` is `variants` and `decode_variant` of each, from one read.
    let all = DtxDecoder::decode_all(&data).unwrap();
    assert_eq!(
        all.iter().map(|(v, _)| v).cloned().collect::<Vec<_>>(),
        variants
    );
    for (variant, chart) in all {
        assert_eq!(
            chart.unwrap(),
            DtxDecoder::decode_variant(&data, variant.index).unwrap()
        );
    }
    assert!(DtxDecoder::decode_all(b"#TITLE: none").is_err());
    // `decode` is the first instrument.
    assert_eq!(
        DtxDecoder::decode(&data).unwrap().metadata.difficulty_name,
        "Drums"
    );
    // Without drums the guitar is variant 0.
    let data = file(&["#00024: 01", "#000A4: 01"]);
    let variants = DtxDecoder::variants(&data).unwrap();
    assert_eq!(
        variants
            .iter()
            .map(|v| (v.index, v.part))
            .collect::<Vec<_>>(),
        [(0, Part::Guitar), (1, Part::Bass)]
    );
}

#[test]
fn a_file_with_no_chip_of_an_instrument_is_an_error() {
    for data in [
        file(&[]),
        file(&["#TITLE: x", "#BPM: 120"]),
        file(&["#WAV01: a.ogg", "#00001: 01", "#00108: 01", "#00004: 01"]),
        file(&["#00031: 01", "#00028: 01"]),
        vec![0, 159, 146, 150, 0xFF, 0xFE, 0x00],
        b"#".to_vec(),
    ] {
        assert!(
            matches!(DtxDecoder::decode(&data), Err(RoxError::InvalidFormat(_))),
            "{data:?}"
        );
        assert!(DtxDecoder::variants(&data).is_err());
    }
}

#[test]
fn the_auto_decoders_route_a_dtx_by_its_extension() {
    use crate::formats::{auto_decode, auto_decode_all, auto_decode_bytes, auto_decode_variant};
    let dir = std::env::temp_dir().join(format!("rox-dtx-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Song.DTX");
    std::fs::write(&path, three_instruments()).unwrap();
    assert_eq!(
        auto_decode(&path).unwrap().metadata.difficulty_name,
        "Drums"
    );
    assert_eq!(
        auto_decode_variant(&path, 2)
            .unwrap()
            .metadata
            .difficulty_name,
        "Bass"
    );
    assert!(auto_decode_variant(&path, 3).is_err());
    assert_eq!(
        auto_decode_bytes("x/song.dtx", &three_instruments())
            .unwrap()
            .key_count,
        2
    );
    let all = auto_decode_all(&path).unwrap().unwrap();
    assert_eq!(
        all.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
        ["Drums", "Guitar", "Bass"]
    );
    assert!(all.iter().all(|v| v.chart.is_ok()));
    // A file with nothing to play is an error, not a panic.
    std::fs::write(dir.join("empty.dtx"), b"#TITLE: x").unwrap();
    assert!(auto_decode(dir.join("empty.dtx")).is_err());
    assert!(auto_decode_all(dir.join("empty.dtx")).is_err());
    // `set.def` is not a chart.
    std::fs::write(dir.join("set.def"), b"#TITLE: x\n#L1FILE: Song.DTX").unwrap();
    assert!(auto_decode(dir.join("set.def")).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}

// ------------------------------------------------------------------------------------------------ hostile input

#[test]
fn a_huge_line_is_read_without_trouble() {
    // 250 000 objects on one line: the 384 ticks of the measure keep 384 distinct positions.
    let line = format!("#00112: {}", "01".repeat(250_000));
    assert_eq!(line.len(), 500_008);
    let (chart, report, plan) = decode(&[line.as_str()]);
    assert_eq!(chart.notes.len(), 384);
    assert_eq!(report.overlapping_notes_dropped, 250_000 - 384);
    assert!(plan.is_empty());
    // The same with an odd length, and with a bad character at the very end.
    let odd = format!("#00112: {}0", "01".repeat(250_000));
    let (_, report, _) = decode(&[odd.as_str()]);
    assert_eq!(report.odd_data_lines, 1);
    let bad = format!("#00112: {}!!", "01".repeat(250_000));
    let (chart, report, _) = decode(&[bad.as_str(), "#00212: 01"]);
    assert_eq!((chart.notes.len(), report.invalid_data_lines), (1, 1));
    // A 500 000-character header and a 500 000-character comment.
    let title = format!("#TITLE: {}", "x".repeat(500_000));
    let comment = format!("; {}", "#".repeat(500_000));
    let (chart, _, _) = decode(&[title.as_str(), comment.as_str(), "#00012: 01"]);
    assert_eq!(chart.metadata.title.len(), 500_000);
}

#[test]
fn many_small_statements_are_read_in_time() {
    let mut lines: Vec<String> = (0..50_000)
        .map(|i| format!("#WAV{:02X}: s{i}.wav", i % 36))
        .collect();
    let digits = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    lines.extend(
        (0..3_600).map(|m| format!("#{}{:02}12: 01", char::from(digits[m / 100]), m % 100)),
    );
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let (chart, _, _) = decode(&refs);
    assert_eq!(chart.notes.len(), 3_600);
    // 255 nested `#IF`s and then some.
    let mut nested: Vec<&str> = vec!["#RANDOM 1"];
    nested.extend(std::iter::repeat_n("#IF 1", 300));
    nested.push("#00012: 01");
    nested.extend(std::iter::repeat_n("#ENDIF", 300));
    let (chart, _, _) = decode(&nested);
    assert_eq!(chart.notes.len(), 1);
}

#[test]
fn absurd_numbers_never_panic() {
    let tiny = format!("#BPM: 0.{}1", "0".repeat(300));
    let huge_length = format!("#00102: 1{}", "0".repeat(300));
    for lines in [
        vec![tiny.as_str(), "#00112: 01", "#00212: 0101"],
        vec![huge_length.as_str(), "#00212: 01", "#00312: 01"],
        vec!["#BASEBPM: 999999999999", "#00103: FF", "#00212: 01"],
        vec!["#00103: 01", "#00203: 01", "#00112: 01"],
        vec![
            "#WAV01: a.ogg",
            "#00201: 01",
            "#00024: 01",
            "#00024: 01",
            "#0012C: 01",
            "#0022C: 01",
            "#00012: 09",
        ],
    ] {
        let data = drums(&lines);
        for index in 0..3 {
            let _ = DtxDecoder::decode_with_audio_plan(&data, index);
        }
        let _ = DtxDecoder::variants(&data);
    }
    // Random bytes, truncated and NUL-filled files.
    for seed in 0..40u32 {
        let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
        let data: Vec<u8> = (0..400)
            .map(|_| {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let alphabet = b"#0123456789ABCDEF:; \n_WAVBPMTITLE";
                alphabet[((state >> 16) as usize) % alphabet.len()]
            })
            .collect();
        let _ = DtxDecoder::variants(&data);
        let _ = DtxDecoder::decode(&data);
    }
    let _ = DtxDecoder::decode(&[0u8; 1000]);
}

#[test]
fn the_part_names_and_report_are_stable() {
    assert_eq!(
        [Part::Drums, Part::Guitar, Part::Bass].map(Part::name),
        ["Drums", "Guitar", "Bass"]
    );
    let (_, report, _) = decode(&["#00012: 01"]);
    assert_eq!(report.part, Part::Drums);
    assert_eq!(report.charset, "UTF-8");
}
