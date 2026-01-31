//! Encoder for converting `RoxChart` to .osu format.

use std::fmt::Write;

use crate::codec::Encoder;
use crate::error::RoxResult;
use crate::model::RoxChart;

/// Encoder for osu!mania beatmaps.
pub struct OsuEncoder;

impl Encoder for OsuEncoder {
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        let mut output = String::new();

        // Format version
        output.push_str("osu file format v14\n\n");

        write_general_section(&mut output, chart);
        write_editor_section(&mut output);
        write_metadata_section(&mut output, chart);
        write_difficulty_section(&mut output, chart);
        write_events_section(&mut output, chart);
        write_timing_points_section(&mut output, chart);
        write_hit_objects_section(&mut output, chart);

        Ok(output.into_bytes())
    }
}

/// Write the [General] section.
fn write_general_section(output: &mut String, chart: &RoxChart) {
    output.push_str("[General]\n");
    let _ = writeln!(output, "AudioFilename: {}", chart.metadata.audio_file);
    let _ = writeln!(
        output,
        "AudioLeadIn: {}",
        chart.metadata.audio_offset_us / 1000
    );
    let _ = writeln!(
        output,
        "PreviewTime: {}",
        chart.metadata.preview_time_us / 1000
    );
    output.push_str("Countdown: 0\n");
    output.push_str("SampleSet: Normal\n");
    output.push_str("StackLeniency: 0.7\n");
    output.push_str("Mode: 3\n");
    output.push_str("LetterboxInBreaks: 0\n");
    output.push_str("SpecialStyle: 0\n");
    output.push_str("WidescreenStoryboard: 0\n\n");
}

/// Write the [Editor] section.
fn write_editor_section(output: &mut String) {
    output.push_str("[Editor]\n");
    output.push_str("DistanceSpacing: 1\n");
    output.push_str("BeatDivisor: 4\n");
    output.push_str("GridSize: 4\n");
    output.push_str("TimelineZoom: 1\n\n");
}

/// Write the [Metadata] section.
fn write_metadata_section(output: &mut String, chart: &RoxChart) {
    output.push_str("[Metadata]\n");
    let _ = writeln!(output, "Title:{}", chart.metadata.title);
    let _ = writeln!(output, "TitleUnicode:{}", chart.metadata.title);
    let _ = writeln!(output, "Artist:{}", chart.metadata.artist);
    let _ = writeln!(output, "ArtistUnicode:{}", chart.metadata.artist);
    let _ = writeln!(output, "Creator:{}", chart.metadata.creator);
    let _ = writeln!(output, "Version:{}", chart.metadata.difficulty_name);
    if let Some(source) = &chart.metadata.source {
        let _ = writeln!(output, "Source:{source}");
    }
    if !chart.metadata.tags.is_empty() {
        let _ = writeln!(output, "Tags:{}", chart.metadata.tags.join(" "));
    }
    // Export chart IDs (default to 0/-1 if not set)
    let _ = writeln!(output, "BeatmapID:{}", chart.metadata.chart_id.unwrap_or(0));
    // Safe: osu format uses -1 for missing set ID
    #[allow(clippy::cast_possible_wrap)]
    let _ = writeln!(
        output,
        "BeatmapSetID:{}",
        chart.metadata.chartset_id.map_or(-1, |id| id as i64)
    );
    output.push('\n');
}

/// Write the [Difficulty] section.
fn write_difficulty_section(output: &mut String, chart: &RoxChart) {
    output.push_str("[Difficulty]\n");
    output.push_str("HPDrainRate:8\n");
    let _ = writeln!(output, "CircleSize:{}", chart.key_count());
    let _ = writeln!(
        output,
        "OverallDifficulty:{}",
        chart.metadata.difficulty_value.unwrap_or(8.0)
    );
    output.push_str("ApproachRate:5\n");
    output.push_str("SliderMultiplier:1.4\n");
    output.push_str("SliderTickRate:1\n\n");
}

