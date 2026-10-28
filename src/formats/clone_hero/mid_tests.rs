//! Tests of the `.mid` decoder and of the Standard MIDI File reader. Every file is built here, byte by byte, from the
//! layout the format notes give (`GuitarGame_ChartFormats` `mid-format`); NO real chart is used (community charts carry
//! no licence that allows redistribution). Times are computed by hand in the comments.
#![allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]

use rox::codec::Decoder;
use rox::model::{RoxChart, TimingPoint};

use super::smf::{self, Event, Events};
use super::tests::SngBuilder;
use super::{ChartContext, ChartReport, MidDecoder, ShiftSource, SngPackage};

// ------------------------------------------------------------------------------------------------- the builder

fn vlq(mut n: u32) -> Vec<u8> {
    let mut out = vec![(n & 0x7F) as u8];
    n >>= 7;
    while n > 0 {
        out.push((n & 0x7F) as u8 | 0x80);
        n >>= 7;
    }
    out.reverse();
    out
}

/// A track under construction: events with an absolute tick, written in order of tick (stable).
#[derive(Clone)]
struct Tk {
    name: Option<String>,
    events: Vec<(u64, Vec<u8>)>,
    running: bool,
}

impl Tk {
    fn new(name: &str) -> Self {
        Self {
            name: Some(name.to_owned()),
            events: Vec::new(),
            running: false,
        }
    }

    fn bare() -> Self {
        Self {
            name: None,
            events: Vec::new(),
            running: false,
        }
    }

    /// Writes consecutive channel events with the status byte left out (and keeps doing so after meta events and
    /// `SysEx`: the violation the format notes list).
    fn running(mut self) -> Self {
        self.running = true;
        self
    }

    fn raw(mut self, tick: u64, bytes: &[u8]) -> Self {
        self.events.push((tick, bytes.to_vec()));
        self
    }

    fn on(self, tick: u64, key: u8) -> Self {
        self.raw(tick, &[0x90, key, 100])
    }

    fn off(self, tick: u64, key: u8) -> Self {
        self.raw(tick, &[0x80, key, 64])
    }

    /// A note off written as a note on of velocity 0.
    fn off0(self, tick: u64, key: u8) -> Self {
        self.raw(tick, &[0x90, key, 0])
    }

    fn note(self, tick: u64, key: u8, len: u64) -> Self {
        self.on(tick, key).off(tick + len, key)
    }

    fn tempo(self, tick: u64, us: u32) -> Self {
        let b = us.to_be_bytes();
        self.raw(tick, &[0xFF, 0x51, 3, b[1], b[2], b[3]])
    }

    fn ts(self, tick: u64, numerator: u8, exponent: u8) -> Self {
        self.raw(tick, &[0xFF, 0x58, 4, numerator, exponent, 24, 8])
    }

    fn text(self, tick: u64, text: &str) -> Self {
        let mut bytes = vec![0xFF, 0x01, text.len() as u8];
        bytes.extend_from_slice(text.as_bytes());
        self.raw(tick, &bytes)
    }

    /// A Phase Shift `SysEx` modifier: `F0 <8> 'P' 'S' 0 0 <difficulty> <modifier> <value> F7`.
    fn ps(self, tick: u64, difficulty: u8, modifier: u8, value: u8) -> Self {
        self.raw(
            tick,
            &[0xF0, 8, b'P', b'S', 0, 0, difficulty, modifier, value, 0xF7],
        )
    }

    fn bytes(&self) -> Vec<u8> {
        let mut events = self.events.clone();
        events.sort_by_key(|e| e.0); // stable
        let mut body = Vec::new();
        if let Some(name) = &self.name {
            body.extend_from_slice(&[0, 0xFF, 0x03, name.len() as u8]);
            body.extend_from_slice(name.as_bytes());
        }
        let mut last_tick = 0;
        let mut last_status: Option<u8> = None;
        for (tick, bytes) in events {
            body.extend(vlq((tick - last_tick) as u32));
            last_tick = tick;
            let status = bytes[0];
            let channel = (0x80..=0xEF).contains(&status);
            if self.running && channel && last_status == Some(status) {
                body.extend_from_slice(&bytes[1..]);
            } else {
                body.extend_from_slice(&bytes);
            }
            if channel {
                last_status = Some(status);
            }
        }
        body.extend_from_slice(&[0, 0xFF, 0x2F, 0]);
        chunk(*b"MTrk", &body)
    }
}

fn chunk(id: [u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(body);
    out
}

fn header(format: u16, tracks: u16, division: u16) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&format.to_be_bytes());
    body.extend_from_slice(&tracks.to_be_bytes());
    body.extend_from_slice(&division.to_be_bytes());
    chunk(*b"MThd", &body)
}

fn midi(format: u16, tpq: u16, tracks: &[Tk]) -> Vec<u8> {
    let mut out = header(format, tracks.len() as u16, tpq);
    for track in tracks {
        out.extend(track.bytes());
    }
    out
}

/// A type 1 file at 480 ticks per quarter note and 120 BPM in 4/4, with `tracks` after the tempo track.
fn file(tracks: &[Tk]) -> Vec<u8> {
    let mut all = vec![Tk::new("notes").tempo(0, 500_000).ts(0, 4, 2)];
    all.extend_from_slice(tracks);
    midi(1, 480, &all)
}

fn guitar() -> Tk {
    Tk::new("PART GUITAR")
}

fn decode(data: &[u8]) -> RoxChart {
    MidDecoder::decode(data).expect("decodes")
}

fn report(data: &[u8], ini: Option<&str>, index: usize) -> (RoxChart, ChartReport) {
    MidDecoder::decode_with_report(data, ini.map(str::as_bytes), index).expect("decodes")
}

fn times(chart: &RoxChart) -> Vec<i64> {
    chart.notes.iter().map(|n| n.time_us).collect()
}

fn columns(chart: &RoxChart) -> Vec<u8> {
    chart.notes.iter().map(|n| n.column).collect()
}

fn bpms(chart: &RoxChart) -> Vec<(i64, f32, u8)> {
    chart
        .timing_points
        .iter()
        .filter_map(|tp| match tp {
            TimingPoint::Bpm {
                time_us,
                bpm,
                signature,
            } => Some((*time_us, *bpm, *signature)),
            TimingPoint::Sv { .. } => None,
        })
        .collect()
}

// ------------------------------------------------------------------------------------------------------- notes

#[test]
fn the_five_frets_of_expert_are_the_five_columns() {
    // 480 ticks = one quarter note = 500 000 µs at 120 BPM.
    let data = file(&[guitar()
        .note(0, 96, 0)
        .note(480, 97, 0)
        .note(960, 98, 0)
        .note(1440, 99, 0)
        .note(1920, 100, 0)]);
    let chart = decode(&data);
    assert_eq!(chart.key_count, 5);
    assert_eq!(columns(&chart), [0, 1, 2, 3, 4]);
    assert_eq!(times(&chart), [0, 500_000, 1_000_000, 1_500_000, 2_000_000]);
    assert!(chart.notes.iter().all(|n| !n.is_hold()));
    assert_eq!(chart.metadata.audio_file, "");
    assert_eq!(chart.metadata.difficulty_name, "Guitar Expert");
}

