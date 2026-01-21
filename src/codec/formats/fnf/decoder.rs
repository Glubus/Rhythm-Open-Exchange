//! Decoder for converting FNF .json to `RoxChart`.

use crate::codec::Decoder;
use crate::error::RoxResult;
use crate::model::{Metadata, Note, RoxChart, TimingPoint};

use super::parser;
use super::types::{FnfChart, FnfSide};

/// Decoder for Friday Night Funkin' charts.
pub struct FnfDecoder;

impl FnfDecoder {
    /// Decode with a specific side selection.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing fails.
    pub fn decode_with_side(data: &[u8], side: FnfSide) -> RoxResult<RoxChart> {
        let fnf = parser::parse(data)?;
        Ok(Self::from_fnf(&fnf, side))
    }

    /// Convert an `FnfChart` to `RoxChart` with the specified side.
    #[must_use]
    pub fn from_fnf(fnf: &FnfChart, side: FnfSide) -> RoxChart {
        let key_count = match side {
            FnfSide::Player | FnfSide::Opponent => 4,
            FnfSide::Both => 8,
        };

        let mut chart = RoxChart::new(key_count);

        // Map metadata
        chart.metadata = Metadata {
            key_count,
            title: fnf.song.song.clone(),
            creator: fnf.song.player2.clone(),
            difficulty_name: "Normal".to_string(),
            is_coop: side == FnfSide::Both, // true for 8K coop mode
            ..Default::default()
        };

        // Track current BPM for timing points
        let mut current_bpm = fnf.song.bpm;
        let mut added_initial_bpm = false;

        // Process each section
        for section in &fnf.song.notes {
            // Handle BPM changes
            if section.change_bpm && section.bpm > 0.0 {
                // Find the first note time in this section for the timing point
                if let Some(first_note) = section.section_notes.first() {
                    #[allow(clippy::cast_possible_truncation)]
                    let time_us = (first_note.time_ms() * 1000.0) as i64;
                    chart
                        .timing_points
                        .push(TimingPoint::bpm(time_us, section.bpm));
                    current_bpm = section.bpm;
                }
            } else if !added_initial_bpm {
                // Add initial BPM at time 0
                chart.timing_points.push(TimingPoint::bpm(0, current_bpm));
                added_initial_bpm = true;
            }

            // Process notes in this section
            for fnf_note in &section.section_notes {
                let raw_lane = fnf_note.lane();

                // Determine if this note belongs to player or opponent
                // In FNF: mustHitSection determines which side is which
                // mustHitSection=true: lanes 0-3 = player, 4-7 = opponent
                // mustHitSection=false: lanes 0-3 = opponent, 4-7 = player
                let (is_player_note, base_lane) = if raw_lane < 4 {
                    (section.must_hit_section, raw_lane)
                } else {
                    (!section.must_hit_section, raw_lane - 4)
                };

                // Filter based on requested side
                let column = match side {
                    FnfSide::Player => {
                        if is_player_note {
                            Some(base_lane)
                        } else {
                            None
                        }
                    }
                    FnfSide::Opponent => {
                        if is_player_note {
                            None
                        } else {
                            Some(base_lane)
                        }
                    }
                    FnfSide::Both => {
                        // Opponent on left (0-3), player on right (4-7)
                        if is_player_note {
                            Some(base_lane + 4)
                        } else {
                            Some(base_lane)
                        }
                    }
                };

                if let Some(col) = column {
                    #[allow(clippy::cast_possible_truncation)]
                    let time_us = (fnf_note.time_ms() * 1000.0) as i64;

                    let note = if fnf_note.is_hold() {
                        #[allow(clippy::cast_possible_truncation)]
                        let duration_us = (fnf_note.duration_ms() * 1000.0) as i64;
                        Note::hold(time_us, duration_us, col)
                    } else {
                        Note::tap(time_us, col)
                    };

                    chart.notes.push(note);
                }
            }
        }

        // Add initial BPM if no sections had notes
        if !added_initial_bpm {
            chart.timing_points.push(TimingPoint::bpm(0, fnf.song.bpm));
        }

        // Sort notes and timing points by time
        chart.notes.sort_by_key(|n| n.time_us);
        chart.timing_points.sort_by_key(|tp| tp.time_us);

        chart
    }
}

impl Decoder for FnfDecoder {
    /// Decode FNF chart, extracting player notes only (4K).
    fn decode(data: &[u8]) -> RoxResult<RoxChart> {
        Self::decode_with_side(data, FnfSide::Player)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::Decoder;

    #[test]
    #[ignore = "FNF is currently WIP/Unstable"]
    fn test_decode_asset_fnf_player() {
        // assets/fnf/test-song.json
        let data = crate::test_utils::get_test_asset("fnf/test-song.json");
        let chart =
            <FnfDecoder as Decoder>::decode(&data).expect("Failed to decode test-song.json");

        // Basic validation
        assert_eq!(chart.key_count(), 4); // Player side is 4K
        assert!(!chart.notes.is_empty());
        assert!(!chart.timing_points.is_empty());
    }

    #[test]
    #[ignore = "FNF is currently WIP/Unstable"]
    fn test_decode_asset_fnf_both() {
        let data = crate::test_utils::get_test_asset("fnf/test-song.json");
        let chart = FnfDecoder::decode_with_side(&data, FnfSide::Both)
            .expect("Failed to decode both sides");

        assert_eq!(chart.key_count(), 8); // Both sides is 8K
        assert!(chart.metadata.is_coop);
    }
}