/// Write the [Events] section.
fn write_events_section(output: &mut String, chart: &RoxChart) {
    output.push_str("[Events]\n");
    output.push_str("//Background and Video events\n");
    if let Some(bg) = &chart.metadata.background_file {
        let _ = writeln!(output, "0,0,\"{bg}\",0,0");
    }
    output.push_str("//Break Periods\n");
    output.push_str("//Storyboard Layer 0 (Background)\n");
    output.push_str("//Storyboard Layer 1 (Fail)\n");
    output.push_str("//Storyboard Layer 2 (Pass)\n");
    output.push_str("//Storyboard Layer 3 (Foreground)\n");
    output.push_str("//Storyboard Sound Samples\n\n");
}

/// Write the [`TimingPoints`] section.
fn write_timing_points_section(output: &mut String, chart: &RoxChart) {
    output.push_str("[TimingPoints]\n");
    for tp in &chart.timing_points {
        #[allow(clippy::cast_precision_loss)]
        let time_ms = tp.time_us as f64 / 1000.0;

        if tp.is_inherited {
            // SV point: beatLength = -100 / sv
            let beat_length = -100.0 / f64::from(tp.scroll_speed);
            let _ = writeln!(output, "{time_ms},{beat_length},4,1,0,100,0,0");
        } else {
            // BPM point: beatLength = 60000 / bpm
            let beat_length = 60000.0 / f64::from(tp.bpm);
            let _ = writeln!(
                output,
                "{},{},{},1,0,100,1,0",
                time_ms, beat_length, tp.signature
            );
        }
    }
    output.push_str("\n\n");
}

/// Write the [`HitObjects`] section.
fn write_hit_objects_section(output: &mut String, chart: &RoxChart) {
    output.push_str("[HitObjects]\n");
    for note in &chart.notes {
        // Safe: time_us / 1000 fits in i32 for typical beatmaps
        #[allow(clippy::cast_possible_truncation)]
        let time_ms = (note.time_us / 1000) as i32;
        let x = column_to_x(note.column, chart.key_count());

        match &note.note_type {
            crate::model::NoteType::Tap => {
                // x,y,time,type,hitSound,extras
                let _ = writeln!(output, "{x},192,{time_ms},1,0,0:0:0:0:");
            }
            crate::model::NoteType::Hold { duration_us } => {
                #[allow(clippy::cast_possible_truncation)]
                let end_time = time_ms + (*duration_us / 1000) as i32;
                // x,y,time,type,hitSound,endTime:extras
                let _ = writeln!(output, "{x},192,{time_ms},128,0,{end_time}:0:0:0:0:");
            }
            crate::model::NoteType::Burst { .. } | crate::model::NoteType::Mine => {
                // Burst and Mine - convert to tap for osu
                let _ = writeln!(output, "{x},192,{time_ms},1,0,0:0:0:0:");
            }
        }
    }
}