#[test]
fn every_difficulty_has_its_own_octave() {
    let data = file(&[guitar()
        .note(0, 96, 0)
        .note(0, 84, 0)
        .note(480, 85, 0)
        .note(0, 72, 0)
        .note(480, 74, 0)
        .note(960, 76, 0)
        .note(0, 60, 0)
        .note(480, 64, 0)]);
    let variants = MidDecoder::variants(&data).unwrap();
    let got: Vec<_> = variants
        .iter()
        .map(|v| (v.index, v.section.as_str(), v.notes, v.key_count))
        .collect();
    assert_eq!(
        got,
        [
            (0, "PART GUITAR Expert", 1, 5),
            (1, "PART GUITAR Hard", 2, 5),
            (2, "PART GUITAR Medium", 3, 5),
            (3, "PART GUITAR Easy", 2, 5),
        ]
    );
    let hard = MidDecoder::decode_variant(&data, 1).unwrap();
    assert_eq!(columns(&hard), [0, 1]);
    let medium = MidDecoder::decode_variant(&data, 2).unwrap();
    assert_eq!(columns(&medium), [0, 2, 4]);
    assert_eq!(times(&medium), [0, 500_000, 1_000_000]);
    let easy = MidDecoder::decode_variant(&data, 3).unwrap();
    assert_eq!(columns(&easy), [0, 4]);
    assert_eq!(hard.metadata.difficulty_name, "Guitar Hard");
    assert_eq!(easy.metadata.difficulty_name, "Guitar Easy");
    assert!(MidDecoder::decode_variant(&data, 4).is_err());
}

#[test]
fn variants_are_ordered_by_instrument_then_difficulty() {
    let data = file(&[
        Tk::new("PART KEYS").note(0, 96, 0),
        Tk::new("PART RHYTHM").note(0, 60, 0).note(0, 96, 0),
        Tk::new("PART BASS").note(0, 72, 0),
        Tk::new("PART GUITAR COOP").note(0, 84, 0),
        guitar().note(0, 96, 0).note(0, 60, 0),
    ]);
    let got: Vec<_> = MidDecoder::variants(&data)
        .unwrap()
        .iter()
        .map(|v| v.section.clone())
        .collect();
    assert_eq!(
        got,
        [
            "PART GUITAR Expert",
            "PART GUITAR Easy",
            "PART GUITAR COOP Hard",
            "PART BASS Medium",
            "PART RHYTHM Expert",
            "PART RHYTHM Easy",
            "PART KEYS Expert",
        ]
    );
    let labels: Vec<_> = (0..got.len())
        .map(|i| {
            MidDecoder::decode_variant(&data, i)
                .unwrap()
                .metadata
                .difficulty_name
                .to_string()
        })
        .collect();
    assert_eq!(
        labels,
        [
            "Guitar Expert",
            "Guitar Easy",
            "Guitar Co-op Hard",
            "Bass Medium",
            "Rhythm Expert",
            "Rhythm Easy",
            "Keys Expert"
        ]
    );
}

#[test]
fn chords_are_simultaneous_notes_and_note_on_velocity_zero_ends_a_note() {
    let data = file(&[guitar()
        .on(0, 96)
        .on(0, 98)
        .off0(480, 96)
        .raw(480, &[0x80, 98, 0])]);
    let chart = decode(&data);
    assert_eq!(columns(&chart), [0, 2]);
    assert_eq!(times(&chart), [0, 0]);
    // 480 ticks = 500 000 µs; 480 ≥ 160 (the cutoff), so both are holds.
    assert!(
        chart
            .notes
            .iter()
            .all(|n| n.is_hold() && n.duration_us() == 500_000)
    );
}

#[test]
fn markers_and_other_notes_are_not_notes() {
    // Solo 103, tap marker 104, versus 105/106, star power 116, BRE 120..124, tremolo 126, trill 127, animation 40..58.
    let mut track = guitar().note(0, 96, 0);
    for key in [
        103, 105, 106, 116, 120, 121, 122, 123, 124, 126, 127, 40, 50, 58,
    ] {
        track = track.note(480, key, 240);
    }
    let (chart, rep) = report(&file(&[track]), None, 0);
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(rep.forced_flags, 0);
    assert_eq!(rep.open_notes_dropped, 0);
    assert_eq!(rep.unpaired_midi_events, 0);
}

#[test]
fn forced_and_tap_markers_are_counted_not_kept() {
    // Expert: force HOPO 101, force strum 102, tap marker 104 (all difficulties). Hard: force HOPO 89, strum 90.
    let track = guitar()
        .note(0, 96, 0)
        .note(0, 101, 240)
        .note(480, 97, 0)
        .note(480, 102, 240)
        .note(480, 104, 240)
        .note(0, 84, 0)
        .note(0, 89, 240)
        .note(480, 90, 240);
    let data = file(&[track]);
    let (expert, rep) = report(&data, None, 0);
    assert_eq!(columns(&expert), [0, 1]);
    assert_eq!(rep.forced_flags, 2);
    assert_eq!(rep.tap_flags, 1);
    let (hard, rep) = report(&data, None, 1);
    assert_eq!(columns(&hard), [0]);
    assert_eq!(rep.forced_flags, 2);
    assert_eq!(rep.tap_flags, 1);
    // A track that has only markers is not a variant.
    let only = file(&[guitar().note(0, 101, 0).note(0, 104, 0)]);
    assert_eq!(MidDecoder::variants(&only).unwrap().len(), 0);
    assert!(MidDecoder::decode(&only).is_err());
}

#[test]
fn open_notes_by_note_need_enhanced_opens() {
    // Expert open = 95, Hard open = 83, Medium 71, Easy 59.
    let plain = file(&[guitar().note(0, 96, 0).note(480, 95, 0).note(480, 83, 0)]);
    let (chart, rep) = report(&plain, None, 0);
    assert_eq!(columns(&chart), [0]);
    assert_eq!(
        rep.open_notes_dropped, 0,
        "without the event 95 is animation data"
    );
    for text in [
        "[ENHANCED_OPENS]",
        "ENHANCED_OPENS",
        "  [ ENHANCED_OPENS ] ",
    ] {
        let data = file(&[guitar()
            .text(0, text)
            .note(0, 96, 0)
            .note(480, 95, 0)
            .note(960, 83, 0)
            .note(960, 84, 0)]);
        let (expert, rep) = report(&data, None, 0);
        assert_eq!(columns(&expert), [0], "{text}");
        assert_eq!(rep.open_notes_dropped, 1, "{text}");
        let (hard, rep) = report(&data, None, 1);
        assert_eq!(columns(&hard), [0], "{text}");
        assert_eq!(rep.open_notes_dropped, 1, "{text}");
    }
    // Another text does not enable them.
    let data = file(&[guitar()
        .text(0, "[ENHANCED_OPEN]")
        .note(0, 96, 0)
        .note(480, 95, 0)]);
    assert_eq!(report(&data, None, 0).1.open_notes_dropped, 0);
    // A difficulty with only open notes is not a variant.
    let data = file(&[guitar().text(0, "[ENHANCED_OPENS]").note(0, 95, 0)]);
    assert_eq!(MidDecoder::variants(&data).unwrap().len(), 0);
}

