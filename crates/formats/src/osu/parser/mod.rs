#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{format, string::ToString};

mod objects;
mod sections;
mod timing;

use super::types::OsuBeatmap;
use rox::error::{RoxError, RoxResult};

pub use objects::parse_hit_object;
pub use sections::{parse_difficulty, parse_event, parse_general, parse_metadata};
pub use timing::parse_timing_point;

const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section { None, General, Editor, Metadata, Difficulty, Events, TimingPoints, HitObjects }

/// Parse a `.osu` file into an [`OsuBeatmap`].
///
/// # Errors
/// Returns an error if data exceeds 100 MB or is not valid UTF-8.
pub fn parse(data: &[u8]) -> RoxResult<OsuBeatmap> {
    if data.len() > MAX_FILE_SIZE {
        return Err(RoxError::InvalidFormat(
            format!("File too large: {} bytes (max 100MB)", data.len())
        ));
    }
    if core::str::from_utf8(data).is_err() {
        return Err(RoxError::InvalidFormat("Invalid UTF-8".to_string()));
    }
    let mut beatmap = OsuBeatmap::default();
    beatmap.hit_objects.reserve(data.len() / 40);
    let mut section = Section::None;
    let mut start = 0;
    let mut line_idx = 0;
    for end in memchr::memchr_iter(b'\n', data) {
        let mut line_bytes = &data[start..end];
        if !line_bytes.is_empty() && line_bytes[line_bytes.len() - 1] == b'\r' {
            line_bytes = &line_bytes[..line_bytes.len() - 1];
        }
        process_line(line_bytes, line_idx, &mut section, &mut beatmap);
        start = end + 1;
        line_idx += 1;
    }
    if start < data.len() {
        process_line(&data[start..], line_idx, &mut section, &mut beatmap);
    }
    Ok(beatmap)
}

fn process_line(line: &[u8], idx: usize, section: &mut Section, beatmap: &mut OsuBeatmap) {
    if is_skippable(line) { return; }
    if let Some(s) = try_parse_section(line) { *section = s; return; }
    if line.starts_with(b"osu file format v") {
        let s = unsafe { core::str::from_utf8_unchecked(line) };
        beatmap.format_version = s.strip_prefix("osu file format v")
            .and_then(|v| v.parse().ok()).unwrap_or(14);
        return;
    }
    dispatch_section(*section, line, idx, beatmap);
}

fn is_skippable(line: &[u8]) -> bool {
    line.is_empty() || (line.len() >= 2 && line[0] == b'/' && line[1] == b'/')
}

fn try_parse_section(line: &[u8]) -> Option<Section> {
    if line.first() == Some(&b'[') && line.last() == Some(&b']') {
        let name = unsafe { core::str::from_utf8_unchecked(&line[1..line.len() - 1]) };
        Some(match name {
            "General" => Section::General, "Editor" => Section::Editor,
            "Metadata" => Section::Metadata, "Difficulty" => Section::Difficulty,
            "Events" => Section::Events, "TimingPoints" => Section::TimingPoints,
            "HitObjects" => Section::HitObjects, _ => Section::None,
        })
    } else { None }
}

fn dispatch_section(section: Section, line: &[u8], idx: usize, beatmap: &mut OsuBeatmap) {
    if section == Section::HitObjects {
        if let Some(ho) = objects::parse_hit_object_bytes(line) {
            beatmap.hit_objects.push(ho);
        } else {
            tracing::warn!("line {}: failed to parse hit object", idx + 1);
        }
        return;
    }
    let s = unsafe { core::str::from_utf8_unchecked(line) }.trim();
    match section {
        Section::General => sections::parse_general(s, &mut beatmap.general),
        Section::Metadata => sections::parse_metadata(s, &mut beatmap.metadata),
        Section::Difficulty => sections::parse_difficulty(s, &mut beatmap.difficulty),
        Section::Events => sections::parse_event(s, &mut beatmap.background),
        Section::TimingPoints => {
            if let Some(tp) = timing::parse_timing_point(s) {
                beatmap.timing_points.push(tp);
            } else {
                tracing::warn!("line {}: failed to parse timing point", idx + 1);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mania_7k_asset() {
        let data = rox_test_utils::get_test_asset("osu/mania_7k.osu");
        let beatmap = parse(&data).expect("parse failed");
        assert_eq!(beatmap.general.mode, 3);
        assert_eq!(beatmap.difficulty.circle_size, 7.0);
        assert!(!beatmap.timing_points.is_empty());
        assert!(!beatmap.hit_objects.is_empty());
    }

    #[test]
    fn test_parse_hold_note() {
        let data = b"osu file format v14\n\n[General]\nMode: 3\n\n[Difficulty]\nCircleSize:4\n\n[HitObjects]\n64,192,1000,128,0,2000:0:0:0:0:\n";
        let beatmap = parse(data).expect("parse failed");
        assert_eq!(beatmap.hit_objects.len(), 1);
        let ho = &beatmap.hit_objects[0];
        assert!(ho.is_hold());
        assert_eq!(ho.end_time, Some(2000));
    }

    #[test]
    fn test_parse_bpm_timing_point() {
        let data = b"osu file format v14\n\n[General]\nMode: 3\n\n[TimingPoints]\n0,322.58,4,1,0,100,1,0\n";
        let beatmap = parse(data).expect("parse failed");
        assert_eq!(beatmap.timing_points.len(), 1);
        assert!(beatmap.timing_points[0].uninherited);
        assert!((beatmap.timing_points[0].bpm().unwrap() - 186.0).abs() < 1.0);
    }

    #[test]
    fn test_file_too_large() {
        let big = vec![0u8; 100 * 1024 * 1024 + 1];
        assert!(parse(&big).is_err());
    }
}
