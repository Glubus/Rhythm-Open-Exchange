//! [WIP / UNSTABLE] Encoder for converting `RoxChart` to FNF .json format.
//!
//! > [!WARNING]
//! > This encoder is currently Work-In-Progress and may not be fully accurate.

use crate::codec::Encoder;
use crate::error::RoxResult;
use crate::model::RoxChart;

use super::types::{FnfChart, FnfNote, FnfSection, FnfSong};

/// Encoder for Friday Night Funkin' charts.
pub struct FnfEncoder;

impl Encoder for FnfEncoder {
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        // Get base BPM from first timing point
        let base_bpm = chart
            .timing_points
            .iter()
            .find(|tp| !tp.is_inherited)
            .map_or(120.0, |tp| tp.bpm);

        // Determine if this is 8K (both sides) or 4K (player only)
        let is_8k = chart.key_count() >= 8;

        // Create a single large section with all notes
        // This matches the JS converter approach
        let mut section_notes: Vec<FnfNote> = Vec::new();

        for note in &chart.notes {
            #[allow(clippy::cast_precision_loss)]
            let time_ms = note.time_us as f64 / 1000.0;

            // Map columns to FNF lanes
            let lane = if is_8k {
                // 8K: columns 0-3 = opponent (lanes 0-3), columns 4-7 = player (lanes 4-7)
                note.column
            } else {
                // 4K: all notes go to player side (lanes 0-3)
                note.column
            };

            let fnf_note = match &note.note_type {
                crate::model::NoteType::Hold { duration_us } => {
                    #[allow(clippy::cast_precision_loss)]
                    let duration_ms = *duration_us as f64 / 1000.0;
                    FnfNote::hold(time_ms, lane, duration_ms)
                }
                _ => FnfNote::tap(time_ms, lane),
            };

            section_notes.push(fnf_note);
        }

        let section = FnfSection {
            section_notes,
            length_in_steps: 160_000, // Large number to contain all notes
            must_hit_section: !is_8k, // true for 4K (player), false for 8K
            change_bpm: true,
            bpm: base_bpm,
            type_of_section: 0,
        };

        let fnf = FnfChart {
            song: FnfSong {
                song: chart.metadata.title.clone(),
                bpm: base_bpm,
                speed: 1.5,
                player1: "bf".to_string(),
                player2: chart.metadata.creator.clone(),
                needs_voices: false,
                valid_score: true,
                notes: vec![section],
                sections: 1,
                section_lengths: Vec::new(),
            },
        };

        // Serialize to pretty JSON
        let json = serde_json::to_string_pretty(&fnf)
            .map_err(|e| crate::error::RoxError::InvalidFormat(format!("JSON error: {e}")))?;

        Ok(json.into_bytes())
    }
}

#[cfg(test)]
mod tests {

    #[test]
    #[cfg(feature = "analysis")]
    #[ignore = "FNF is currently WIP/Unstable"]
    fn test_roundtrip_both() {
        use super::*;
        use crate::analysis::RoxAnalysis;
        use crate::codec::Decoder;
        use crate::codec::formats::fnf::FnfDecoder;
        let data = crate::test_utils::get_test_asset("fnf/test-song.json");
        // Decode both sides (8K)
        let chart1 = FnfDecoder::decode(&data).unwrap();
        let encoded = FnfEncoder::encode(&chart1).unwrap();
        let chart2 = FnfDecoder::decode(&encoded).unwrap();

        assert_eq!(chart1.key_count(), chart2.key_count());
        assert_eq!(
            chart1.notes_hash(),
            chart2.notes_hash(),
            "Notes hash mismatch"
        );
        // FNF only has one BPM for the whole song in this encoder implementation currently,
        // but let's check timings hash anyway.
        assert_eq!(
            chart1.timings_hash(),
            chart2.timings_hash(),
            "Timings hash mismatch"
        );
    }
}
