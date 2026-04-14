#![warn(clippy::pedantic)]
use rox::codec::Encoder;
use rox::error::RoxResult;
use rox::model::RoxChart;
use rox_macros::Format;

use super::types::{QuaChart, QuaHitObject, QuaSliderVelocity, QuaTimingPoint};

#[derive(Format)]
#[format(extensions = ["qua"])]
pub struct QuaEncoder;

impl Encoder for QuaEncoder {
    fn encode_inner(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        let qua = to_qua(chart);
        let s = serde_yaml::to_string(&qua)
            .map_err(|e| rox::error::RoxError::Serialize(e.to_string()))?;
        Ok(s.into_bytes())
    }
}

fn build_qua_timing_points(chart: &RoxChart) -> Vec<QuaTimingPoint> {
    chart
        .timing_points
        .iter()
        .filter(|tp| !tp.is_sv())
        .map(|tp| {
            #[allow(clippy::cast_precision_loss)] // µs→ms: precision loss negligible at audio scale
            let start_time = tp.time_us() as f64 / 1000.0;
            QuaTimingPoint { start_time, bpm: tp.bpm_value().unwrap_or(120.0), signature: None }
        })
        .collect()
}

fn build_qua_slider_velocities(chart: &RoxChart) -> Vec<QuaSliderVelocity> {
    chart
        .timing_points
        .iter()
        .filter(|tp| tp.is_sv())
        .map(|tp| {
            #[allow(clippy::cast_precision_loss)] // µs→ms: precision loss negligible at audio scale
            let start_time = tp.time_us() as f64 / 1000.0;
            QuaSliderVelocity {
                start_time,
                multiplier: f64::from(tp.scroll_speed().unwrap_or(1.0)),
            }
        })
        .collect()
}

fn build_qua_hit_objects(chart: &RoxChart) -> Vec<QuaHitObject> {
    chart
        .notes
        .iter()
        .map(|note| {
            #[allow(clippy::cast_precision_loss)] // µs→ms: precision loss negligible at audio scale
            let start_time = note.time_us as f64 / 1000.0;
            let lane = note.column + 1; // Quaver lanes are 1-indexed
            let end_time = if note.end_time_us() > note.time_us {
                #[allow(clippy::cast_precision_loss)] // µs→ms: precision loss negligible at audio scale
                Some(note.end_time_us() as f64 / 1000.0)
            } else {
                None
            };
            QuaHitObject { start_time, lane, end_time }
        })
        .collect()
}

fn to_qua(chart: &RoxChart) -> QuaChart {
    QuaChart {
        audio_file: chart.metadata.audio_file.to_string(),
        #[allow(clippy::cast_possible_truncation)] // ms→µs: safe for any realistic timestamp
        preview_time: (chart.metadata.preview_time_us / 1000) as i32,
        background_file: chart.metadata.background_file.as_ref().map(ToString::to_string),
        map_id: chart.metadata.chart_id.and_then(|id| i32::try_from(id).ok()).unwrap_or(-1),
        title: chart.metadata.title.to_string(),
        artist: chart.metadata.artist.to_string(),
        creator: chart.metadata.creator.to_string(),
        difficulty_name: chart.metadata.difficulty_name.to_string(),
        source: chart.metadata.source.as_ref().map(ToString::to_string),
        tags: Some(
            chart
                .metadata
                .tags
                .iter()
                .map(compact_str::CompactString::as_str)
                .collect::<Vec<_>>()
                .join(" "),
        ),
        bpm_does_not_affect_sv: true,
        initial_scroll_velocity: 1.0,
        timing_points: build_qua_timing_points(chart),
        slider_velocities: build_qua_slider_velocities(chart),
        hit_objects: build_qua_hit_objects(chart),
        ..QuaChart::default()
    }
}
