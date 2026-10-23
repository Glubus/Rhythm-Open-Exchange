//! Timing of the `.sm` reader: `#OFFSET` (where beat 0 sits in the audio), stops, delays, warps,
//! the 192nd-note rows and the refusals of what it can not time.
use rox::codec::{Decoder, Encoder};
use rox::model::{Note, NoteType, RoxChart, TimingPoint};

use super::decoder::{SmDecoder, from_chart};
use super::encoder::SmEncoder;
use super::parser::parse;

/// A `.sm` file: `header` tags, then one `dance-single` chart with `measures`.
fn sm(header: &str, measures: &str) -> String {
    format!(
        "#TITLE:T;\n#MUSIC:song.ogg;\n{header}\n#NOTES:\n     dance-single:\n     :\n     Hard:\n     5:\n     0,0,0,0,0:\n{measures}\n;\n"
    )
}

/// One tap per beat on columns 0..4 (beats 0 to 3).
const FOUR: &str = "1000\n0100\n0010\n0001";

fn decode(text: &str) -> RoxChart {
    SmDecoder::decode(text.as_bytes()).expect("decodes and validates")
}

fn taps(chart: &RoxChart) -> Vec<(i64, u8)> {
    chart
        .notes
        .iter()
        .filter(|n| matches!(n.note_type, NoteType::Tap))
        .map(|n| (n.time_us, n.column))
        .collect()
}

#[test]
fn a_negative_offset_puts_beat_zero_that_far_into_the_audio() {
    // The StepMania meaning: `#OFFSET:-1.5` = the first beat sounds 1.5 s into the song. 120 BPM.
    let chart = decode(&sm("#OFFSET:-1.500;\n#BPMS:0.000=120.000;", FOUR));
    assert_eq!(
        taps(&chart),
        [
            (1_500_000, 0),
            (2_000_000, 1),
            (2_500_000, 2),
            (3_000_000, 3)
        ]
    );
    assert_eq!(chart.timing_points, [TimingPoint::bpm(1_500_000, 120.0)]);
    assert_eq!(
        chart.metadata.audio_offset_us, 0,
        "nothing is left for a consumer to apply twice"
    );
}

#[test]
fn a_positive_offset_puts_beat_zero_before_the_audio_starts() {
    let chart = decode(&sm("#OFFSET:0.100;\n#BPMS:0.000=120.000;", FOUR));
    assert_eq!(taps(&chart)[..2], [(-100_000, 0), (400_000, 1)]);
}

#[test]
fn the_offset_moves_holds_and_later_tempo_changes_as_one() {
    let chart = decode(&sm(
        "#OFFSET:-2.000;\n#BPMS:0.000=120.000,2.000=240.000;",
        "2000\n0100\n3000\n0001",
    ));
    assert!(chart.notes.contains(&Note::hold(2_000_000, 1_000_000, 0)));
    assert!(chart.notes.contains(&Note::tap(2_500_000, 1)));
    // Beat 3 is a quarter second after the tempo change at beat 2 (3 s in the audio).
    assert!(chart.notes.contains(&Note::tap(3_250_000, 3)));
    assert_eq!(
        chart.timing_points,
        [
            TimingPoint::bpm(2_000_000, 120.0),
            TimingPoint::bpm(3_000_000, 240.0)
        ]
    );
}

#[test]
fn a_stop_delays_what_follows_it_in_a_sm_file() {
    // Beat 1 at 0.5 s; the stop freezes the song for 1 s after the note of its beat.
    let chart = decode(&sm(
        "#OFFSET:-1.000;\n#BPMS:0.000=120.000;\n#STOPS:1.000=1.000;",
        FOUR,
    ));
    assert_eq!(
        taps(&chart),
        [
            (1_000_000, 0),
            (1_500_000, 1),
            (3_000_000, 2),
            (3_500_000, 3)
        ]
    );
}

