#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use super::super::types::{OsuDifficulty, OsuGeneral, OsuMetadata};

pub fn parse_general(line: &str, general: &mut OsuGeneral) {
    let Some((key, value)) = line.split_once(':') else {
        return;
    };
    let value = value.trim();
    match key.trim() {
        "AudioFilename" => general.audio_filename = value.to_string(),
        "AudioLeadIn" => general.audio_lead_in = value.parse().unwrap_or(0),
        "PreviewTime" => general.preview_time = value.parse().unwrap_or(-1),
        "Mode" => general.mode = value.parse().unwrap_or(0),
        _ => {}
    }
}

pub fn parse_metadata(line: &str, metadata: &mut OsuMetadata) {
    let Some((key, value)) = line.split_once(':') else {
        return;
    };
    let value = value.trim();
    match key.trim() {
        "Title" => metadata.title = value.to_string(),
        "TitleUnicode" => metadata.title_unicode = Some(value.to_string()),
        "Artist" => metadata.artist = value.to_string(),
        "ArtistUnicode" => metadata.artist_unicode = Some(value.to_string()),
        "Creator" => metadata.creator = value.to_string(),
        "Version" => metadata.version = value.to_string(),
        "Source" if !value.is_empty() => metadata.source = Some(value.to_string()),
        "Tags" => metadata.tags = value.split_whitespace().map(ToString::to_string).collect(),
        "BeatmapID" => metadata.beatmap_id = value.parse().ok(),
        "BeatmapSetID" => metadata.beatmap_set_id = value.parse().ok(),
        _ => {}
    }
}

pub fn parse_difficulty(line: &str, difficulty: &mut OsuDifficulty) {
    let Some((key, value)) = line.split_once(':') else {
        return;
    };
    let value = value.trim();
    match key.trim() {
        "CircleSize" => difficulty.circle_size = value.parse().unwrap_or(4.0),
        "OverallDifficulty" => difficulty.overall_difficulty = value.parse().unwrap_or(5.0),
        "HPDrainRate" => difficulty.hp_drain_rate = value.parse().unwrap_or(5.0),
        _ => {}
    }
}

pub fn parse_event(line: &str, background: &mut Option<String>) {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() >= 3 && parts[0] == "0" && parts[1] == "0" {
        let filename = parts[2].trim_matches('"');
        if !filename.is_empty() {
            *background = Some(filename.to_string());
        }
    }
}
