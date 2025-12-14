//! Decoder for converting .osu to `RoxChart`.

use std::collections::HashMap;

use crate::codec::Decoder;
use crate::error::RoxResult;
use crate::model::{Hitsound, Metadata, Note, RoxChart, TimingPoint};

use super::parser;
use super::types::OsuBeatmap;

/// Decoder for osu!mania beatmaps.
pub struct OsuDecoder;

impl OsuDecoder {
    /// Convert an `OsuBeatmap` to `RoxChart`.
    #[must_use]
    pub fn from_beatmap(beatmap: &OsuBeatmap) -> RoxChart {
        // Safe: circle_size is always 4-18 for mania which fits in u8
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let key_count = beatmap.difficulty.circle_size as u8;
        let mut chart = RoxChart::new(key_count);

        // Map metadata
        chart.metadata = Metadata {
            // Map osu! IDs
            chart_id: beatmap.metadata.beatmap_id.map(|id| id as u64),
            chartset_id: beatmap.metadata.beatmap_set_id.map(|id| id as u64),
            key_count,
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

        // Convert timing points
        for tp in &beatmap.timing_points {
            // Safe: time in ms fits in i64 after multiplying by 1000
            #[allow(clippy::cast_possible_truncation)]
            let time_us = (tp.time * 1000.0) as i64;

            if tp.uninherited {
                // BPM point
                if let Some(bpm) = tp.bpm() {
                    let mut timing = TimingPoint::bpm(time_us, bpm);
                    timing.signature = tp.meter;
                    chart.timing_points.push(timing);
                }
            } else {
                // SV point
                let sv = tp.scroll_velocity();
                chart.timing_points.push(TimingPoint::sv(time_us, sv));
            }
        }

        // Map to track unique hitsound files and their indices
        let mut hitsound_map: HashMap<String, u16> = HashMap::new();

        // Convert hit objects to notes
        for ho in &beatmap.hit_objects {
            let column = ho.column(key_count);
            let time_us = i64::from(ho.time) * 1000;

            let mut note = if ho.is_hold() {
                let duration_us = i64::from(ho.duration_ms()) * 1000;
                Note::hold(time_us, duration_us, column)
            } else {
                Note::tap(time_us, column)
            };

            // Parse hitsound from extras
            // Format: endTime:sampleSet:additions:customIndex:volume:filename
            // Or for taps: sampleSet:additions:customIndex:volume:filename
            if !ho.extras.is_empty() {
                let parts: Vec<&str> = ho.extras.split(':').collect();

                // For holds, the first part is endTime, so filename is at index 5
                // For taps, filename is at index 4 (if present)
                let filename_idx = if ho.is_hold() { 5 } else { 4 };

                if let Some(&filename) = parts.get(filename_idx) {
                    let filename = filename.trim();
                    if !filename.is_empty() {
                        // Get or create hitsound index
                        let hitsound_index = if let Some(&idx) = hitsound_map.get(filename) {
                            idx
                        } else {
                            // Parse volume from extras (index 4 for holds, 3 for taps)
                            let volume_idx = if ho.is_hold() { 4 } else { 3 };
                            let volume: Option<u8> = parts
                                .get(volume_idx)
                                .and_then(|v| v.parse().ok())
                                .filter(|&v| v > 0 && v <= 100);

                            let hitsound = if let Some(vol) = volume {
                                Hitsound::with_volume(filename, vol)
                            } else {
                                Hitsound::new(filename)
                            };

                            let idx = chart.hitsounds.len() as u16;
                            chart.hitsounds.push(hitsound);
                            hitsound_map.insert(filename.to_string(), idx);
                            idx
                        };

                        note.hitsound_index = Some(hitsound_index);
                    }
                }
            }

            chart.notes.push(note);
        }

        // Sort notes by time
        chart.notes.sort_by_key(|n| n.time_us);

        chart
    }
}

impl Decoder for OsuDecoder {
    fn decode(data: &[u8]) -> RoxResult<RoxChart> {
        let beatmap = parser::parse(data)?;

        // Validate it's mania mode (3)
        if beatmap.general.mode != 3 {
            return Err(crate::error::RoxError::InvalidFormat(format!(
                "Not a mania beatmap (mode={}, expected 3)",
                beatmap.general.mode
            )));
        }

        Ok(Self::from_beatmap(&beatmap))
    }
}
