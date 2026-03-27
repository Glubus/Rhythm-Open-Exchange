#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};

use rox::codec::Encoder;
use rox::error::RoxResult;
use rox::model::{NoteType, RoxChart, TimingPoint};
use rox_macros::Format;

use super::types::{FnfChart, FnfNote, FnfSection, FnfSong};

#[derive(Format)]
#[format(extensions = ["json"])]
pub struct FnfEncoder;

impl Encoder for FnfEncoder {
    fn encode_inner(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        let fnf = to_fnf(chart);
        serde_json::to_vec_pretty(&fnf)
            .map_err(|e| rox::error::RoxError::Serialize(e.to_string()))
    }
}

fn to_fnf(chart: &RoxChart) -> FnfChart {
    let base_bpm = chart
        .timing_points
        .iter()
        .find(|tp| tp.is_bpm())
        .and_then(TimingPoint::bpm_value)
        .unwrap_or(120.0);
    let is_8k = chart.key_count >= 8;
    let notes = build_section_notes(chart);
    FnfChart {
        song: FnfSong {
            song: chart.metadata.title.to_string(),
            bpm: base_bpm,
            speed: 1.0,
            player1: "bf".to_string(),
            player2: chart.metadata.creator.to_string(),
            notes: vec![FnfSection {
                section_notes: notes,
                length_in_steps: 160_000,
                must_hit_section: !is_8k,
                change_bpm: true,
                bpm: base_bpm,
            }],
            ..FnfSong::default()
        },
    }
}

fn build_section_notes(chart: &RoxChart) -> Vec<FnfNote> {
    chart
        .notes
        .iter()
        .map(|note| {
            #[allow(clippy::cast_precision_loss)]
            let time_ms = note.time_us as f64 / 1000.0;
            match &note.note_type {
                NoteType::Hold { duration_us } | NoteType::Burst { duration_us } => {
                    #[allow(clippy::cast_precision_loss)]
                    let dur_ms = *duration_us as f64 / 1000.0;
                    FnfNote::hold(time_ms, note.column, dur_ms)
                }
                _ => FnfNote::tap(time_ms, note.column),
            }
        })
        .collect()
}
