//! Encoder for converting `RoxChart` to .qua format.

use crate::codec::Encoder;
use crate::error::RoxResult;
use crate::model::RoxChart;

use super::types::{QuaChart, QuaHitObject, QuaSliderVelocity, QuaTimingPoint};

/// Encoder for Quaver beatmaps.
pub struct QuaEncoder;

impl Encoder for QuaEncoder {
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        use compact_str::CompactString;

        let mut qua = QuaChart {
            audio_file: chart.metadata.audio_file.to_string(),
            // Safe: preview_time_us / 1000 fits in i32 for typical beatmaps
            #[allow(clippy::cast_possible_truncation)]
            preview_time: (chart.metadata.preview_time_us / 1000) as i32,
            background_file: Some(
                chart
                    .metadata
                    .background_file
                    .as_ref()
                    .unwrap_or(&CompactString::new(""))
                    .to_string(),
            ),
            map_id: if let Some(id) = chart.metadata.chart_id {
                id as i32
            } else {
                -1
            },
            title: chart.metadata.title.to_string(),
            artist: chart.metadata.artist.to_string(),
            creator: chart.metadata.creator.to_string(),
            difficulty_name: chart.metadata.difficulty_name.to_string(),
            source: Some(
                chart
                    .metadata
                    .source
                    .clone()
                    .unwrap_or_default()
                    .to_string(),
            ),
            tags: Some(
                chart
                    .metadata
                    .tags
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            description: None,
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

#[cfg(test)]
mod tests {

    #[test]
    fn test_roundtrip() {
        use super::*;
        use crate::codec::formats::qua::QuaDecoder;
        use crate::codec::Decoder;
        let data = crate::test_utils::get_test_asset("quaver/4K.qua");
        let chart1 = QuaDecoder::decode(&data).unwrap();
        let encoded = QuaEncoder::encode(&chart1).unwrap();
        let chart2 = QuaDecoder::decode(&encoded).unwrap();

        assert_eq!(chart1.key_count(), chart2.key_count());

        // Use deep comparison with tolerance instead of hashes due to YAML float rounding
        assert_eq!(chart1.notes.len(), chart2.notes.len());
        for (n1, n2) in chart1.notes.iter().zip(chart2.notes.iter()) {
            assert_eq!(n1.column, n2.column);
            assert!(
                (n1.time_us - n2.time_us).abs() <= 1000,
                "Note time mismatch"
            );
        }

        assert_eq!(chart1.timing_points.len(), chart2.timing_points.len());
        for (tp1, tp2) in chart1.timing_points.iter().zip(chart2.timing_points.iter()) {
            assert!(
                (tp1.time_us - tp2.time_us).abs() <= 1000,
                "Timing point time mismatch"
            );
            if !tp1.is_inherited {
                assert!((tp1.bpm - tp2.bpm).abs() < 0.01, "BPM mismatch");
            }
        }
    }
}