#[test]
fn open_phrases_turn_the_notes_inside_into_dropped_opens() {
    // Phrase on Expert (difficulty byte 3) from tick 480 to 960: the notes at 480 (a chord) and 720 are inside, the
    // note at 960 is not (the last tick of a phrase is not part of it).
    let track = guitar()
        .note(0, 96, 0)
        .ps(480, 3, 0x01, 1)
        .ps(960, 3, 0x01, 0)
        .note(480, 96, 0)
        .note(480, 97, 0)
        .note(720, 98, 0)
        .note(960, 99, 0)
        .note(0, 84, 0)
        .note(480, 84, 0);
    let data = file(&[track]);
    let (expert, rep) = report(&data, None, 0);
    assert_eq!(columns(&expert), [0, 3]);
    assert_eq!(times(&expert), [0, 1_000_000]);
    assert_eq!(rep.open_notes_dropped, 3);
    // The Hard notes are untouched.
    let (hard, rep) = report(&data, None, 1);
    assert_eq!(columns(&hard), [0, 0]);
    assert_eq!(rep.open_notes_dropped, 0);
}

#[test]
fn an_open_phrase_for_all_difficulties_and_a_phrase_of_no_length() {
    let track = guitar()
        .note(0, 96, 0)
        .note(0, 84, 0)
        .note(480, 96, 0)
        .note(480, 84, 0)
        // All difficulties (0xFF) at tick 480 and a start and end on the same tick: the note of that tick is inside.
        .ps(480, 0xFF, 0x01, 1)
        .ps(480, 0xFF, 0x01, 0);
    let data = file(&[track]);
    let (expert, rep) = report(&data, None, 0);
    assert_eq!(columns(&expert), [0]);
    assert_eq!(rep.open_notes_dropped, 1);
    let (hard, _) = report(&data, None, 1);
    assert_eq!(columns(&hard), [0]);
}

#[test]
fn overlapping_open_phrases_count_each_note_once() {
    let track = guitar()
        .note(0, 96, 0)
        .note(480, 96, 0)
        .note(720, 97, 0)
        .ps(240, 3, 0x01, 1)
        .ps(960, 3, 0x01, 0)
        .ps(480, 0xFF, 0x01, 1)
        .ps(1200, 0xFF, 0x01, 0);
    let (chart, rep) = report(&file(&[track]), None, 0);
    assert_eq!(times(&chart), [0]);
    assert_eq!(rep.open_notes_dropped, 2);
}

#[test]
fn tap_phrases_are_counted_and_keep_their_notes() {
    let track = guitar()
        .note(0, 96, 0)
        .note(480, 97, 0)
        .ps(0, 0xFF, 0x04, 1)
        .ps(960, 0xFF, 0x04, 0)
        .ps(0, 3, 0x04, 1)
        .ps(480, 3, 0x04, 0);
    let (chart, rep) = report(&file(&[track]), None, 0);
    assert_eq!(columns(&chart), [0, 1]);
    assert_eq!(rep.tap_flags, 2);
    assert_eq!(rep.open_notes_dropped, 0);
}

#[test]
fn unknown_and_unpaired_sysex_are_ignored() {
    let track = guitar()
        .note(0, 96, 0)
        // Wrong header, wrong length, another type, another modifier, a bad difficulty: skipped.
        .raw(0, &[0xF0, 8, b'X', b'S', 0, 0, 3, 1, 1, 0xF7])
        .raw(0, &[0xF0, 3, 1, 2, 0xF7])
        .raw(0, &[0xF0, 8, b'P', b'S', 0, 1, 3, 1, 1, 0xF7])
        .ps(0, 3, 0x07, 1)
        .ps(0, 9, 0x01, 1)
        // A start with no end, an end with no start.
        .ps(0, 3, 0x01, 1)
        .ps(100, 2, 0x01, 0);
    let (chart, rep) = report(&file(&[track]), None, 0);
    assert_eq!(columns(&chart), [0]);
    assert_eq!(rep.open_notes_dropped, 0);
    assert_eq!(rep.unpaired_midi_events, 2);
}

// ---------------------------------------------------------------------------------------------------- sustains

#[test]
fn the_default_cutoff_is_a_third_of_the_resolution() {
    // 480 / 3 = 160 ticks: 159 is a tap (shorter), 160 and 161 are holds.
    let data = file(&[guitar()
        .note(0, 96, 159)
        .note(480, 97, 160)
        .note(960, 98, 161)
        .note(1440, 99, 0)]);
    let (chart, rep) = report(&data, None, 0);
    let holds: Vec<_> = chart.notes.iter().map(rox::model::Note::is_hold).collect();
    assert_eq!(holds, [false, true, true, false]);
    assert_eq!(rep.sustains_cut_to_taps, 1);
    // 160 ticks = 160 x 500 000 / 480 µs.
    assert_eq!(chart.notes[1].duration_us(), 166_667);
    assert_eq!(chart.notes[2].duration_us(), 167_708);
}

#[test]
fn the_ini_cutoff_replaces_the_default() {
    let data = file(&[guitar()
        .note(0, 96, 1)
        .note(480, 97, 479)
        .note(960, 98, 480)]);
    // 0 is read as 1 tick: every sustain of 1 tick or more stays.
    let (chart, _) = report(&data, Some("[song]\nsustain_cutoff_threshold = 0\n"), 0);
    assert_eq!(
        chart
            .notes
            .iter()
            .map(rox::model::Note::is_hold)
            .collect::<Vec<_>>(),
        [true, true, true]
    );
    let (chart, rep) = report(&data, Some("[song]\nsustain_cutoff_threshold = 480\n"), 0);
    assert_eq!(
        chart
            .notes
            .iter()
            .map(rox::model::Note::is_hold)
            .collect::<Vec<_>>(),
        [false, false, true]
    );
    assert_eq!(rep.sustains_cut_to_taps, 2);
    // A negative value is no value: the default (160) applies.
    let (chart, _) = report(&data, Some("[song]\nsustain_cutoff_threshold = -1\n"), 0);
    assert_eq!(
        chart
            .notes
            .iter()
            .map(rox::model::Note::is_hold)
            .collect::<Vec<_>>(),
        [false, true, true]
    );
    // A cutoff that is not a number is no value either.
    let (chart, _) = report(&data, Some("[song]\nsustain_cutoff_threshold = soon\n"), 0);
    assert_eq!(chart.notes.iter().filter(|n| n.is_hold()).count(), 2);
}

