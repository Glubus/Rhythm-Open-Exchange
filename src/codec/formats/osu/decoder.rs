//! Decoder for converting .osu to RoxChart.

use crate::codec::Decoder;
use crate::error::RoxResult;
use crate::model::{Metadata, Note, RoxChart, TimingPoint};

use super::parser;
use super::types::OsuBeatmap;

/// Decoder for osu!mania beatmaps.
pub struct OsuDecoder;

impl OsuDecoder {
    /// Convert an OsuBeatmap to RoxChart.
    pub fn from_beatmap(beatmap: &OsuBeatmap) -> RoxChart {
        let key_count = beatmap.difficulty.circle_size as u8;
        let mut chart = RoxChart::new(key_count);

        // Map metadata
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
            audio_offset_us: beatmap.general.audio_lead_in as i64 * 1000,
            preview_time_us: if beatmap.general.preview_time > 0 {
                beatmap.general.preview_time as i64 * 1000
            } else {
                0
            },
            source: beatmap.metadata.source.clone(),
            tags: beatmap.metadata.tags.clone(),
            ..Default::default()
        };

        // Convert timing points
        for tp in &beatmap.timing_points {
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

        // Convert hit objects to notes
        for ho in &beatmap.hit_objects {
            let column = ho.column(key_count);
            let time_us = ho.time as i64 * 1000;

            let note = if ho.is_hold() {
                let duration_us = ho.duration_ms() as i64 * 1000;
                Note::hold(time_us, duration_us, column)
            } else {
                Note::tap(time_us, column)
            };

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
        Ok(Self::from_beatmap(&beatmap))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_sample() {
        let data = include_bytes!("../../../../assets/osu/mania_7k.osu");
        let chart = OsuDecoder::decode(data).expect("Failed to decode");

        assert_eq!(chart.key_count, 7);
        assert!(!chart.notes.is_empty());
        assert!(!chart.timing_points.is_empty());
        assert_eq!(chart.metadata.difficulty_name, "7K Awakened");
    }
}
