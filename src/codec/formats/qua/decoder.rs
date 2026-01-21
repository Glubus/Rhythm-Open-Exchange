//! Decoder for converting .qua to `RoxChart`.

use crate::codec::Decoder;
use crate::error::RoxResult;
use crate::model::{Metadata, Note, RoxChart, TimingPoint};

use super::parser;
use super::types::QuaChart;

/// Decoder for Quaver beatmaps.
pub struct QuaDecoder;

impl QuaDecoder {
    /// Convert a `QuaChart` to `RoxChart`.
    #[must_use]
    pub fn from_qua(qua: &QuaChart) -> RoxChart {
        let key_count = qua.mode.key_count();
        let mut chart = RoxChart::new(key_count);

        // Map metadata
        chart.metadata = Metadata {
            // Map Quaver IDs (i32 -> Option<u64>)
            chart_id: if qua.map_id > 0 {
                #[allow(clippy::cast_sign_loss)]
                Some(qua.map_id as u64)
            } else {
                None
            },
            chartset_id: if qua.map_set_id > 0 {
                #[allow(clippy::cast_sign_loss)]
                Some(qua.map_set_id as u64)
            } else {
                None
            },
            key_count,
            title: qua.title.clone(),
            artist: qua.artist.clone(),
            creator: qua.creator.clone(),
            difficulty_name: qua.difficulty_name.clone(),
            audio_file: qua.audio_file.clone(),
            background_file: qua.background_file.clone(),
            preview_time_us: i64::from(qua.preview_time) * 1000,
            source: qua.source.clone(),
            tags: qua
                .tags
                .as_ref()
                .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default(),
            ..Default::default()
        };

        // Convert timing points (BPM)
        for tp in &qua.timing_points {
            // Safe: time in ms fits in i64 after multiplying by 1000
            #[allow(clippy::cast_possible_truncation)]
            let time_us = (tp.start_time * 1000.0) as i64;
            let mut timing = TimingPoint::bpm(time_us, tp.bpm);
            timing.signature = tp
                .signature
                .as_ref()
                .map_or(4, super::types::TimeSignature::beats);
            chart.timing_points.push(timing);
        }

        // Convert slider velocities to SV timing points
        for sv in &qua.slider_velocities {
            #[allow(clippy::cast_possible_truncation)]
            let time_us = (sv.start_time * 1000.0) as i64;
            #[allow(clippy::cast_possible_truncation)]
            let multiplier = sv.multiplier as f32;
            chart
                .timing_points
                .push(TimingPoint::sv(time_us, multiplier));
        }

        // Sort timing points by time
        chart.timing_points.sort_by_key(|tp| tp.time_us);

        // Convert hit objects
        for ho in &qua.hit_objects {
            #[allow(clippy::cast_possible_truncation)]
            let time_us = (ho.start_time * 1000.0) as i64;
            // Quaver lanes are 1-indexed
            let column = ho.lane.saturating_sub(1);

            let note = if let Some(end_time) = ho.end_time {
                #[allow(clippy::cast_possible_truncation)]
                let end_us = (end_time * 1000.0) as i64;
                let duration_us = end_us - time_us;
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

impl Decoder for QuaDecoder {
    fn decode(data: &[u8]) -> RoxResult<RoxChart> {
        let qua = parser::parse(data)?;
        Ok(Self::from_qua(&qua))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::Decoder;

    #[test]
    fn test_decode_asset_4k() {
        let data = crate::test_utils::get_test_asset("quaver/4K.qua");
        let chart = <QuaDecoder as Decoder>::decode(&data).expect("Failed to decode 4K.qua");

        // Basic validation
        assert_eq!(chart.key_count(), 4);
        assert!(!chart.notes.is_empty());
        assert!(!chart.timing_points.is_empty());
    }
}