#[test]
fn a_sustain_stops_at_the_next_note_of_its_fret() {
    // A hold of 480 ticks on green, the next green 240 ticks later: it ends there (240 ticks = 250 000 µs). Notes of one
    // key and channel never overlap (the next note on would be ignored), so the second green is on channel 1. A red
    // under it is not touched; a hold that ends exactly where the next note starts is not shortened.
    let data = file(&[guitar()
        .raw(0, &[0x90, 96, 100])
        .raw(240, &[0x91, 96, 100])
        .raw(240, &[0x81, 96, 0])
        .raw(480, &[0x80, 96, 0])
        .note(0, 97, 960)
        .note(1440, 98, 480)
        .note(1920, 98, 0)]);
    let (chart, rep) = report(&data, None, 0);
    assert_eq!(columns(&chart), [0, 1, 0, 2, 2]);
    let green = &chart.notes[0];
    assert!(green.is_hold());
    assert_eq!(green.duration_us(), 250_000);
    let red = &chart.notes[1];
    assert_eq!(red.duration_us(), 1_000_000);
    assert_eq!(chart.notes[3].duration_us(), 500_000);
    assert_eq!(rep.sustains_clamped, 1);
}

#[test]
fn a_disjoint_chord_keeps_the_length_of_each_note() {
    let data = file(&[guitar().note(0, 96, 480).note(0, 97, 960)]);
    let chart = decode(&data);
    assert_eq!(chart.notes[0].duration_us(), 500_000);
    assert_eq!(chart.notes[1].duration_us(), 1_000_000);
}

#[test]
fn a_second_note_of_the_same_tick_and_fret_is_dropped() {
    // Back to back on the same tick: the note off closes the first, the second note on opens another (same tick).
    let data = file(&[guitar().on(0, 96).off(0, 96).on(0, 96).off(240, 96)]);
    let (chart, rep) = report(&data, None, 0);
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(rep.duplicate_notes_dropped, 1);
    assert!(!chart.notes[0].is_hold(), "the first one (length 0) stays");
}

#[test]
fn pairing_is_by_channel_and_key() {
    // Key 96 sounds on channel 0 and on channel 1 at once; each off closes its own.
    let data = file(&[guitar()
        .raw(0, &[0x90, 96, 100])
        .raw(0, &[0x91, 96, 100])
        .raw(480, &[0x81, 96, 0])
        .raw(960, &[0x80, 96, 0])]);
    let (chart, rep) = report(&data, None, 0);
    // The same tick and fret twice: one stays, the first by order of closing (channel 1, 480 ticks).
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(rep.duplicate_notes_dropped, 1);
    assert_eq!(rep.unpaired_midi_events, 0);
}

#[test]
fn unpaired_note_events_are_counted_and_ignored() {
    let track = guitar()
        .note(0, 96, 0)
        .off(100, 97) // an off with no on
        .on(200, 98)
        .on(300, 98) // an on of a key already sounding
        .on(480, 99); // never closed
    let (chart, rep) = report(&file(&[track]), None, 0);
    // The note 98 closes never (the track ends), 99 as well: only the first note remains.
    assert_eq!(columns(&chart), [0]);
    assert_eq!(rep.unpaired_midi_events, 4);
}

// ------------------------------------------------------------------------------------------------------- time

#[test]
fn tempo_changes_and_signatures() {
    // 480 ticks per quarter note. 120 BPM (500 000 µs) for 480 ticks = 500 000 µs, then 240 BPM (250 000 µs per
    // quarter): tick 960 is 500 000 + 480 x 250 000 / 480 = 750 000 µs; tick 1440 is 1 000 000 µs.
    let tempo = Tk::new("notes")
        .tempo(0, 500_000)
        .ts(0, 3, 2)
        .tempo(480, 250_000)
        .ts(480, 7, 3);
    let notes = guitar().note(0, 96, 0).note(960, 97, 0).note(1440, 98, 0);
    let data = midi(1, 480, &[tempo, notes]);
    let chart = decode(&data);
    assert_eq!(times(&chart), [0, 750_000, 1_000_000]);
    // 7/8 = 3.5 quarter notes, rounded to 4.
    assert_eq!(bpms(&chart), [(0, 120.0, 3), (500_000, 240.0, 4)]);
}

#[test]
fn an_odd_tempo_and_a_resolution_of_96() {
    // 428 571 µs per quarter note, 96 ticks per quarter note: tick 960 = 10 quarter notes = 4 285 710 µs.
    let tempo = Tk::new("t").tempo(0, 428_571);
    let data = midi(1, 96, &[tempo, guitar().note(960, 96, 0)]);
    let (chart, rep) = report(&data, None, 0);
    assert_eq!(times(&chart), [4_285_710]);
    assert_eq!(rep.resolution, 96);
    let TimingPoint::Bpm { bpm, signature, .. } = chart.timing_points[0] else {
        panic!("a BPM point")
    };
    assert!((bpm - 60.0e6 / 428_571.0).abs() < 1e-3);
    assert_eq!(signature, 4, "no time signature: 4");
}

#[test]
fn the_tempo_is_120_until_the_first_tempo_event() {
    let none = midi(1, 480, &[Tk::new("t"), guitar().note(960, 96, 0)]);
    let (chart, rep) = report(&none, None, 0);
    assert_eq!(times(&chart), [1_000_000]);
    assert!(rep.default_tempo_used);
    assert_eq!(bpms(&chart), [(0, 120.0, 4)]);
    // A first tempo at tick 480: 120 BPM before it.
    let late = midi(
        1,
        480,
        &[Tk::new("t").tempo(480, 250_000), guitar().note(960, 96, 0)],
    );
    let (chart, rep) = report(&late, None, 0);
    assert!(rep.default_tempo_used);
    assert_eq!(times(&chart), [750_000]);
    assert_eq!(bpms(&chart), [(0, 120.0, 4), (500_000, 240.0, 4)]);
    let (_, rep) = report(&file(&[guitar().note(0, 96, 0)]), None, 0);
    assert!(!rep.default_tempo_used);
}

#[test]
fn bad_sync_events_are_counted_and_the_last_event_of_a_tick_wins() {
    let tempo = Tk::new("t")
        .tempo(0, 0) // a tempo of 0: ignored
        .tempo(0, 600_000)
        .tempo(0, 300_000) // the last of the tick wins
        .raw(0, &[0xFF, 0x51, 2, 1, 2]) // a tempo of two bytes
        .ts(0, 0, 2) // numerator 0
        .ts(0, 4, 32) // exponent beyond 31
        .raw(0, &[0xFF, 0x58, 1, 4]) // a signature of one byte
        .ts(0, 3, 2)
        .ts(0, 5, 2);
    let data = midi(1, 480, &[tempo, guitar().note(480, 96, 0)]);
    let (chart, rep) = report(&data, None, 0);
    // 300 000 µs per quarter note: tick 480 = 300 000 µs.
    assert_eq!(times(&chart), [300_000]);
    assert_eq!(bpms(&chart), [(0, 200.0, 5)]);
    assert_eq!(rep.ignored_sync_events, 5);
    assert_eq!(rep.replaced_sync_events, 2);
}

