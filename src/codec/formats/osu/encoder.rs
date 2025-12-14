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

        // [General]
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

        // [Editor]
        output.push_str("[Editor]\n");
        output.push_str("DistanceSpacing: 1\n");
        output.push_str("BeatDivisor: 4\n");
        output.push_str("GridSize: 4\n");
        output.push_str("TimelineZoom: 1\n\n");

        // [Metadata]
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
        let _ = writeln!(
            output,
            "BeatmapSetID:{}",
            chart.metadata.chartset_id.map_or(-1, |id| id as i64)
        );
        output.push('\n');

        // [Difficulty]
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

        // [Events]
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

        // [TimingPoints]
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

        // [HitObjects]
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

        Ok(output.into_bytes())
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
