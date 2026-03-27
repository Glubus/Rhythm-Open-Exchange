#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use rox::codec::Decoder;
use rox::error::RoxResult;
use rox::model::{Metadata, Note, RoxChart, TimingPoint};
use rox_macros::Format;

use super::parser;
use super::types::{FnfChart, FnfSection, FnfSide};

#[derive(Format)]
#[format(extensions = ["json"])]
pub struct FnfDecoder;

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
    let key_count = match side {
        FnfSide::Both => 8,
        _ => 4,
    };
    let mut chart = RoxChart::new(key_count);
    chart.metadata = build_metadata(fnf, side);
    build_timing_and_notes(fnf, side, &mut chart);
    chart.notes.sort_by_key(|n| n.time_us);
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

fn build_timing_and_notes(fnf: &FnfChart, side: FnfSide, chart: &mut RoxChart) {
    let mut added_initial_bpm = false;
    let mut current_bpm = fnf.song.bpm;
    for section in &fnf.song.notes {
        if section.change_bpm && section.bpm > 0.0 {
            if let Some(first) = section.section_notes.first() {
                #[allow(clippy::cast_possible_truncation)]
                let time_us = (first.time_ms() * 1000.0) as i64;
                chart.timing_points.push(TimingPoint::bpm(time_us, section.bpm));
                current_bpm = section.bpm;
            }
        } else if !added_initial_bpm {
            chart.timing_points.push(TimingPoint::bpm(0, current_bpm));
            added_initial_bpm = true;
        }
        add_section_notes(section, side, chart);
    }
    if !added_initial_bpm {
        chart.timing_points.push(TimingPoint::bpm(0, fnf.song.bpm));
    }
}

fn add_section_notes(section: &FnfSection, side: FnfSide, chart: &mut RoxChart) {
    for fnf_note in &section.section_notes {
        let raw_lane = fnf_note.lane();
        let (is_player, base_lane) = if raw_lane < 4 {
            (section.must_hit_section, raw_lane)
        } else {
            (!section.must_hit_section, raw_lane - 4)
        };
        let column = match side {
            FnfSide::Player => {
                if is_player {
                    Some(base_lane)
                } else {
                    None
                }
            }
            FnfSide::Opponent => {
                if is_player {
                    None
                } else {
                    Some(base_lane)
                }
            }
            FnfSide::Both => Some(if is_player { base_lane + 4 } else { base_lane }),
        };
        if let Some(col) = column {
            #[allow(clippy::cast_possible_truncation)]
            let time_us = (fnf_note.time_ms() * 1000.0) as i64;
            let note = if fnf_note.is_hold() {
                #[allow(clippy::cast_possible_truncation)]
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

    use crate::fnf::encoder::FnfEncoder;

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