#[test]
fn only_the_first_track_gives_the_tempo_map() {
    let data = file(&[guitar().tempo(0, 100_000).note(480, 96, 0).ts(0, 3, 2)]);
    let (chart, rep) = report(&data, None, 0);
    assert_eq!(times(&chart), [500_000]);
    assert_eq!(bpms(&chart), [(0, 120.0, 4)]);
    assert_eq!(rep.sync_events_off_first_track, 2);
}

#[test]
fn a_note_beyond_ten_million_seconds_is_dropped() {
    // 1 tick per quarter note and 16 777 215 µs per quarter note: tick 2^28 - 1 is far beyond 10^7 s.
    let tempo = Tk::new("t").tempo(0, 0x00FF_FFFF);
    let data = midi(
        1,
        1,
        &[tempo, guitar().note(0, 96, 0).note(0x0FFF_FFFF, 97, 0)],
    );
    let (chart, rep) = report(&data, None, 0);
    assert_eq!(columns(&chart), [0]);
    assert_eq!(rep.events_out_of_range, 1);
}

#[test]
fn the_ini_delay_moves_every_time_and_has_no_other_source() {
    let data = file(&[guitar().note(480, 96, 0)]);
    let (chart, rep) = report(&data, Some("[song]\ndelay = 250\n"), 0);
    assert_eq!(times(&chart), [750_000]);
    assert_eq!(bpms(&chart), [(250_000, 120.0, 4)]);
    assert_eq!(rep.shift_us, 250_000);
    assert_eq!(rep.shift_source, ShiftSource::IniDelay);
    let (chart, _) = report(&data, Some("[song]\ndelay = -100\n"), 0);
    assert_eq!(times(&chart), [400_000]);
    let (chart, rep) = report(&data, None, 0);
    assert_eq!(times(&chart), [500_000]);
    assert_eq!(rep.shift_source, ShiftSource::None);
    assert_eq!(chart.metadata.audio_offset_us, 0);
    let (_, rep) = report(&data, Some("[song]\ndelay = 99999999999999\n"), 0);
    assert!(rep.shift_out_of_range);
}

#[test]
fn metadata_comes_from_the_song_ini() {
    let data = file(&[guitar().note(0, 96, 0), Tk::new("PART BASS").note(0, 96, 0)]);
    let ini = "[song]\nname = Example\nartist = Somebody\ncharter = Me\ngenre = Rock\nbackground = bg.png\n\
               diff_guitar = 4\ndiff_bass = -1\npreview_start_time = 5000\npreview_end_time = 35000\n";
    let guitar_chart = MidDecoder::decode_with_ini(&data, Some(ini.as_bytes()), 0).unwrap();
    assert_eq!(guitar_chart.metadata.title, "Example");
    assert_eq!(guitar_chart.metadata.artist, "Somebody");
    assert_eq!(guitar_chart.metadata.creator, "Me");
    assert_eq!(guitar_chart.metadata.genre.as_deref(), Some("Rock"));
    assert_eq!(
        guitar_chart.metadata.background_file.as_deref(),
        Some("bg.png")
    );
    assert_eq!(guitar_chart.metadata.difficulty_value, Some(4.0));
    assert_eq!(guitar_chart.metadata.preview_time_us, 5_000_000);
    assert_eq!(guitar_chart.metadata.preview_duration_us, 30_000_000);
    let bass = MidDecoder::decode_with_ini(&data, Some(ini.as_bytes()), 1).unwrap();
    assert_eq!(bass.metadata.difficulty_value, None);
    // No song.ini: nothing in the file names the song.
    let bare = decode(&data);
    assert_eq!(bare.metadata.title, "");
    assert_eq!(bare.metadata.artist, "");
    assert_eq!(bare.metadata.audio_file, "");
}

// ---------------------------------------------------------------------------------------------------- tracks

#[test]
fn unsupported_tracks_are_reported() {
    let data = file(&[
        Tk::new("EVENTS").text(0, "[section Intro]"),
        Tk::new("BEAT").note(0, 12, 10),
        Tk::new("T1 GEMS").note(0, 96, 0),
        Tk::new("PART DRUMS").note(0, 96, 0),
        Tk::new("PART VOCALS").note(0, 60, 0),
        Tk::new("PART GUITAR GHL").note(0, 94, 0),
        Tk::new("PART REAL_GUITAR").note(0, 96, 0),
        Tk::new("HARM1").note(0, 60, 0),
        Tk::new("VENUE"),
        guitar().note(0, 96, 0),
    ]);
    let (chart, rep) = report(&data, None, 0);
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(
        rep.unsupported_tracks,
        [
            "T1 GEMS",
            "PART DRUMS",
            "PART VOCALS",
            "PART GUITAR GHL",
            "PART REAL_GUITAR",
            "HARM1"
        ]
    );
    assert_eq!(MidDecoder::variants(&data).unwrap().len(), 1);
}

#[test]
fn track_names_are_exact_and_found_at_tick_zero() {
    // A track whose first events are not the name, a track with no name, a name in the wrong case, a name after tick 0.
    let data = file(&[
        Tk::bare()
            .text(0, "x")
            .raw(
                0,
                &[
                    0xFF, 0x03, 9, b'P', b'A', b'R', b'T', b' ', b'B', b'A', b'S', b'S',
                ],
            )
            .note(0, 96, 0),
        Tk::bare().note(0, 96, 0),
        Tk::new("part rhythm").note(0, 96, 0),
        Tk::bare()
            .raw(
                10,
                &[
                    0xFF, 0x03, 9, b'P', b'A', b'R', b'T', b' ', b'K', b'E', b'Y', b'S',
                ],
            )
            .note(0, 96, 0),
    ]);
    let variants = MidDecoder::variants(&data).unwrap();
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0].section, "PART BASS Expert");
}

#[test]
fn the_first_track_of_a_type_1_file_is_the_tempo_track_only() {
    let data = midi(1, 480, &[guitar().tempo(0, 500_000).note(0, 96, 0)]);
    assert_eq!(MidDecoder::variants(&data).unwrap().len(), 0);
    // A file with no track at all is an error.
    assert!(MidDecoder::variants(&header(1, 0, 480)).is_err());
}

#[test]
fn a_type_0_file_has_its_tempo_and_its_notes_in_one_track() {
    let single = guitar().tempo(0, 250_000).note(480, 96, 0);
    let data = midi(0, 480, &[single]);
    let chart = decode(&data);
    // 250 000 µs per quarter note: tick 480 = 250 000 µs.
    assert_eq!(times(&chart), [250_000]);
}

