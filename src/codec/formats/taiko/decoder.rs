//! Decoder for converting osu!taiko to `RoxChart` (4K).
//!
//! Converts Taiko drums to a 4K layout:
//! - Columns 0, 3: Kats (rim hits) - alternating
//! - Columns 1, 2: Dons (center hits) - alternating
//! - Big notes (Finish): Hit both columns at once

use crate::codec::Decoder;
use crate::error::RoxResult;
use crate::model::{Metadata, Note, RoxChart, TimingPoint};

use super::types::{AlternationState, ColumnLayout};
use crate::codec::formats::taiko::parser;

/// Decoder for osu!taiko beatmaps.
pub struct TaikoDecoder;

impl TaikoDecoder {
    /// Decode with a specific column layout.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing fails.
    pub fn decode_with_layout(data: &[u8], layout: ColumnLayout) -> RoxResult<RoxChart> {
        let mut state = AlternationState::new(layout);
        Self::decode_with_state(data, &mut state)
    }

    /// Decode with custom state (useful for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the data is not valid UTF-8 or has invalid format.
    pub fn decode_with_state(data: &[u8], state: &mut AlternationState) -> RoxResult<RoxChart> {
        let beatmap = parser::parse(data)?;

        // Taiko converts to 4K
        let mut chart = RoxChart::new(4);

        // Map metadata (reusing OsuBeatmap fields)
        chart.metadata = Metadata {
            title: beatmap
                .metadata
                .title_unicode
                .clone()
                .unwrap_or_else(|| beatmap.metadata.title.clone()),
            artist: beatmap
                .metadata
                .artist_unicode
                .clone()
                .unwrap_or_else(|| beatmap.metadata.artist.clone()),
            creator: beatmap.metadata.creator.clone(),
            difficulty_name: beatmap.metadata.version.clone(),
            difficulty_value: Some(beatmap.difficulty.overall_difficulty),
            audio_file: beatmap.general.audio_filename.clone(),
            background_file: beatmap.background.clone(),
            audio_offset_us: i64::from(beatmap.general.audio_lead_in) * 1000,
            preview_time_us: if beatmap.general.preview_time > 0 {
                i64::from(beatmap.general.preview_time) * 1000
            } else {
                0
            },
            source: beatmap.metadata.source.clone(),
            tags: beatmap.metadata.tags.clone(),
            ..Default::default()
        };

        // Convert BPM timing points
        for tp in &beatmap.timing_points {
            #[allow(clippy::cast_possible_truncation)]
            let time_us = (tp.time * 1000.0) as i64;

            if tp.uninherited {
                if let Some(bpm) = tp.bpm() {
                    let mut timing = TimingPoint::bpm(time_us, bpm);
                    timing.signature = tp.meter;
                    chart.timing_points.push(timing);
                }
            } else {
                // SV logic if needed, but Taiko SV is complex.
                // For now, let's stick to BPM.
            }
        }

        // Ensure at least one BPM point
        if chart.timing_points.is_empty() {
            chart.timing_points.push(TimingPoint::bpm(0, 120.0));
        }

        // Convert hit objects
        for ho in &beatmap.hit_objects {
            // Skip spinners
            if ho.is_spinner() {
                continue;
            }

            #[allow(clippy::cast_possible_truncation)]
            let time_us = (ho.time_ms * 1000.0) as i64;
            let is_big = ho.hitsound.is_big();

            // Determine columns based on note type
            let columns = if ho.hitsound.is_kat() {
                state.next_kat_columns(is_big)
            } else {
                // Default to Don (including empty hitsound)
                state.next_don_columns(is_big)
            };

            // Create notes for each column
            for col in columns {
                chart.notes.push(Note::tap(time_us, col));
            }
        }

        // Sort notes by time
        chart.notes.sort_by_key(|n| n.time_us);

        Ok(chart)
    }
}

impl Decoder for TaikoDecoder {
    fn decode(data: &[u8]) -> RoxResult<RoxChart> {
        let mut state = AlternationState::default();
        Self::decode_with_state(data, &mut state)
    }
}
