#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use rox::codec::Decoder;
use rox::error::RoxResult;
use rox::model::{Metadata, Note, RoxChart, TimingPoint};

use super::parser;
use super::types::{FnfChart, FnfSection, FnfSide};

pub struct FnfDecoder;
crate::impl_format!(FnfDecoder, ["json"]);

impl FnfDecoder {
    /// Decode with explicit side selection.
    /// # Errors
    /// Returns an error if parsing fails.
    pub fn decode_with_side(data: &[u8], side: FnfSide) -> RoxResult<RoxChart> {
        let fnf = parser::parse(data)?;
        Ok(from_fnf(&fnf, side))
    }
}

impl Decoder for FnfDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        Self::decode_with_side(data, FnfSide::Player)
    }
}

#[must_use]
pub fn from_fnf(fnf: &FnfChart, side: FnfSide) -> RoxChart {
    let keys = fnf.song.keys_per_side();
    let key_count = match side {
        FnfSide::Both => keys * 2,
        _ => keys,
    };
    let mut chart = RoxChart::new(key_count);
    chart.metadata = build_metadata(fnf, side);
    build_timing_points(fnf, &mut chart);
    build_notes(fnf, side, &mut chart);
    // The game's own charts stack notes on a lane (two at once, or one inside a long note): one is played, the others dropped.
    chart.drop_overlapping_notes();
    chart.timing_points.sort_by_key(TimingPoint::time_us);
    chart
}

fn build_metadata(fnf: &FnfChart, side: FnfSide) -> Metadata {
    Metadata {
        title: fnf.song.song.clone().into(),
        artist: "Unknown".into(),
        creator: fnf.song.player2.clone().into(),
        difficulty_name: "Normal".into(),
        audio_file: "Inst.ogg".into(),
        source: Some("Friday Night Funkin'".into()),
        tags: vec!["fnf".into()],
        is_coop: side == FnfSide::Both,
        ..Metadata::default()
    }
}

/// The column of a note of lane `raw_lane` (`0..keys` one side, `keys..2*keys` the other); a lane beyond both sides is no note.
fn resolve_column(raw_lane: u8, must_hit_section: bool, side: FnfSide, keys: u8) -> Option<u8> {
    let (is_player, base_lane) = if raw_lane < keys {
        (must_hit_section, raw_lane)
    } else if raw_lane < keys * 2 {
        (!must_hit_section, raw_lane - keys)
    } else {
        return None;
    };
    match side {
        FnfSide::Player => is_player.then_some(base_lane),
        FnfSide::Opponent => (!is_player).then_some(base_lane),
        FnfSide::Both => Some(if is_player { base_lane + keys } else { base_lane }),
    }
}

fn build_timing_points(fnf: &FnfChart, chart: &mut RoxChart) {
    chart.timing_points.push(TimingPoint::bpm(0, fnf.song.bpm));
    for section in &fnf.song.notes {
        if section.change_bpm
            && section.bpm > 0.0
            && let Some(first) = section.section_notes.first()
        {
            #[allow(clippy::cast_possible_truncation)] // ms→µs: safe for any realistic timestamp
            let time_us = (first.time_ms() * 1000.0) as i64;
            if time_us > 0 {
                chart
                    .timing_points
                    .push(TimingPoint::bpm(time_us, section.bpm));
            }
        }
    }
}

fn build_notes(fnf: &FnfChart, side: FnfSide, chart: &mut RoxChart) {
    let absolute_lanes = fnf
        .song
        .format
        .as_deref()
        .is_some_and(|format| format.starts_with("psych_v1"));
    let keys = fnf.song.keys_per_side();
    for section in &fnf.song.notes {
        add_section_notes(section, absolute_lanes, side, keys, chart);
    }
}

