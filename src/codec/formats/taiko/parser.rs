//! Parser for osu!taiko format.
//!
//! Reuses the osu parser but extracts Taiko-specific information.

use crate::error::{RoxError, RoxResult};

use super::types::{TaikoHitObject, TaikoHitsound};

/// Parsed Taiko beatmap data.
#[derive(Debug, Default)]
pub struct TaikoBeatmap {
    /// Song title.
    pub title: String,
    /// Artist name.
    pub artist: String,
    /// Mapper name.
    pub creator: String,
    /// Difficulty name.
    pub version: String,
    /// Audio filename.
    pub audio_file: String,
    /// Background filename.
    pub background: Option<String>,
    /// Audio offset in ms.
    pub offset_ms: i32,
    /// Preview time in ms.
    pub preview_time_ms: i32,
    /// BPM timing points: `(time_ms, bpm)`.
    pub bpm_changes: Vec<(f64, f32)>,
    /// Hit objects.
    pub hit_objects: Vec<TaikoHitObject>,
}

/// Parse a Taiko beatmap from raw bytes.
///
/// # Errors
///
/// Returns an error if the data is not valid UTF-8 or has invalid format.
pub fn parse(data: &[u8]) -> RoxResult<TaikoBeatmap> {
    let content = std::str::from_utf8(data)
        .map_err(|e| RoxError::InvalidFormat(format!("Invalid UTF-8: {e}")))?;

    let mut beatmap = TaikoBeatmap::default();
    let mut current_section = "";

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        // Section headers
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line;
            continue;
        }

        match current_section {
            "[General]" => parse_general_line(line, &mut beatmap),
            "[Metadata]" => parse_metadata_line(line, &mut beatmap),
            "[Events]" => parse_events_line(line, &mut beatmap),
            "[TimingPoints]" => parse_timing_line(line, &mut beatmap),
            "[HitObjects]" => parse_hit_object_line(line, &mut beatmap),
            _ => {}
        }
    }

    Ok(beatmap)
}

fn parse_general_line(line: &str, beatmap: &mut TaikoBeatmap) {
    if let Some((key, value)) = line.split_once(':') {
        let value = value.trim();
        match key.trim() {
            "AudioFilename" => beatmap.audio_file = value.to_string(),
            "PreviewTime" => beatmap.preview_time_ms = value.parse().unwrap_or(0),
            "Mode" => {
                // Verify it's Taiko mode (1)
                if value != "1" {
                    // We'll still try to parse, might work
                }
            }
            _ => {}
        }
    }
}

fn parse_metadata_line(line: &str, beatmap: &mut TaikoBeatmap) {
    if let Some((key, value)) = line.split_once(':') {
        let value = value.trim();
        match key.trim() {
            "Title" => {
                if beatmap.title.is_empty() {
                    beatmap.title = value.to_string();
                }
            }
            "TitleUnicode" => beatmap.title = value.to_string(),
            "Artist" => {
                if beatmap.artist.is_empty() {
                    beatmap.artist = value.to_string();
                }
            }
            "ArtistUnicode" => beatmap.artist = value.to_string(),
            "Creator" => beatmap.creator = value.to_string(),
            "Version" => beatmap.version = value.to_string(),
            _ => {}
        }
    }
}

fn parse_events_line(line: &str, beatmap: &mut TaikoBeatmap) {
    // Parse background: 0,0,"filename.jpg",0,0
    if let Some(rest) = line.strip_prefix("0,0,\"")
        && let Some(end) = rest.find('"')
    {
        beatmap.background = Some(rest[..end].to_string());
    }
}

fn parse_timing_line(line: &str, beatmap: &mut TaikoBeatmap) {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() >= 2 {
        let time_ms: f64 = parts[0].parse().unwrap_or(0.0);
        let beat_length: f64 = parts[1].parse().unwrap_or(0.0);

        // Check if uninherited (BPM point)
        let uninherited = if parts.len() >= 7 {
            parts[6].parse::<i32>().unwrap_or(1) == 1
        } else {
            beat_length > 0.0
        };

        if uninherited && beat_length > 0.0 {
            #[allow(clippy::cast_possible_truncation)]
            let bpm = (60000.0 / beat_length) as f32;
            beatmap.bpm_changes.push((time_ms, bpm));
        }
    }
}

fn parse_hit_object_line(line: &str, beatmap: &mut TaikoBeatmap) {
    let parts: Vec<&str> = line.split(',').collect();

    // Format: x,y,time,type,hitSound,...
    if parts.len() >= 5 {
        let time_ms: f64 = parts[2].parse().unwrap_or(0.0);
        let object_type: u32 = parts[3].parse().unwrap_or(0);
        let hitsound: u32 = parts[4].parse().unwrap_or(0);

        beatmap.hit_objects.push(TaikoHitObject {
            time_ms,
            hitsound: TaikoHitsound::from_bits_truncate(hitsound),
            object_type,
        });
    }
}