#[test]
fn a_delay_holds_the_note_of_its_beat_in_a_sm_file() {
    let chart = decode(&sm(
        "#OFFSET:0.000;\n#BPMS:0.000=120.000;\n#DELAYS:1.000=0.500;",
        FOUR,
    ));
    assert_eq!(
        taps(&chart),
        [(0, 0), (1_000_000, 1), (1_500_000, 2), (2_000_000, 3)]
    );
}

#[test]
fn a_warp_drops_its_notes_and_skips_its_beats_in_a_sm_file() {
    let chart = decode(&sm(
        "#OFFSET:0.000;\n#BPMS:0.000=120.000;\n#WARPS:1.000=1.000;",
        FOUR,
    ));
    assert_eq!(taps(&chart), [(0, 0), (500_000, 2), (1_000_000, 3)]);
}

#[test]
fn notes_sit_on_rows_of_a_192nd_of_a_measure() {
    // Five lines per measure: beats 0, 0.8, 1.6, ... fall between rows; the game rounds each to
    // the nearest 1/48 beat (0.8 beat = 38.4 rows -> 38 rows = 0.791666.. beat = 395 833 us).
    let chart = decode(
        &sm(
            "#OFFSET:0.000;\n#BPMS:0.000=120.000;",
            "10000\n01000\n00100\n00010\n00001",
        )
        .replace("dance-single", "pump-single"),
    );
    assert_eq!(chart.key_count, 5);
    assert_eq!(taps(&chart)[1], (395_833, 1));
}

#[test]
fn a_sm_file_with_negative_timing_is_refused_with_a_reason_never_played_with_it_ignored() {
    for header in [
        "#BPMS:0.000=120.000,16.000=-120.000;",
        "#BPMS:0.000=120.000;\n#STOPS:4.000=-0.500;",
        "#BPMS:0.000=0.000;",
    ] {
        let file = parse(sm(header, FOUR).as_bytes()).unwrap();
        assert!(file.charts.is_empty(), "{header}");
        assert!(
            file.skipped[0].contains("unsupported") || file.skipped[0].contains("no valid"),
            "{:?}",
            file.skipped
        );
        let error = SmDecoder::decode(sm(header, FOUR).as_bytes())
            .unwrap_err()
            .to_string();
        assert!(error.contains("No playable chart"), "{error}");
    }
}

#[test]
fn charts_of_a_sm_file_that_can_not_be_played_are_skipped_the_others_keep_their_order() {
    let text = format!(
        "#TITLE:T;\n#OFFSET:0;\n#BPMS:0=120;\n#NOTES:\n lights-cabinet:\n :\n Easy:\n 1:\n 0,0,0,0,0:\n 1000\n;\n{}",
        sm("", FOUR)
            .split_once("#NOTES:")
            .map(|(_, notes)| format!("#NOTES:{notes}"))
            .unwrap()
    );
    let file = parse(text.as_bytes()).unwrap();
    assert_eq!(file.charts.len(), 1);
    assert_eq!(file.skipped.len(), 1);
    assert!(
        file.skipped[0].starts_with("chart 1: unsupported step type `lights-cabinet`"),
        "{:?}",
        file.skipped
    );
    assert_eq!(from_chart(&file, &file.charts[0]).notes.len(), 4);
}

#[test]
fn the_encoder_writes_the_offset_the_decoder_reads() {
    let mut chart = RoxChart::new(4);
    chart.timing_points.push(TimingPoint::bpm(1_500_000, 120.0));
    for (index, time) in [1_500_000, 2_000_000, 2_500_000].into_iter().enumerate() {
        chart
            .notes
            .push(Note::tap(time, u8::try_from(index).unwrap()));
    }
    let written = SmEncoder::encode(&chart).unwrap();
    assert!(
        core::str::from_utf8(&written)
            .unwrap()
            .contains("#OFFSET:-1.500000;")
    );
    let again = SmDecoder::decode(&written).unwrap();
    assert_eq!(again.notes, chart.notes);
    assert_eq!(again.timing_points, chart.timing_points);
}
