#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec, format};

use super::types::{SmChart, SmFile, SmMetadata, SmNote, SmNoteType, timing};
use rox::error::{RoxError, RoxResult};

const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Parse an SM file from raw bytes.
///
/// # Errors
/// Returns an error if data exceeds 100MB or is invalid UTF-8.
pub fn parse(data: &[u8]) -> RoxResult<SmFile> {
    if data.len() > MAX_FILE_SIZE {
        return Err(RoxError::InvalidFormat(
            format!("File too large: {} bytes (max 100MB)", data.len())
        ));
    }
    let content = core::str::from_utf8(data)
        .map_err(|e| RoxError::InvalidFormat(format!("Invalid UTF-8: {e}")))?;
    let mut sm = SmFile::default();
    parse_metadata(content, &mut sm.metadata);
    if let Some(offset) = parse_float_field(content, "#OFFSET:") {
        #[allow(clippy::cast_possible_truncation)]
        { sm.offset_us = (offset * 1_000_000.0) as i64; }
    }
    sm.bpms = parse_bpms(content);
    parse_charts(content, &mut sm.charts, &sm.bpms);
    Ok(sm)
}

fn parse_metadata(content: &str, metadata: &mut SmMetadata) {
    if let Some(v) = parse_string_field(content, "#TITLE:") { metadata.title = v; }
    if let Some(v) = parse_string_field(content, "#ARTIST:") { metadata.artist = v; }
    if let Some(v) = parse_string_field(content, "#CREDIT:") { metadata.credit = v; }
    if let Some(v) = parse_string_field(content, "#MUSIC:") { metadata.music = v; }
    if let Some(v) = parse_string_field(content, "#BANNER:") { metadata.banner = v; }
    if let Some(v) = parse_string_field(content, "#BACKGROUND:") { metadata.background = v; }
    if let Some(v) = parse_float_field(content, "#SAMPLESTART:") { metadata.sample_start = v; }
    if let Some(v) = parse_float_field(content, "#SAMPLELENGTH:") { metadata.sample_length = v; }
}

fn parse_string_field(content: &str, tag: &str) -> Option<String> {
    let start = content.find(tag)?;
    let after_tag = &content[start + tag.len()..];
    let end = after_tag.find(';')?;
    Some(after_tag[..end].trim().to_string())
}

fn parse_float_field(content: &str, tag: &str) -> Option<f64> {
    let value_str = parse_string_field(content, tag)?;
    value_str.parse().ok()
}

fn parse_bpms(content: &str) -> Vec<(i64, f32)> {
    let pairs = parse_pairs(content, "#BPMS:");
    let mut result = Vec::new();
    let mut current_time_us: i64 = 0;
    let mut current_beat: f64 = 0.0;
    let mut current_bpm: f32 = 120.0;
    for (beat, bpm) in pairs {
        if beat > current_beat {
            let beats_elapsed = beat - current_beat;
            let rows_elapsed = beats_elapsed * timing::ROWS_PER_BEAT;
            current_time_us += timing::rows_to_us(rows_elapsed, current_bpm);
            current_beat = beat;
        }
        #[allow(clippy::cast_possible_truncation)]
        let bpm_f32 = bpm as f32;
        result.push((current_time_us, bpm_f32));
        current_bpm = bpm_f32;
    }
    if result.is_empty() || result[0].0 > 0 {
        result.insert(0, (0, 120.0));
    }
    result
}

fn parse_pairs(content: &str, tag: &str) -> Vec<(f64, f64)> {
    let Some(value_str) = parse_string_field(content, tag) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for pair in value_str.split(',') {
        let pair = pair.trim();
        if pair.is_empty() { continue; }
        let parts: Vec<&str> = pair.split('=').collect();
        if parts.len() == 2
            && let (Ok(beat), Ok(value)) = (
                parts[0].trim().parse::<f64>(),
                parts[1].trim().parse::<f64>(),
            )
        {
            result.push((beat, value));
        }
    }
    result.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(core::cmp::Ordering::Equal));
    result
}

#[allow(dead_code)]
fn beat_to_us(target_beat: f64, bpms: &[(i64, f32)]) -> i64 {
    if bpms.is_empty() {
        return timing::rows_to_us(target_beat * timing::ROWS_PER_BEAT, 120.0);
    }
    let mut current_time_us: i64 = 0;
    let mut current_beat: f64 = 0.0;
    let mut current_bpm = bpms[0].1;
    let mut bpm_idx = 0;
    while bpm_idx < bpms.len() && bpms[bpm_idx].0 == 0 {
        current_bpm = bpms[bpm_idx].1;
        bpm_idx += 1;
    }
    for &(bpm_time_us, new_bpm) in &bpms[1..] {
        let rows_elapsed = timing::us_to_rows(bpm_time_us - current_time_us, current_bpm);
        let bpm_beat = current_beat + rows_elapsed / timing::ROWS_PER_BEAT;
        if bpm_beat >= target_beat { break; }
        current_time_us = bpm_time_us;
        current_beat = bpm_beat;
        current_bpm = new_bpm;
    }
    if target_beat > current_beat {
        let remaining_rows = (target_beat - current_beat) * timing::ROWS_PER_BEAT;
        current_time_us += timing::rows_to_us(remaining_rows, current_bpm);
    }
    current_time_us
}

fn parse_charts(content: &str, charts: &mut Vec<SmChart>, bpms: &[(i64, f32)]) {
    let sections: Vec<&str> = content.split("#NOTES:").skip(1).collect();
    for section in sections {
        let end = section.find('#').unwrap_or(section.len());
        if let Some(chart) = parse_chart(&section[..end], bpms) {
            charts.push(chart);
        }
    }
}