#[test]
fn a_repeated_track_keeps_the_first_except_part_guitar_which_keeps_the_last() {
    let data = file(&[
        Tk::new("PART BASS").note(0, 96, 0),
        Tk::new("PART BASS").note(0, 97, 0).note(480, 97, 0),
        guitar().note(0, 96, 0),
        guitar().note(0, 98, 0).note(480, 98, 0),
    ]);
    let (guitar_chart, rep) = report(&data, None, 0);
    assert_eq!(columns(&guitar_chart), [2, 2]);
    assert_eq!(rep.duplicate_tracks_ignored, 2);
    let (bass, _) = report(&data, None, 1);
    assert_eq!(columns(&bass), [0]);
    assert_eq!(MidDecoder::variants(&data).unwrap().len(), 2);
}

// ---------------------------------------------------------------------------------------- running status etc

#[test]
fn running_status_is_read_and_is_not_reset_by_meta_events() {
    // Chord notes written one after the other have the same status byte: it is left out. A text event in the middle
    // (a meta event) does not reset it: the format notes list that as a violation that charts commit.
    let build = |running: bool| {
        let mut track = guitar()
            .on(0, 96)
            .on(0, 98)
            .text(240, "[note]")
            .on(240, 99)
            .off(480, 96)
            .off(480, 98)
            .off(480, 99);
        if running {
            track = track.running();
        }
        file(&[track])
    };
    let long = build(false);
    let short = build(true);
    assert!(short.len() < long.len());
    let a = decode(&long);
    let b = decode(&short);
    // 240 ticks = 250 000 µs.
    assert_eq!(times(&a), [0, 0, 250_000]);
    assert_eq!(times(&a), times(&b));
    assert_eq!(columns(&a), [0, 2, 3]);
    assert_eq!(columns(&a), columns(&b));
    assert_eq!(
        b.notes
            .iter()
            .map(rox::model::Note::duration_us)
            .collect::<Vec<_>>(),
        [500_000, 500_000, 250_000]
    );
}

#[test]
fn sysex_may_hold_bytes_above_7f_and_unknown_chunks_are_skipped() {
    let track = guitar()
        .raw(0, &[0xF0, 4, 0xFF, 0xFE, 0x80, 0xF7])
        .raw(0, &[0xF7, 2, 0xFF, 0xFF])
        .raw(0, &[0xB0, 7, 100]) // a control change
        .raw(0, &[0xC0, 5]) // a program change
        .raw(0, &[0xE0, 0, 64]) // a pitch bend
        .raw(0, &[0xFF, 0x7F, 3, 1, 2, 3]) // a vendor meta event
        .note(0, 96, 0);
    let mut data = header(1, 3, 480);
    data.extend(Tk::new("notes").tempo(0, 500_000).bytes());
    data.extend(chunk(*b"XFIH", &[1, 2, 3]));
    data.extend(track.bytes());
    data.extend_from_slice(&[0, 0, 0]); // trailing bytes that are not a chunk
    assert_eq!(decode(&data).notes.len(), 1);
}

#[test]
fn an_event_after_the_end_of_track_is_not_read() {
    let mut body = Vec::new();
    body.extend_from_slice(&[0, 0xFF, 0x03, 11]);
    body.extend_from_slice(b"PART GUITAR");
    body.extend_from_slice(&[0, 0x90, 96, 100, 0, 0x80, 96, 0]);
    body.extend_from_slice(&[0, 0xFF, 0x2F, 0]);
    body.extend_from_slice(&[0, 0x90, 97, 100, 0, 0x80, 97, 0]); // after the end
    let mut data = header(1, 2, 480);
    data.extend(Tk::new("t").bytes());
    data.extend(chunk(*b"MTrk", &body));
    assert_eq!(columns(&decode(&data)), [0]);
}

// -------------------------------------------------------------------------------------------- the file reader

#[test]
fn the_reader_gives_events_with_absolute_ticks() {
    let track = Tk::new("n")
        .tempo(10, 500_000)
        .ts(10, 3, 2)
        .on(20, 60)
        .off(30, 60)
        .off0(40, 60);
    let bytes = track.bytes();
    let whole = midi(1, 120, &[track]);
    let parsed = smf::parse(&whole).unwrap();
    assert_eq!(
        (parsed.format, parsed.ticks_per_quarter, parsed.tracks.len()),
        (1, 120, 1)
    );
    assert_eq!(parsed.tracks[0], &bytes[8..]);
    let events: Vec<_> = Events::new(parsed.tracks[0]).map(|e| e.unwrap()).collect();
    assert_eq!(
        events,
        [
            (0, Event::TrackName(b"n")),
            (10, Event::Tempo(500_000)),
            (
                10,
                Event::TimeSignature {
                    numerator: 3,
                    exponent: 2
                }
            ),
            (
                20,
                Event::NoteOn {
                    channel: 0,
                    key: 60
                }
            ),
            (
                30,
                Event::NoteOff {
                    channel: 0,
                    key: 60
                }
            ),
            (
                40,
                Event::NoteOff {
                    channel: 0,
                    key: 60
                }
            ),
            (40, Event::EndOfTrack),
        ]
    );
}

#[test]
fn variable_length_numbers() {
    // The examples of the Standard MIDI File specification.
    for (n, bytes) in [
        (0u32, vec![0x00]),
        (0x40, vec![0x40]),
        (0x7F, vec![0x7F]),
        (0x80, vec![0x81, 0x00]),
        (0x2000, vec![0xC0, 0x00]),
        (0x3FFF, vec![0xFF, 0x7F]),
        (0x4000, vec![0x81, 0x80, 0x00]),
        (0x10_0000, vec![0xC0, 0x80, 0x00]),
        (0x1F_FFFF, vec![0xFF, 0xFF, 0x7F]),
        (0x20_0000, vec![0x81, 0x80, 0x80, 0x00]),
        (0x0800_0000, vec![0xC0, 0x80, 0x80, 0x00]),
        (0x0FFF_FFFF, vec![0xFF, 0xFF, 0xFF, 0x7F]),
    ] {
        assert_eq!(vlq(n), bytes, "{n:#x}");
        // As a delta time in front of a note on.
        let mut track = bytes.clone();
        track.extend_from_slice(&[0x90, 60, 100]);
        let (tick, event) = Events::new(&track).next().unwrap().unwrap();
        assert_eq!(tick, u64::from(n));
        assert_eq!(
            event,
            Event::NoteOn {
                channel: 0,
                key: 60
            }
        );
    }
    // Five bytes with the continuation bit set: refused.
    assert!(
        Events::new(&[0xFF, 0xFF, 0xFF, 0xFF, 0x7F, 0x90, 60, 100])
            .next()
            .unwrap()
            .is_err()
    );
}

