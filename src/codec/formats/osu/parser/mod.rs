//! Parser for .osu file format.

mod objects;
mod sections;
mod timing;

use super::types::OsuBeatmap;
use crate::error::{RoxError, RoxResult};

pub use objects::parse_hit_object;
pub use sections::{parse_difficulty, parse_event, parse_general, parse_metadata};
pub use timing::parse_timing_point;

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

// Safety limit: 100MB for .osu files
const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Parse a .osu file into an `OsuBeatmap`.
///
/// # Why this design?
/// The parser processes the file line-by-line using a state machine (`Section` enum).
/// This approach was chosen over a full tokenizer/parser generator because:
/// 1. The .osu format is line-oriented and relatively simple.
/// 2. Performance is critical for loading large beatmap packs.
/// 3. It allows for lenient parsing (skipping unknown sections) which is standard in the osu! ecosystem.
///
/// # Errors
///
/// Returns an error if:
/// - The data is not valid UTF-8
/// - The beatmap is not in mania mode (mode != 3)
/// - The file is larger than 100MB
pub fn parse(data: &[u8]) -> RoxResult<OsuBeatmap> {
    if data.len() > MAX_FILE_SIZE {
        return Err(RoxError::InvalidFormat(format!(
            "File too large: {} bytes (max {}MB)",
            data.len(),
            MAX_FILE_SIZE / 1024 / 1024
        )));
    }

    // Validate UTF-8 upfront
    if std::str::from_utf8(data).is_err() {
        return Err(RoxError::InvalidFormat("Invalid UTF-8".to_string()));
    }

    let mut beatmap = OsuBeatmap::default();
    // Estimate capacity based on file size (approx 40 bytes per HitObject line)
    // This reduces reallocations for large files
    beatmap.hit_objects.reserve(data.len() / 40);
    let mut section = Section::None;

    let mut start = 0;
    let mut line_idx = 0;

    // Iterate over newlines using SIMD-accelerated memchr
    for end in memchr::memchr_iter(b'\n', data) {
        let mut line_bytes = &data[start..end];

        // Handle CRLF (trim \r)
        if !line_bytes.is_empty() && line_bytes[line_bytes.len() - 1] == b'\r' {
            line_bytes = &line_bytes[..line_bytes.len() - 1];
        }

        process_line(line_bytes, line_idx, &mut section, &mut beatmap);

        start = end + 1;
        line_idx += 1;
    }

    // Process the last line if there is no trailing newline
    if start < data.len() {
        let line_bytes = &data[start..];
        process_line(line_bytes, line_idx, &mut section, &mut beatmap);
    }

    Ok(beatmap)
}

#[inline(always)]
fn process_line(
    line_bytes: &[u8],
    line_idx: usize,
    section: &mut Section,
    beatmap: &mut OsuBeatmap,
) {
    if is_skippable(line_bytes) {
        return;
    }

    if let Some(new_section) = try_parse_section(line_bytes) {
        *section = new_section;
        return;
    }

    if is_format_version(line_bytes) {
        parse_format_version(line_bytes, beatmap);
        return;
    }

    handle_section_content(section, line_bytes, line_idx, beatmap);
}

#[inline(always)]
fn is_skippable(line_bytes: &[u8]) -> bool {
    line_bytes.is_empty()
        || (line_bytes.len() >= 2 && line_bytes[0] == b'/' && line_bytes[1] == b'/')
}

#[inline(always)]
fn is_format_version(line_bytes: &[u8]) -> bool {
    line_bytes.starts_with(b"osu file format v")
}

#[inline(always)]
fn parse_format_version(line_bytes: &[u8], beatmap: &mut OsuBeatmap) {
    let line = unsafe { std::str::from_utf8_unchecked(line_bytes) };
    beatmap.format_version = line
        .strip_prefix("osu file format v")
        .and_then(|s| s.parse().ok())
        .unwrap_or(14);
}

#[inline(always)]
fn try_parse_section(line_bytes: &[u8]) -> Option<Section> {
    if line_bytes.len() > 2 && line_bytes[0] == b'[' && line_bytes[line_bytes.len() - 1] == b']' {
        let line = unsafe { std::str::from_utf8_unchecked(line_bytes) };
        let section_name = &line[1..line.len() - 1];
        Some(match section_name {
            "General" => Section::General,
            "Editor" => Section::Editor,
            "Metadata" => Section::Metadata,
            "Difficulty" => Section::Difficulty,
            "Events" => Section::Events,
            "TimingPoints" => Section::TimingPoints,
            "HitObjects" => Section::HitObjects,
            _ => Section::None,
        })
    } else {
        None
    }
}

