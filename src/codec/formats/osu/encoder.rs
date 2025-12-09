//! Encoder for converting RoxChart to .osu format.

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
        output.push_str(&format!("AudioFilename: {}\n", chart.metadata.audio_file));
        output.push_str(&format!(
            "AudioLeadIn: {}\n",
            chart.metadata.audio_offset_us / 1000
        ));
        output.push_str(&format!(
            "PreviewTime: {}\n",
            chart.metadata.preview_time_us / 1000
        ));
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
        output.push_str(&format!("Title:{}\n", chart.metadata.title));
        output.push_str(&format!("TitleUnicode:{}\n", chart.metadata.title));
        output.push_str(&format!("Artist:{}\n", chart.metadata.artist));
        output.push_str(&format!("ArtistUnicode:{}\n", chart.metadata.artist));
        output.push_str(&format!("Creator:{}\n", chart.metadata.creator));
        output.push_str(&format!("Version:{}\n", chart.metadata.difficulty_name));
        if let Some(source) = &chart.metadata.source {
            output.push_str(&format!("Source:{}\n", source));
        }
        if !chart.metadata.tags.is_empty() {
            output.push_str(&format!("Tags:{}\n", chart.metadata.tags.join(" ")));
        }
        output.push_str("BeatmapID:0\n");
        output.push_str("BeatmapSetID:-1\n\n");

        // [Difficulty]
        output.push_str("[Difficulty]\n");
        output.push_str("HPDrainRate:8\n");
        output.push_str(&format!("CircleSize:{}\n", chart.key_count));
        output.push_str(&format!(
            "OverallDifficulty:{}\n",
            chart.metadata.difficulty_value.unwrap_or(8.0)
        ));
        output.push_str("ApproachRate:5\n");
        output.push_str("SliderMultiplier:1.4\n");
        output.push_str("SliderTickRate:1\n\n");

        // [Events]
        output.push_str("[Events]\n");
        output.push_str("//Background and Video events\n");
        if let Some(bg) = &chart.metadata.background_file {
            output.push_str(&format!("0,0,\"{}\",0,0\n", bg));
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
            let time_ms = tp.time_us as f64 / 1000.0;

            if !tp.is_inherited {
                // BPM point: beatLength = 60000 / bpm
                let beat_length = 60000.0 / tp.bpm as f64;
                output.push_str(&format!(
                    "{},{},{},1,0,100,1,0\n",
                    time_ms, beat_length, tp.signature
                ));
            } else {
                // SV point: beatLength = -100 / sv
                let beat_length = -100.0 / tp.scroll_speed as f64;
                output.push_str(&format!("{},{},4,1,0,100,0,0\n", time_ms, beat_length));
            }
        }
        output.push_str("\n\n");

        // [HitObjects]
        output.push_str("[HitObjects]\n");
        for note in &chart.notes {
            let time_ms = (note.time_us / 1000) as i32;
            let x = column_to_x(note.column, chart.key_count);

            match &note.note_type {
                crate::model::NoteType::Tap => {
                    // x,y,time,type,hitSound,extras
                    output.push_str(&format!("{},192,{},1,0,0:0:0:0:\n", x, time_ms));
                }
                crate::model::NoteType::Hold { duration_us } => {
                    let end_time = time_ms + (*duration_us / 1000) as i32;
                    // x,y,time,type,hitSound,endTime:extras
                    output.push_str(&format!(
                        "{},192,{},128,0,{}:0:0:0:0:\n",
                        x, time_ms, end_time
                    ));
                }
                _ => {
                    // Burst and Mine - convert to tap for osu
                    output.push_str(&format!("{},192,{},1,0,0:0:0:0:\n", x, time_ms));
                }
            }
        }

        Ok(output.into_bytes())
    }
}

/// Convert column index to X position for osu.
/// For 7K: 36, 109, 182, 256, 329, 402, 475
pub fn column_to_x(column: u8, key_count: u8) -> i32 {
    // Use the same formula osu uses: x = floor(column * 512 / key_count) + floor(512 / key_count / 2)
    // For perfect centering, we calculate the column center position
    let column_width = 512.0 / key_count as f64;
    (column as f64 * column_width + column_width / 2.0).floor() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_to_x() {
        // 7K: 36, 109, 182, 256, 329, 402, 475
        assert_eq!(column_to_x(0, 7), 36);
        assert_eq!(column_to_x(1, 7), 109);
        assert_eq!(column_to_x(2, 7), 182);
        assert_eq!(column_to_x(3, 7), 256); // center
        assert_eq!(column_to_x(4, 7), 329);
        assert_eq!(column_to_x(5, 7), 402);
        assert_eq!(column_to_x(6, 7), 475);
    }
}
