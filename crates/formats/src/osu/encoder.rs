#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use core::fmt::Write;
use rox::codec::Encoder;
use rox::error::RoxResult;
use rox::model::{NoteType, RoxChart, TimingPoint};
use rox_macros::Format;

/// Encoder for osu!mania beatmaps.
#[derive(Format)]
#[format(extensions = ["osu"])]
pub struct OsuEncoder;

impl Encoder for OsuEncoder {
    fn encode_inner(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        let mut out = String::new();
        out.push_str("osu file format v14\n\n");
        write_general(&mut out, chart);
        write_editor(&mut out);
        write_metadata(&mut out, chart);
        write_difficulty(&mut out, chart);
        write_events(&mut out, chart);
        write_timing_points(&mut out, chart);
        write_hit_objects(&mut out, chart);
        Ok(out.into_bytes())
    }
}

/// Convert column index to X position for osu!mania.
#[must_use]
pub fn column_to_x(column: u8, key_count: u8) -> i32 {
    let col = i32::from(column);
    let k = i32::from(key_count);
    (2 * col + 1) * 256 / k
}

fn write_general(out: &mut String, chart: &RoxChart) {
    out.push_str("[General]\n");
    let _ = writeln!(out, "AudioFilename: {}", chart.metadata.audio_file);
    let _ = writeln!(
        out,
        "AudioLeadIn: {}",
        chart.metadata.audio_offset_us / 1000
    );
    let _ = writeln!(
        out,
        "PreviewTime: {}",
        chart.metadata.preview_time_us / 1000
    );
    out.push_str("Countdown: 0\nSampleSet: Normal\nStackLeniency: 0.7\nMode: 3\n");
    out.push_str("LetterboxInBreaks: 0\nSpecialStyle: 0\nWidescreenStoryboard: 0\n\n");
}

fn write_editor(out: &mut String) {
    out.push_str("[Editor]\nDistanceSpacing: 1\nBeatDivisor: 4\nGridSize: 4\nTimelineZoom: 1\n\n");
}

fn write_metadata(out: &mut String, chart: &RoxChart) {
    out.push_str("[Metadata]\n");
    let _ = writeln!(out, "Title:{}", chart.metadata.title);
    let _ = writeln!(out, "TitleUnicode:{}", chart.metadata.title);
    let _ = writeln!(out, "Artist:{}", chart.metadata.artist);
    let _ = writeln!(out, "ArtistUnicode:{}", chart.metadata.artist);
    let _ = writeln!(out, "Creator:{}", chart.metadata.creator);
    let _ = writeln!(out, "Version:{}", chart.metadata.difficulty_name);
    if let Some(src) = &chart.metadata.source {
        let _ = writeln!(out, "Source:{src}");
    }
    if !chart.metadata.tags.is_empty() {
        let tags: Vec<&str> = chart
            .metadata
            .tags
            .iter()
            .map(compact_str::CompactString::as_str)
            .collect();
        let _ = writeln!(out, "Tags:{}", tags.join(" "));
    }
    let _ = writeln!(out, "BeatmapID:{}", chart.metadata.chart_id.unwrap_or(0));
    #[allow(clippy::cast_possible_wrap)] // ms→µs i64: safe for any realistic timestamp or duration
    let _ = writeln!(
        out,
        "BeatmapSetID:{}",
        chart.metadata.chartset_id.map_or(-1i64, |id| id as i64)
    );
    out.push('\n');
}

fn write_difficulty(out: &mut String, chart: &RoxChart) {
    out.push_str("[Difficulty]\nHPDrainRate:8\n");
    let _ = writeln!(out, "CircleSize:{}", chart.key_count);
    let _ = writeln!(
        out,
        "OverallDifficulty:{}",
        chart.metadata.difficulty_value.unwrap_or(8.0)
    );
    out.push_str("ApproachRate:5\nSliderMultiplier:1.4\nSliderTickRate:1\n\n");
}

fn write_events(out: &mut String, chart: &RoxChart) {
    out.push_str("[Events]\n//Background and Video events\n");
    if let Some(bg) = &chart.metadata.background_file {
        let _ = writeln!(out, "0,0,\"{bg}\",0,0");
    }
    out.push_str("//Break Periods\n//Storyboard Layer 0 (Background)\n\n");
}

fn write_timing_points(out: &mut String, chart: &RoxChart) {
    out.push_str("[TimingPoints]\n");
    for tp in &chart.timing_points {
        #[allow(clippy::cast_precision_loss)]
        // i64→f64: precision loss acceptable for timing values
        let time_ms = tp.time_us() as f64 / 1000.0;
        match tp {
            TimingPoint::Sv { scroll_speed, .. } => {
                let beat_length = -100.0 / f64::from(*scroll_speed);
                let _ = writeln!(out, "{time_ms},{beat_length},4,1,0,100,0,0");
            }
            TimingPoint::Bpm { bpm, signature, .. } => {
                let beat_length = 60_000.0 / f64::from(*bpm);
                let _ = writeln!(out, "{time_ms},{beat_length},{signature},1,0,100,1,0");
            }
        }
    }
    out.push_str("\n\n");
}

fn write_hit_objects(out: &mut String, chart: &RoxChart) {
    out.push_str("[HitObjects]\n");
    for note in &chart.notes {
        #[allow(clippy::cast_possible_truncation)]
        // ms→µs i64: safe for any realistic timestamp or duration
        let time_ms = (note.time_us / 1000) as i32;
        let x = column_to_x(note.column, chart.key_count);
        match &note.note_type {
            NoteType::Tap | NoteType::Mine => {
                let _ = writeln!(out, "{x},192,{time_ms},1,0,0:0:0:0:");
            }
            NoteType::Hold { duration_us } | NoteType::Burst { duration_us } => {
                #[allow(clippy::cast_possible_truncation)]
                // ms→µs i64: safe for any realistic timestamp or duration
                let end_time = time_ms + (*duration_us / 1000) as i32;
                let _ = writeln!(out, "{x},192,{time_ms},128,0,{end_time}:0:0:0:0:");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::{
        codec::Encoder,
        model::{Note, TimingPoint},
    };
    use rstest::rstest;

    #[rstest]
    #[case(4, &[64, 192, 320, 448])]
    #[case(7, &[36, 109, 182, 256, 329, 402, 475])]
    fn test_column_to_x(#[case] key_count: u8, #[case] expected: &[i32]) {
        for (col, &x) in expected.iter().enumerate() {
            assert_eq!(column_to_x(col as u8, key_count), x);
        }
    }

    #[rstest]
    fn test_encode_produces_valid_osu() {
        let mut chart = rox::model::RoxChart::new(4);
        chart.metadata.title = "Test".into();
        chart.metadata.audio_file = "audio.mp3".into();
        chart.timing_points.push(TimingPoint::bpm(0, 180.0));
        chart.notes.push(Note::tap(1_000_000, 0));
        let encoded = OsuEncoder::encode(&chart).expect("encode failed");
        let s = core::str::from_utf8(&encoded).expect("utf8");
        assert!(s.contains("osu file format v14"));
        assert!(s.contains("Mode: 3"));
        assert!(s.contains("CircleSize:4"));
    }
}
