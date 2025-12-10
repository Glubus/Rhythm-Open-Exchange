//! Encoder for converting `RoxChart` to .qua format.

use crate::codec::Encoder;
use crate::error::RoxResult;
use crate::model::RoxChart;

use super::types::{QuaChart, QuaHitObject, QuaMode, QuaSliderVelocity, QuaTimingPoint};

/// Encoder for Quaver beatmaps.
pub struct QuaEncoder;

impl Encoder for QuaEncoder {
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        let mode = match chart.key_count {
            7 => QuaMode::Keys7,
            // Default to 4K for unsupported key counts
            _ => QuaMode::Keys4,
        };

        let mut qua = QuaChart {
            audio_file: chart.metadata.audio_file.clone(),
            // Safe: preview_time_us / 1000 fits in i32 for typical beatmaps
            #[allow(clippy::cast_possible_truncation)]
            preview_time: (chart.metadata.preview_time_us / 1000) as i32,
            background_file: chart.metadata.background_file.clone(),
            mode,
            title: chart.metadata.title.clone(),
            artist: chart.metadata.artist.clone(),
            creator: chart.metadata.creator.clone(),
            difficulty_name: chart.metadata.difficulty_name.clone(),
            source: chart.metadata.source.clone(),
            tags: if chart.metadata.tags.is_empty() {
                None
            } else {
                Some(chart.metadata.tags.join(", "))
            },
            initial_scroll_velocity: 1.0,
            bpm_does_not_affect_sv: true,
            ..Default::default()
        };

        // Convert timing points
        for tp in &chart.timing_points {
            // Safe: time_us / 1000 is small enough for f64
            #[allow(clippy::cast_precision_loss)]
            let start_time = tp.time_us as f64 / 1000.0;

            if tp.is_inherited {
                // SV point
                qua.slider_velocities.push(QuaSliderVelocity {
                    start_time,
                    multiplier: f64::from(tp.scroll_speed),
                });
            } else {
                // BPM point
                qua.timing_points.push(QuaTimingPoint {
                    start_time,
                    bpm: tp.bpm,
                    signature: None,
                });
            }
        }

        // Convert notes
        for note in &chart.notes {
            #[allow(clippy::cast_precision_loss)]
            let start_time = note.time_us as f64 / 1000.0;
            // Quaver lanes are 1-indexed
            let lane = note.column + 1;

            let end_time = match &note.note_type {
                crate::model::NoteType::Hold { duration_us } => {
                    #[allow(clippy::cast_precision_loss)]
                    let end = (note.time_us + duration_us) as f64 / 1000.0;
                    Some(end)
                }
                _ => None,
            };

            qua.hit_objects.push(QuaHitObject {
                start_time,
                lane,
                end_time,
                key_sounds: Vec::new(),
            });
        }

        // Serialize to YAML
        let yaml = serde_yaml::to_string(&qua).map_err(|e| {
            crate::error::RoxError::InvalidFormat(format!("YAML encoding error: {e}"))
        })?;

        Ok(yaml.into_bytes())
    }
}