fn add_section_notes(
    section: &FnfSection,
    absolute_lanes: bool,
    side: FnfSide,
    keys: u8,
    chart: &mut RoxChart,
) {
    // `psych_v1`: lanes 0-3 are the player's, 4-7 the opponent's, `mustHitSection` only moves the camera
    // (Psych Engine `PlayState.generateSong`: `gottaHitNote = noteData < 4`).
    let must_hit = absolute_lanes || section.must_hit_section;
    for fnf_note in section.section_notes.iter().filter(|note| note.is_note()) {
        if let Some(col) = resolve_column(fnf_note.lane(), must_hit, side, keys) {
            #[allow(clippy::cast_possible_truncation)] // ms→µs: safe for any realistic timestamp
            let time_us = (fnf_note.time_ms() * 1000.0) as i64;
            let note = if fnf_note.is_hold() {
                #[allow(clippy::cast_possible_truncation)] // ms→µs: safe for any realistic duration
                let dur_us = (fnf_note.duration_ms() * 1000.0) as i64;
                Note::hold(time_us, dur_us, col)
            } else {
                Note::tap(time_us, col)
            };
            chart.notes.push(note);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::codec::Decoder;
    use rox::codec::Encoder;
    use rstest::rstest;

    use crate::formats::fnf::encoder::FnfEncoder;

    #[rstest]
    fn test_decode_player_side() {
        let data = rox_test_utils::get_test_asset("fnf/test-song.json");
        let chart = FnfDecoder::decode(&data).expect("decode failed");
        assert_eq!(chart.key_count, 4);
        assert!(!chart.notes.is_empty());
        assert!(!chart.timing_points.is_empty());
    }

    #[rstest]
    fn test_decode_both_sides() {
        let data = rox_test_utils::get_test_asset("fnf/test-song.json");
        let chart = FnfDecoder::decode_with_side(&data, FnfSide::Both).expect("decode failed");
        assert_eq!(chart.key_count, 8);
        assert!(chart.metadata.is_coop);
    }

    fn psych_json(format: &str) -> Vec<u8> {
        format!(
            r#"{{"song":{{"song":"t","bpm":120,{format}"notes":[
            {{"mustHitSection":false,"sectionNotes":[[0,0,0],[100,4,0]]}},
            {{"mustHitSection":true,"sectionNotes":[[200,1,0],[300,5,0]]}}]}}}}"#
        )
        .into_bytes()
    }

    fn columns(data: &[u8], side: FnfSide) -> Vec<(i64, u8)> {
        let chart = FnfDecoder::decode_with_side(data, side).expect("decode failed");
        chart.notes.iter().map(|n| (n.time_us, n.column)).collect()
    }

    #[rstest]
    fn psych_v1_lanes_ignore_must_hit_section() {
        let data = psych_json(r#""format":"psych_v1","#);
        assert_eq!(columns(&data, FnfSide::Player), vec![(0, 0), (200_000, 1)]);
        assert_eq!(
            columns(&data, FnfSide::Opponent),
            vec![(100_000, 0), (300_000, 1)]
        );
    }

    #[rstest]
    fn legacy_lanes_follow_must_hit_section() {
        let data = psych_json("");
        assert_eq!(
            columns(&data, FnfSide::Player),
            vec![(100_000, 0), (200_000, 1)]
        );
    }

    /// What real charts carry (the game's v0.2.x files, Psych, Kade, Troll Engine): a note type after the duration, an event
    /// in the notes (lane -1), `null`/`false`/text in fields the chart does not need, NUL padding after the object. The
    /// notes are read and the rest is ignored.
    #[rstest]
    fn engine_extras_and_padding_are_ignored() {
        let mut data = br#"{"song":{"song":"t","bpm":120,"speed":null,"needsVoices":"yes","notes":[
            {"mustHitSection":true,"bpm":false,"lengthInSteps":null,"sectionNotes":[[0,0,0,"Hurt Note"],[100,-1,"Hey!","",""],[200,1,50.5,"No Animation"],["x",2,0]]}]}}"#
            .to_vec();
        data.extend([0u8; 64]);
        assert_eq!(columns(&data, FnfSide::Player), vec![(0, 0), (200_000, 1)]);
        let chart = FnfDecoder::decode(&data).expect("decode failed");
        assert_eq!(chart.notes[1].duration_us(), 50_500);
    }

    /// The game writes `changeBPM`: a section that sets it changes the tempo from its start.
    #[rstest]
    fn change_bpm_as_the_game_writes_it_adds_a_timing_point() {
        let data = br#"{"song":{"song":"t","bpm":120,"notes":[
            {"mustHitSection":true,"sectionNotes":[[0,0,0]]},
            {"mustHitSection":true,"changeBPM":true,"bpm":180,"sectionNotes":[[2000,1,0]]}]}}"#;
        let chart = FnfDecoder::decode(data).expect("decode failed");
        let bpms: Vec<f32> = chart.timing_points.iter().filter_map(|point| match point {
            TimingPoint::Bpm { bpm, .. } => Some(*bpm),
            _ => None,
        }).collect();
        assert_eq!(bpms, [120.0, 180.0]);
    }

    /// A chart without the `{"song": {...}}` wrapper (its `song` is the name) is read the same, and notes stacked on a lane (the game's
    /// own Senpai chart has two at once) keep one, the long note first.
    #[rstest]
    fn a_flat_chart_is_read_and_stacked_notes_keep_one() {
        let data = br#"{"song":"Stress","bpm":120,"notes":[{"mustHitSection":true,"sectionNotes":[[0,1,0],[0,1,300],[100,1,0],[300,1,0],[300,2,0]]}]}"#;
        let chart = FnfDecoder::decode(data).expect("decode failed");
        assert_eq!(chart.metadata.title, "Stress");
        let notes: Vec<(i64, u8, i64)> = chart.notes.iter().map(|note| (note.time_us, note.column, note.duration_us())).collect();
        assert_eq!(notes, [(0, 1, 300_000), (300_000, 1, 0), (300_000, 2, 0)]);
    }

    /// An engine with more lanes says how many per side (`keys`, NightmareVision): 5 keys give lanes 0-4 and 5-9, read on 5 columns.
    #[rstest]
    fn keys_per_side_widen_the_chart() {
        let data = br#"{"song":{"song":"t","bpm":120,"keys":5,"notes":[{"mustHitSection":true,"sectionNotes":[[0,4,0],[100,9,0],[200,12,0]]}]}}"#;
        let chart = FnfDecoder::decode(data).expect("decode failed");
        assert_eq!(chart.key_count, 5);
        assert_eq!(chart.notes.iter().map(|note| note.column).collect::<Vec<_>>(), [4]);
        assert_eq!(FnfDecoder::decode_with_side(data, FnfSide::Both).expect("both").key_count, 10);
    }

    #[rstest]
    fn test_roundtrip() {
        let data = rox_test_utils::get_test_asset("fnf/test-song.json");
        let chart = FnfDecoder::decode(&data).expect("decode failed");
        let encoded = FnfEncoder::encode(&chart).expect("encode failed");
        let chart2 = FnfDecoder::decode(&encoded).expect("decode 2 failed");
        assert_eq!(chart.key_count, chart2.key_count);
        assert_eq!(chart.notes.len(), chart2.notes.len());
    }
}
