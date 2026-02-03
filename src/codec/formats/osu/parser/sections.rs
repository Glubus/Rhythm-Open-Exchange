use super::super::types::{OsuDifficulty, OsuGeneral, OsuMetadata};

pub fn parse_general(line: &str, general: &mut OsuGeneral) {
    if let Some((key, value)) = line.split_once(':') {
        let value = value.trim();
        match key.trim() {
            "AudioFilename" => general.audio_filename = value.to_string(),
            "AudioLeadIn" => general.audio_lead_in = parse_field(value, "AudioLeadIn", 0),
            "PreviewTime" => general.preview_time = parse_field(value, "PreviewTime", -1),
            "Mode" => general.mode = parse_field(value, "Mode", 0),
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
            "CircleSize" => difficulty.circle_size = parse_field(value, "CircleSize", 4.0),
            "OverallDifficulty" => {
                difficulty.overall_difficulty = parse_field(value, "OverallDifficulty", 5.0);
            }
            "HPDrainRate" => difficulty.hp_drain_rate = parse_field(value, "HPDrainRate", 5.0),
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

/// Helper to parse fields and log on failure
fn parse_field<T: std::str::FromStr>(value: &str, field_name: &str, default: T) -> T {
    if let Ok(v) = value.parse() {
        v
    } else {
        tracing::warn!("Failed to parse {}: '{}', using default", field_name, value);
        default
    }
}
