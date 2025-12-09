//! Parser for osu!taiko format.
//!
//! Reuses the osu parser struct parsing logic but extracts Taiko-specific hit objects.

use crate::codec::formats::osu::parser::{
    parse_difficulty, parse_event, parse_general, parse_metadata, parse_timing_point,
};
use crate::error::{RoxError, RoxResult};

use super::types::{TaikoBeatmap, TaikoHitObject, TaikoHitsound};

/// Parse a Taiko beatmap from raw bytes.
///
/// # Errors
///
/// Returns an error if the data is not valid UTF-8 or has invalid format.
pub fn parse(data: &[u8]) -> RoxResult<TaikoBeatmap> {
    let content = std::str::from_utf8(data)
        .map_err(|e| RoxError::InvalidFormat(format!("Invalid UTF-8: {e}")))?;

    let mut beatmap = TaikoBeatmap::default();
    let mut section = "";

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

        // Section headers
        if line.starts_with('[') && line.ends_with(']') {
            section = line;
            continue;
        }

        match section {
            "[General]" => parse_general(line, &mut beatmap.general),
            "[Metadata]" => parse_metadata(line, &mut beatmap.metadata),
            "[Difficulty]" => parse_difficulty(line, &mut beatmap.difficulty),
            "[Events]" => parse_event(line, &mut beatmap.background),
            "[TimingPoints]" => {
                if let Some(tp) = parse_timing_point(line) {
                    beatmap.timing_points.push(tp);
                }
            }
            "[HitObjects]" => parse_hit_object_line(line, &mut beatmap),
            _ => {}
        }
    }

    Ok(beatmap)
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