#[inline(always)]
fn handle_section_content(
    section: &mut Section,
    line_bytes: &[u8],
    line_idx: usize,
    beatmap: &mut OsuBeatmap,
) {
    match section {
        Section::HitObjects => {
            if let Some(ho) =
                crate::codec::formats::osu::parser::objects::parse_hit_object_bytes(line_bytes)
            {
                beatmap.hit_objects.push(ho);
            } else {
                let line = unsafe { std::str::from_utf8_unchecked(line_bytes) };
                tracing::warn!(line = line_idx + 1, "Failed to parse hit object: {}", line);
            }
        }
        _ => handle_text_section(section, line_bytes, line_idx, beatmap),
    }
}

#[inline(always)]
fn handle_text_section(
    section: &mut Section,
    line_bytes: &[u8],
    line_idx: usize,
    beatmap: &mut OsuBeatmap,
) {
    let line = unsafe { std::str::from_utf8_unchecked(line_bytes) };
    let line = line.trim();
    match section {
        Section::General => parse_general(line, &mut beatmap.general),
        Section::Metadata => parse_metadata(line, &mut beatmap.metadata),
        Section::Difficulty => parse_difficulty(line, &mut beatmap.difficulty),
        Section::Events => parse_event(line, &mut beatmap.background),
        Section::TimingPoints => {
            if let Some(tp) = parse_timing_point(line) {
                beatmap.timing_points.push(tp);
            } else {
                tracing::warn!(
                    line = line_idx + 1,
                    "Failed to parse timing point: {}",
                    line
                );
            }
        }
        Section::HitObjects => unreachable!(),
        Section::None | Section::Editor => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::formats::osu::OsuHitObject;

    #[test]
    fn test_parse_timing_point_bpm() {
        let data = b"osu file format v14\n\n[General]\nMode: 3\n\n[TimingPoints]\n404,322.58064516129,4,1,1,50,1,0\n";
        let beatmap = parse(data).unwrap();

        assert_eq!(beatmap.timing_points.len(), 1);
        let tp = &beatmap.timing_points[0];
        assert_eq!(tp.time, 404.0);
        assert!(tp.uninherited);
        assert!((tp.bpm().unwrap() - 186.0).abs() < 1.0);
    }

    #[test]
    fn test_parse_timing_point_sv() {
        let data = b"osu file format v14\n\n[General]\nMode: 3\n\n[TimingPoints]\n21855,-133.333333333333,4,1,1,50,0,0\n";
        let beatmap = parse(data).unwrap();

        let tp = &beatmap.timing_points[0];
        assert!(!tp.uninherited);
        assert!((tp.scroll_velocity() - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_parse_timing_point_sv_normal() {
        let data =
            b"osu file format v14\n\n[General]\nMode: 3\n\n[TimingPoints]\n32500,-100,4,1,1,50,0,0\n";
        let beatmap = parse(data).unwrap();

        let tp = &beatmap.timing_points[0];
        assert!((tp.scroll_velocity() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_hit_object_tap() {
        let data = b"osu file format v14\n\n[General]\nMode: 3\n\n[Difficulty]\nCircleSize:7\n\n[HitObjects]\n402,192,1694,5,0,0:0:0:0:\n";
        let beatmap = parse(data).unwrap();

        assert_eq!(beatmap.hit_objects.len(), 1);
        let ho = &beatmap.hit_objects[0];
        assert_eq!(ho.x, 402);
        assert_eq!(ho.time, 1694);
    }

    #[test]
    fn test_column_calculation() {
        let ho = OsuHitObject {
            x: 36,
            y: 192,
            time: 0,
            object_type: 1,
            hit_sound: 0,
            end_time: None,
            extras: compact_str::CompactString::new(""),
        };
        assert_eq!(ho.column(7), 0);

        let ho2 = OsuHitObject {
            x: 475,
            ..ho.clone()
        };
        assert_eq!(ho2.column(7), 6);

        let ho3 = OsuHitObject {
            x: 256,
            ..ho.clone()
        };
        assert_eq!(ho3.column(7), 3); // center
    }

    #[test]
    fn test_parse_full_sample() {
        let data = crate::test_utils::get_test_asset("osu/mania_7k.osu");
        let beatmap = parse(&data).unwrap();

        assert_eq!(beatmap.general.mode, 3); // mania
        assert_eq!(beatmap.difficulty.circle_size, 7.0); // 7K
        assert!(!beatmap.timing_points.is_empty());
        assert!(!beatmap.hit_objects.is_empty());
        assert_eq!(beatmap.metadata.version, "7K Awakened");
    }
}