fn parse_chart(content: &str, bpms: &[(i64, f32)]) -> Option<SmChart> {
    let lines: Vec<&str> = content.lines().map(str::trim).collect();
    let mut chart = SmChart::default();
    let mut idx = 0;
    while idx < lines.len() && lines[idx].is_empty() { idx += 1; }
    // Parse 5 header fields
    let mut header_fields: Vec<String> = Vec::new();
    while idx < lines.len() && header_fields.len() < 5 {
        let line = lines[idx];
        if line.is_empty() { idx += 1; continue; }
        header_fields.push(line.trim_end_matches(':').to_string());
        idx += 1;
    }
    if header_fields.len() < 5 { return None; }
    chart.stepstype.clone_from(&header_fields[0]);
    // header_fields[1] is description (ignored in new types)
    chart.difficulty.clone_from(&header_fields[2]);
    chart.meter = header_fields[3].parse().unwrap_or(1);
    // header_fields[4] is radar values (ignored)
    chart.column_count = SmChart::column_count_from_stepstype(&chart.stepstype);
    // Parse measures
    parse_measures(&lines, idx, bpms, &mut chart);
    Some(chart)
}

fn parse_measures(lines: &[&str], start_idx: usize, bpms: &[(i64, f32)], chart: &mut SmChart) {
    let mut idx = start_idx;
    let mut measure_num = 0usize;
    let mut current_measure_lines: Vec<&str> = Vec::new();
    while idx < lines.len() {
        let raw = lines[idx];
        let line = raw.find("//").map_or(raw, |p| &raw[..p]).trim();
        if line.is_empty() { idx += 1; continue; }
        if line == ";" {
            if !current_measure_lines.is_empty() {
                parse_measure_notes(&current_measure_lines, measure_num, bpms, chart.column_count, &mut chart.notes);
            }
            break;
        }
        if line == "," {
            parse_measure_notes(&current_measure_lines, measure_num, bpms, chart.column_count, &mut chart.notes);
            current_measure_lines.clear();
            measure_num += 1;
            idx += 1;
            continue;
        }
        if is_note_line(line) {
            #[allow(clippy::cast_possible_truncation)]
            if line.len() as u8 > chart.column_count { chart.column_count = line.len() as u8; }
            current_measure_lines.push(lines[idx]); // push original (not stripped) for column count
        }
        idx += 1;
    }
}

fn is_note_line(line: &str) -> bool {
    !line.is_empty() && line.chars().all(|c| matches!(c, '0'|'1'|'2'|'3'|'4'|'M'|'m'|'L'|'l'|'F'|'f'))
}

fn parse_measure_notes(
    lines: &[&str], measure_num: usize, bpms: &[(i64, f32)],
    _column_count: u8, notes: &mut Vec<SmNote>,
) {
    if lines.is_empty() { return; }
    let num_lines = lines.len();
    #[allow(clippy::cast_precision_loss)]
    let rows_per_line = timing::ROWS_PER_MEASURE / num_lines as f64;
    for (line_idx, line) in lines.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let row = measure_num as f64 * timing::ROWS_PER_MEASURE + line_idx as f64 * rows_per_line;
        let time_us = row_to_us(row, bpms);
        for (col, ch) in line.chars().enumerate() {
            let note_type = SmNoteType::from_char(ch);
            if note_type.is_note() {
                #[allow(clippy::cast_possible_truncation)]
                notes.push(SmNote { time_us, column: col as u8, note_type });
            }
        }
    }
}

fn row_to_us(row: f64, bpms: &[(i64, f32)]) -> i64 {
    if bpms.is_empty() { return timing::rows_to_us(row, 120.0); }
    let mut current_time_us: i64 = 0;
    let mut current_row: f64 = 0.0;
    let mut current_bpm = bpms[0].1;
    for &(bpm_time_us, new_bpm) in &bpms[1..] {
        let bpm_row = current_row + timing::us_to_rows(bpm_time_us - current_time_us, current_bpm);
        if bpm_row >= row { break; }
        current_time_us = bpm_time_us;
        current_row = bpm_row;
        current_bpm = new_bpm;
    }
    current_time_us + timing::rows_to_us(row - current_row, current_bpm)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASIC_SM: &[u8] = b"
#TITLE:Test Song;
#ARTIST:Test Artist;
#CREDIT:Mapper;
#MUSIC:song.ogg;
#OFFSET:0;
#BPMS:0=120;
#STOPS:;

#NOTES:
     dance-single:
     :
     Hard:
     8:
     0,0,0,0,0:
0000
1000
0100
0010
,
0001
0000
1000
0100
;
";

    #[test]
    fn test_parse_basic_sm() {
        let sm = parse(BASIC_SM).expect("parse failed");
        assert_eq!(sm.metadata.title, "Test Song");
        assert_eq!(sm.metadata.artist, "Test Artist");
        assert!(!sm.bpms.is_empty());
        assert!(!sm.charts.is_empty());
        assert_eq!(sm.charts[0].column_count, 4);
        assert!(!sm.charts[0].notes.is_empty());
    }

    #[test]
    fn test_parse_4k_asset() {
        let data = rox_test_utils::get_test_asset("stepmania/4k.sm");
        let sm = parse(&data).expect("parse failed");
        assert!(!sm.charts.is_empty());
        assert_eq!(sm.charts[0].column_count, 4);
    }

    #[test]
    fn test_file_too_large() {
        let big = vec![0u8; 100 * 1024 * 1024 + 1];
        assert!(parse(&big).is_err());
    }
}