/// Convert column index to X position for osu.
/// For 7K: 36, 109, 182, 256, 329, 402, 475
#[must_use]
pub fn column_to_x(column: u8, key_count: u8) -> i32 {
    // Formula: center of column = (2*column + 1) * 256 / key_count
    // Use integer arithmetic to avoid floating-point precision errors
    let column = i32::from(column);
    let key_count = i32::from(key_count);
    (2 * column + 1) * 256 / key_count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Note, TimingPoint};

    /// Helper to verify all columns for a key count
    fn verify_columns(key_count: u8, expected: &[i32]) {
        assert_eq!(
            expected.len(),
            key_count as usize,
            "Wrong number of expected values for {}K",
            key_count
        );
        for (col, &expected_x) in expected.iter().enumerate() {
            let actual = column_to_x(col as u8, key_count);
            assert_eq!(
                actual, expected_x,
                "{}K column {} failed: expected {}, got {}",
                key_count, col, expected_x, actual
            );
        }
    }

    #[test]
    fn test_column_to_x_4k() {
        verify_columns(4, &[64, 192, 320, 448]);
    }

    #[test]
    fn test_column_to_x_5k() {
        verify_columns(5, &[51, 153, 256, 358, 460]);
    }

    #[test]
    fn test_column_to_x_6k() {
        verify_columns(6, &[42, 128, 213, 298, 384, 469]);
    }

    #[test]
    fn test_column_to_x_7k() {
        verify_columns(7, &[36, 109, 182, 256, 329, 402, 475]);
    }

    #[test]
    fn test_column_to_x_8k() {
        verify_columns(8, &[32, 96, 160, 224, 288, 352, 416, 480]);
    }

    #[test]
    fn test_column_to_x_9k() {
        verify_columns(9, &[28, 85, 142, 199, 256, 312, 369, 426, 483]);
    }

    #[test]
    fn test_column_to_x_10k() {
        verify_columns(10, &[25, 76, 128, 179, 230, 281, 332, 384, 435, 486]);
    }

    #[test]
    fn test_column_to_x_12k() {
        verify_columns(
            12,
            &[21, 64, 106, 149, 192, 234, 277, 320, 362, 405, 448, 490],
        );
    }

    #[test]
    fn test_column_to_x_14k() {
        verify_columns(
            14,
            &[
                18, 54, 91, 128, 164, 201, 237, 274, 310, 347, 384, 420, 457, 493,
            ],
        );
    }

    #[test]
    fn test_column_to_x_16k() {
        verify_columns(
            16,
            &[
                16, 48, 80, 112, 144, 176, 208, 240, 272, 304, 336, 368, 400, 432, 464, 496,
            ],
        );
    }

    #[test]
    fn test_column_to_x_18k() {
        verify_columns(
            18,
            &[
                14, 42, 71, 99, 128, 156, 184, 213, 241, 270, 298, 327, 355, 384, 412, 440, 469,
                497,
            ],
        );
    }

    #[test]
    fn test_column_roundtrip() {
        for key_count in [4, 5, 6, 7, 8, 9, 10] {
            for col in 0..key_count {
                let x = column_to_x(col, key_count);
                #[allow(clippy::cast_possible_truncation)]
                let decoded_col = ((x * i32::from(key_count)) / 512) as u8;
                assert_eq!(
                    decoded_col, col,
                    "Roundtrip failed for {}K column {}",
                    key_count, col
                );
            }
        }
    }

    #[test]
    fn test_encode_basic() {
        let mut chart = RoxChart::new(7);
        chart.metadata.title = "Test".into();
        chart.metadata.artist = "Artist".into();
        chart.metadata.creator = "Mapper".into();
        chart.metadata.difficulty_name = "Hard".into();
        chart.metadata.audio_file = "audio.mp3".into();
        chart.timing_points.push(TimingPoint::bpm(0, 180.0));
        chart.notes.push(Note::tap(1_000_000, 0));
        chart.notes.push(Note::tap(1_500_000, 3));
        chart.notes.push(Note::hold(2_000_000, 500_000, 6));

        let encoded = OsuEncoder::encode(&chart).unwrap();
        let output = String::from_utf8_lossy(&encoded);

        assert!(output.contains("osu file format v14"));
        assert!(output.contains("Mode: 3"));
        assert!(output.contains("CircleSize:7"));
    }

    #[test]
    #[cfg(feature = "analysis")]
    fn test_roundtrip() {
        use crate::analysis::RoxAnalysis;
        use crate::codec::formats::osu::OsuDecoder;
        use crate::codec::Decoder;
        let data = crate::test_utils::get_test_asset("osu/mania_7k.osu");
        let chart1 = <OsuDecoder as Decoder>::decode(&data).unwrap();
        let encoded = OsuEncoder::encode(&chart1).unwrap();
        let chart2 = <OsuDecoder as Decoder>::decode(&encoded).unwrap();

        assert_eq!(chart1.key_count(), chart2.key_count());

        // Compare using hashes
        assert_eq!(
            chart1.notes_hash(),
            chart2.notes_hash(),
            "Notes hash mismatch"
        );
        assert_eq!(
            chart1.timings_hash(),
            chart2.timings_hash(),
            "Timings hash mismatch"
        );
    }
}