#[test]
fn the_header_is_checked() {
    let track = Tk::new("t").bytes();
    let build = |h: Vec<u8>| {
        let mut out = h;
        out.extend_from_slice(&track);
        out
    };
    assert!(smf::parse(&build(header(1, 1, 480))).is_ok());
    assert!(smf::parse(&build(header(0, 1, 480))).is_ok());
    // Format 2, SMPTE division, division 0, no header.
    for bad in [
        header(2, 1, 480),
        header(1, 1, 0xE728),
        header(1, 1, 0),
        header(1, 1, 0x8000),
    ] {
        assert!(smf::parse(&build(bad)).is_err());
    }
    assert!(smf::parse(b"").is_err());
    assert!(smf::parse(b"RIFF").is_err());
    assert!(smf::parse(&track).is_err());
    // A header longer than 6 bytes: the extra bytes are skipped; shorter: refused.
    let mut longer = b"MThd".to_vec();
    longer.extend_from_slice(&8u32.to_be_bytes());
    longer.extend_from_slice(&[0, 1, 0, 1, 1, 0xE0, 9, 9]);
    longer.extend_from_slice(&track);
    assert_eq!(smf::parse(&longer).unwrap().ticks_per_quarter, 480);
    let mut shorter = b"MThd".to_vec();
    shorter.extend_from_slice(&4u32.to_be_bytes());
    shorter.extend_from_slice(&[0, 1, 0, 1]);
    assert!(smf::parse(&shorter).is_err());
    // The track count of the header is only metadata.
    let mut lying = header(1, 9, 480);
    lying.extend_from_slice(&track);
    assert_eq!(smf::parse(&lying).unwrap().tracks.len(), 1);
}

// ----------------------------------------------------------------------------------------------- hostile input

#[test]
fn hostile_input_never_panics() {
    let good = file(&[guitar()
        .text(0, "[ENHANCED_OPENS]")
        .note(0, 96, 480)
        .ps(0, 3, 1, 1)
        .ps(480, 3, 1, 0)
        .note(480, 95, 0)]);
    // Every truncation, and every byte replaced by the extreme values.
    for len in 0..good.len() {
        let _ = MidDecoder::variants(&good[..len]);
    }
    for at in 0..good.len() {
        for value in [0x00, 0x7F, 0x80, 0xFF] {
            let mut bad = good.clone();
            bad[at] = value;
            let _ = MidDecoder::variants(&bad);
            let _ = MidDecoder::decode(&bad);
        }
    }
    // Pseudo-random bytes behind a valid header.
    let mut state = 0x1234_5678_u32;
    for _ in 0..200 {
        let mut data = header(1, 2, 480);
        let mut body = Vec::new();
        for _ in 0..256 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            body.push((state >> 24) as u8);
        }
        data.extend(chunk(*b"MTrk", &body));
        data.extend(chunk(*b"MTrk", &body));
        let _ = MidDecoder::variants(&data);
    }
}

#[test]
fn bad_lengths_and_counts_are_errors() {
    // A chunk longer than the file.
    let mut data = header(1, 1, 480);
    data.extend_from_slice(b"MTrk");
    data.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
    data.extend_from_slice(&[0, 0xFF, 0x2F, 0]);
    assert!(MidDecoder::variants(&data).is_err());
    // A SysEx and a meta event longer than their track; a name of 0x0FFFFFFF bytes.
    for event in [
        vec![0, 0xF0, 0xFF, 0xFF, 0xFF, 0x7F, 1],
        vec![0, 0xFF, 0x01, 0xFF, 0xFF, 0xFF, 0x7F, 1],
        vec![0, 0xFF, 0x03, 0xFF, 0xFF, 0xFF, 0x7F, 1],
        vec![0, 0xFF, 0x51, 5, 1],
    ] {
        let mut data = header(1, 2, 480);
        data.extend(Tk::new("t").bytes());
        data.extend(chunk(*b"MTrk", &event));
        let _ = MidDecoder::variants(&data); // no name: skipped, never a panic or a huge allocation
        let mut data = header(1, 1, 480);
        data.extend(chunk(*b"MTrk", &event));
        assert!(
            MidDecoder::variants(&data).is_err(),
            "the tempo track is read: {event:?}"
        );
    }
    // A channel event cut short, a status byte inside the data, a data byte with no running status, a bad status.
    let broken = |events: &[u8]| {
        let mut body = vec![0, 0xFF, 0x03, 11];
        body.extend_from_slice(b"PART GUITAR");
        body.extend_from_slice(events);
        let mut data = header(1, 2, 480);
        data.extend(Tk::new("t").bytes());
        data.extend(chunk(*b"MTrk", &body));
        MidDecoder::variants(&data)
    };
    assert!(broken(&[0, 0x90, 96]).is_err());
    assert!(broken(&[0, 0x90, 96, 0x90]).is_err());
    assert!(broken(&[0, 0x90, 0x90, 100]).is_err());
    assert!(broken(&[0, 0xF1, 0]).is_err());
    assert!(broken(&[0, 0xF8]).is_err());
    assert!(broken(&[0, 0x40, 100]).is_err(), "no running status yet");
    assert!(broken(&[0x80]).is_err(), "a delta time with no event");
    assert_eq!(
        broken(&[0, 0x90, 96, 100, 0, 0x80, 96, 0]).unwrap().len(),
        1
    );
}

#[test]
fn a_file_with_too_many_chunks_or_too_large_is_refused() {
    let mut data = header(1, 1, 480);
    for _ in 0..=smf::MAX_CHUNKS {
        data.extend(chunk(*b"MTrk", &[]));
    }
    assert!(MidDecoder::variants(&data).is_err());
    let mut data = header(1, 1, 480);
    data.resize(smf::MAX_FILE_BYTES + 1, 0);
    assert!(MidDecoder::variants(&data).is_err());
}

#[test]
fn a_flood_of_notes_in_one_track_is_bounded() {
    // A million notes and one more in a track: refused, and the file stays far below the size limit.
    let mut body = vec![0, 0xFF, 0x03, 11];
    body.extend_from_slice(b"PART GUITAR");
    for _ in 0..1_000_001 {
        body.extend_from_slice(&[0, 0x90, 96, 100, 0, 0x80, 96, 0]);
    }
    let mut data = header(1, 2, 480);
    data.extend(Tk::new("t").bytes());
    data.extend(chunk(*b"MTrk", &body));
    assert!(data.len() < smf::MAX_FILE_BYTES);
    assert!(MidDecoder::variants(&data).is_err());
    // A track with a few thousand notes is fine.
    let mut track = guitar();
    for i in 0..20_000u64 {
        track = track.note(i * 480, 96 + (i % 5) as u8, 0);
    }
    let chart = decode(&file(&[track]));
    assert_eq!(chart.notes.len(), 20_000);
}

#[test]
fn decoded_charts_always_validate() {
    // `decode` validates; a messy chart (chords, holds, tempo changes, equal times) passes it.
    let tempo = Tk::new("t")
        .tempo(0, 500_000)
        .tempo(300, 400_000)
        .tempo(301, 900_000);
    let track = guitar()
        .note(0, 96, 1000)
        .note(0, 97, 300)
        .note(300, 96, 10)
        .note(301, 98, 0)
        .note(302, 98, 0);
    let data = midi(1, 480, &[tempo, track]);
    assert!(MidDecoder::decode(&data).is_ok());
}

// --------------------------------------------------------------------------------------------- the audio plan

