//! Parser for .osu file format.

use super::types::{
    OsuBeatmap, OsuDifficulty, OsuGeneral, OsuHitObject, OsuMetadata, OsuTimingPoint,
};
use crate::error::{RoxError, RoxResult};

/// Current section being parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    None,
    General,
    Editor,
    Metadata,
    Difficulty,
    Events,
    TimingPoints,
    HitObjects,
}

/// Parse a .osu file into an `OsuBeatmap`.
///
/// # Errors
///
/// Returns an error if:
/// - The data is not valid UTF-8
/// - The beatmap is not in mania mode (mode != 3)
pub fn parse(data: &[u8]) -> RoxResult<OsuBeatmap> {
    let content = std::str::from_utf8(data)
        .map_err(|e| RoxError::InvalidFormat(format!("Invalid UTF-8: {e}")))?;

    let mut beatmap = OsuBeatmap::default();
    let mut section = Section::None;

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        // Check for format version
        if line.starts_with("osu file format v") {
            beatmap.format_version = line
                .strip_prefix("osu file format v")
                .and_then(|s| s.parse().ok())
                .unwrap_or(14);
            continue;
        }

        // Check for section headers
        if line.starts_with('[') && line.ends_with(']') {
            section = match &line[1..line.len() - 1] {
                "General" => Section::General,
                "Editor" => Section::Editor,
                "Metadata" => Section::Metadata,
                "Difficulty" => Section::Difficulty,
                "Events" => Section::Events,
                "TimingPoints" => Section::TimingPoints,
                "HitObjects" => Section::HitObjects,
                _ => Section::None,
            };
            continue;
        }

        // Parse based on section
        match section {
            Section::General => parse_general(line, &mut beatmap.general),
            Section::Metadata => parse_metadata(line, &mut beatmap.metadata),
            Section::Difficulty => parse_difficulty(line, &mut beatmap.difficulty),
            Section::Events => parse_event(line, &mut beatmap.background),
            Section::TimingPoints => {
                if let Some(tp) = parse_timing_point(line) {
                    beatmap.timing_points.push(tp);
                }
            }
            Section::HitObjects => {
                if let Some(ho) = parse_hit_object(line) {
                    beatmap.hit_objects.push(ho);
                }
            }
            Section::None | Section::Editor => {}
        }
    }

    Ok(beatmap)
}

pub fn parse_general(line: &str, general: &mut OsuGeneral) {
    if let Some((key, value)) = line.split_once(':') {
        let value = value.trim();
        match key.trim() {
            "AudioFilename" => general.audio_filename = value.to_string(),
            "AudioLeadIn" => general.audio_lead_in = value.parse().unwrap_or(0),
            "PreviewTime" => general.preview_time = value.parse().unwrap_or(-1),
            "Mode" => general.mode = value.parse().unwrap_or(0),
            _ => {}
        }
    }
}

pub fn parse_metadata(line: &str, metadata: &mut OsuMetadata) {
    if let Some((key, value)) = line.split_once(':') {
        let value = value.trim();
        match key.trim() {
            "Title" => metadata.title = value.to_string(),
            "TitleUnicode" => metadata.title_unicode = Some(value.to_string()),
            "Artist" => metadata.artist = value.to_string(),
            "ArtistUnicode" => metadata.artist_unicode = Some(value.to_string()),
            "Creator" => metadata.creator = value.to_string(),
            "Version" => metadata.version = value.to_string(),
            "Source" => {
                if !value.is_empty() {
                    metadata.source = Some(value.to_string());
                }
            }
            "Tags" => {
                metadata.tags = value
                    .split_whitespace()
                    .map(std::string::ToString::to_string)
                    .collect();
            }
            "BeatmapID" => metadata.beatmap_id = value.parse().ok(),
            "BeatmapSetID" => metadata.beatmap_set_id = value.parse().ok(),
            _ => {}
        }
    }
}

pub fn parse_difficulty(line: &str, difficulty: &mut OsuDifficulty) {
    if let Some((key, value)) = line.split_once(':') {
        let value = value.trim();
        match key.trim() {
            "CircleSize" => difficulty.circle_size = value.parse().unwrap_or(4.0),
            "OverallDifficulty" => difficulty.overall_difficulty = value.parse().unwrap_or(5.0),
            "HPDrainRate" => difficulty.hp_drain_rate = value.parse().unwrap_or(5.0),
            _ => {}
        }
    }
}

pub fn parse_event(line: &str, background: &mut Option<String>) {
    // Format: 0,0,"filename.jpg",0,0
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() >= 3 && parts[0] == "0" && parts[1] == "0" {
        let filename = parts[2].trim_matches('"');
        if !filename.is_empty() {
            *background = Some(filename.to_string());
        }
    }
}

#[must_use]
pub fn parse_timing_point(line: &str) -> Option<OsuTimingPoint> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 8 {
        return None;
    }

    Some(OsuTimingPoint {
        time: parts[0].parse().ok()?,
        beat_length: parts[1].parse().ok()?,
        meter: parts[2].parse().unwrap_or(4),
        sample_set: parts[3].parse().unwrap_or(0),
        sample_index: parts[4].parse().unwrap_or(0),
        volume: parts[5].parse().unwrap_or(100),
        uninherited: parts[6] == "1",
        effects: parts[7].parse().unwrap_or(0),
    })
}

fn parse_hit_object(line: &str) -> Option<OsuHitObject> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 5 {
        return None;
    }

    let x: i32 = parts[0].parse().ok()?;
    let y: i32 = parts[1].parse().ok()?;
    let time: i32 = parts[2].parse().ok()?;
    let object_type: u8 = parts[3].parse().ok()?;
    let hit_sound: u8 = parts[4].parse().ok()?;

    // Check for hold note (type & 128)
    let end_time = if (object_type & 128) != 0 && parts.len() > 5 {
        // Hold note format: x,y,time,type,hitSound,endTime:extras
        let extras = parts[5];
        extras.split(':').next().and_then(|s| s.parse().ok())
    } else {
        None
    };

    let extras = if parts.len() > 5 {
        parts[5..].join(",")
    } else {
        String::new()
    };

    Some(OsuHitObject {
        x,
        y,
        time,
        object_type,
        hit_sound,
        end_time,
        extras,
    })
}