fn plan_names(plan: &rox::audio_plan::AudioPlan) -> Vec<String> {
    plan.events.iter().map(|e| e.file.to_string()).collect()
}

#[test]
fn the_plan_is_the_one_of_a_chart_without_stream_tags() {
    let data = file(&[guitar().note(0, 96, 0)]);
    let (_, _, plan) =
        MidDecoder::decode_with_audio_plan(&data, &ChartContext::default(), 0).unwrap();
    // Every reserved stem as an optional `.ogg` (14 of them), none from a tag.
    assert_eq!(plan.events.len(), 14);
    assert!(plan.events.iter().all(|e| e.optional && e.time_us == 0));
    assert_eq!(plan_names(&plan)[0], "song.ogg");
    let files: Vec<String> = [
        "song.opus",
        "guitar.ogg",
        "drums_1.ogg",
        "drums.ogg",
        "notes.mid",
        "song.ini",
        "x/rhythm.ogg",
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    let ctx = ChartContext {
        ini: Some(&b"[song]\ndelay = 100\n"[..]),
        files: Some(files.as_slice()),
    };
    let (chart, rep, plan) = MidDecoder::decode_with_audio_plan(&data, &ctx, 0).unwrap();
    assert_eq!(
        plan_names(&plan),
        ["song.opus", "guitar.ogg", "drums_1.ogg"]
    );
    assert!(plan.events.iter().all(|e| !e.optional && e.time_us == 0));
    // The delay is in the chart, the stems start at 0.
    assert_eq!(chart.notes[0].time_us, 100_000);
    assert_eq!(rep.shift_us, 100_000);
}

// ------------------------------------------------------------------------------------------------------ .sng

#[test]
fn a_sng_may_hold_a_notes_mid_and_prefers_it_to_a_notes_chart() {
    let mid = file(&[
        guitar().note(0, 96, 0).note(480, 98, 0),
        Tk::new("PART BASS").note(0, 60, 0),
    ]);
    let chart_text = "[Song]\n{\n  Resolution = 192\n}\n[ExpertSingle]\n{\n  0 = N 0 0\n}\n";
    let only_mid = SngBuilder::new()
        .meta("name", "Mid Song")
        .meta("delay", "100")
        .file("notes.mid", &mid)
        .file("song.ogg", b"a")
        .file("guitar.ogg", b"g")
        .build();
    let package = SngPackage::open(&only_mid).unwrap();
    let variants = package.variants().unwrap();
    assert_eq!(
        variants
            .iter()
            .map(|v| v.section.as_str())
            .collect::<Vec<_>>(),
        ["PART GUITAR Expert", "PART BASS Easy"]
    );
    let chart = package.decode_variant(0).unwrap();
    assert_eq!(chart.metadata.title, "Mid Song");
    assert_eq!(times(&chart), [100_000, 600_000]);
    let (_, _, plan) = package.decode_with_audio_plan("pack.sng", 1).unwrap();
    assert_eq!(plan.events.len(), 2);
    assert!(package.decode_variant(2).is_err());
    // Both files: notes.mid wins (YARG.Core tries notes.mid first), whatever their order.
    for order in [0, 1] {
        let mut builder = SngBuilder::new();
        builder = if order == 0 {
            builder
                .file("notes.chart", chart_text.as_bytes())
                .file("NOTES.MID", &mid)
        } else {
            builder
                .file("notes.mid", &mid)
                .file("notes.chart", chart_text.as_bytes())
        };
        let bytes = builder.build();
        let package = SngPackage::open(&bytes).unwrap();
        assert_eq!(package.variants().unwrap().len(), 2);
    }
    // A notes.mid that is not a MIDI file is an error (no fallback to the chart, as in the reader of YARG.Core).
    let bad = SngBuilder::new()
        .file("notes.mid", b"MThd")
        .file("notes.chart", chart_text.as_bytes())
        .build();
    assert!(SngPackage::open(&bad).unwrap().variants().is_err());
}

// -------------------------------------------------------------------------------------------------- dispatch

#[test]
fn auto_decode_routes_a_notes_mid_by_its_name_and_reads_its_song_ini() {
    use crate::formats::auto::detect::{DetectedFormat, detect};
    use crate::formats::{auto_decode, auto_decode_all, auto_decode_variant};
    let dir = std::env::temp_dir().join(format!("rox-clone-hero-mid-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let data = file(&[guitar().note(480, 96, 0).note(0, 84, 0)]);
    std::fs::write(dir.join("notes.mid"), &data).unwrap();
    std::fs::write(
        dir.join("Song.INI"),
        "[Song]\nname = Folder Song\ndelay = 50\n",
    )
    .unwrap();
    let first = auto_decode(dir.join("notes.mid")).unwrap();
    assert_eq!(first.metadata.title, "Folder Song");
    assert_eq!(times(&first), [550_000]);
    let second = auto_decode_variant(dir.join("notes.mid"), 1).unwrap();
    assert_eq!(second.metadata.difficulty_name, "Guitar Hard");
    assert!(auto_decode_variant(dir.join("notes.mid"), 2).is_err());
    let all = auto_decode_all(dir.join("notes.mid")).unwrap().unwrap();
    assert_eq!(
        all.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
        ["PART GUITAR Expert", "PART GUITAR Hard"]
    );
    assert!(all.iter().all(|v| v.chart.is_ok()));
    assert_eq!(all[0].chart.as_ref().unwrap().metadata.title, "Folder Song");
    // The name is case-insensitive.
    std::fs::write(dir.join("NOTES.MID"), &data).unwrap();
    assert_eq!(
        detect(&dir.join("NOTES.MID")).unwrap(),
        DetectedFormat::CloneHeroMid
    );
    // Any other .mid is an ordinary MIDI file: not a chart.
    std::fs::write(dir.join("song.mid"), &data).unwrap();
    assert!(auto_decode(dir.join("song.mid")).is_err());
    assert!(auto_decode_variant(dir.join("song.mid"), 0).is_err());
    assert!(auto_decode_all(dir.join("song.mid")).unwrap().is_none());
    // A broken notes.mid is an error, not a panic.
    std::fs::write(dir.join("notes.mid"), b"MThd nope").unwrap();
    assert!(auto_decode(dir.join("notes.mid")).is_err());
    assert!(auto_decode_all(dir.join("notes.mid")).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn auto_decode_bytes_and_encode_follow_the_name() {
    use crate::formats::{auto_decode_bytes, auto_encode};
    let data = file(&[guitar().note(0, 96, 0)]);
    assert_eq!(
        auto_decode_bytes("x/notes.mid", &data).unwrap().notes.len(),
        1
    );
    assert!(auto_decode_bytes("x/other.mid", &data).is_err());
    let chart = decode(&data);
    let path = std::env::temp_dir().join(format!("rox-notes-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    assert!(
        auto_encode(&chart, path.join("notes.mid")).is_err(),
        "read, not written"
    );
    std::fs::remove_dir_all(&path).unwrap();
}
