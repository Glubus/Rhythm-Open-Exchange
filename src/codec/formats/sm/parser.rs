#![allow(
    clippy::doc_markdown,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::needless_range_loop
)]
//! Parser for StepMania (.sm) file format.

use crate::error::{RoxError, RoxResult};

use super::types::{timing, SmChart, SmFile, SmMetadata, SmNote, SmNoteType};

// Safety limit: 100MB for .sm files to prevent memory exhaustion
const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Parse an SM file from raw bytes.
///
/// # Why this design?
/// The parser processes tags (`#TAG:VALUE;`) sequentially.
/// We use `find` and string slicing instead of regex for performance.
/// Floating point parsing is done with explicit error handling to avoid silent data corruption.
///
/// # Errors
///
/// Returns an error if:
/// - The data is not valid UTF-8
/// - The file is larger than 100MB (Safety)
pub fn parse(data: &[u8]) -> RoxResult<SmFile> {
    if data.len() > MAX_FILE_SIZE {
        return Err(RoxError::InvalidFormat(format!(
            "File too large: {} bytes (max {}MB)",
            data.len(),
            MAX_FILE_SIZE / 1024 / 1024
        )));
    }

    let content = std::str::from_utf8(data)
        .map_err(|e| RoxError::InvalidFormat(format!("Invalid UTF-8: {e}")))?;

    let mut sm = SmFile::default();

    // Parse metadata
    parse_metadata(content, &mut sm.metadata);

    // Parse offset (convert to microseconds, make it positive offset)
    if let Some(offset) = parse_float_field(content, "#OFFSET:") {
        // SM offset is in seconds, negative means music starts before notes
        // We store as microseconds
        #[allow(clippy::cast_possible_truncation)]
        {
            sm.offset_us = (offset * 1_000_000.0) as i64;
        }
    }

    // Parse BPMs
    sm.bpms = parse_bpms(content);

    // Parse stops
    sm.stops = parse_stops(content, &sm.bpms);

    // Parse charts
    parse_charts(content, &mut sm.charts, &sm.bpms, &sm.stops);

    Ok(sm)
}

/// Parse metadata fields from content.
fn parse_metadata(content: &str, metadata: &mut SmMetadata) {
    if let Some(v) = parse_string_field(content, "#TITLE:") {
        metadata.title = v;
    }
    if let Some(v) = parse_string_field(content, "#SUBTITLE:") {
        metadata.subtitle = v;
    }
    if let Some(v) = parse_string_field(content, "#ARTIST:") {
        metadata.artist = v;
    }
    if let Some(v) = parse_string_field(content, "#TITLETRANSLIT:") {
        metadata.title_translit = v;
    }
    if let Some(v) = parse_string_field(content, "#ARTISTTRANSLIT:") {
        metadata.artist_translit = v;
    }
    if let Some(v) = parse_string_field(content, "#CREDIT:") {
        metadata.credit = v;
    }
    if let Some(v) = parse_string_field(content, "#MUSIC:") {
        metadata.music = v;
    }
    if let Some(v) = parse_string_field(content, "#BANNER:") {
        metadata.banner = v;
    }
    if let Some(v) = parse_string_field(content, "#BACKGROUND:") {
        metadata.background = v;
    }
    if let Some(v) = parse_float_field(content, "#SAMPLESTART:") {
        metadata.sample_start = v;
    }
    if let Some(v) = parse_float_field(content, "#SAMPLELENGTH:") {
        metadata.sample_length = v;
    }
}

/// Parse a string field like `#TITLE:value;`
fn parse_string_field(content: &str, tag: &str) -> Option<String> {
    let start = content.find(tag)?;
    let after_tag = &content[start + tag.len()..];
    let end = after_tag.find(';')?;
    Some(after_tag[..end].trim().to_string())
}

/// Parse a float field like `#OFFSET:-0.123;`
fn parse_float_field(content: &str, tag: &str) -> Option<f64> {
    let value_str = parse_string_field(content, tag)?;
    match value_str.parse() {
        Ok(v) => Some(v),
        Err(_) => {
            tracing::warn!("Failed to parse float for {}: '{}'", tag, value_str);
            None
        }
    }
}

/// Parse BPM changes from `#BPMS:beat=bpm,beat=bpm,...;`
/// Returns Vec of (time_us, bpm).
fn parse_bpms(content: &str) -> Vec<(i64, f32)> {
    let pairs = parse_pairs(content, "#BPMS:");

    // Convert beat positions to microseconds
    // This requires cumulative timing calculation
    let mut result = Vec::new();
    let mut current_time_us: i64 = 0;
    let mut current_beat: f64 = 0.0;
    let mut current_bpm: f32 = 120.0;

    for (beat, bpm) in pairs {
        if beat > current_beat {
            // Calculate time elapsed from current_beat to this beat
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

    // Ensure we have at least one BPM at time 0
    if result.is_empty() || result[0].0 > 0 {
        result.insert(0, (0, 120.0));
    }

    result
}

/// Parse STOPS from `#STOPS:beat=duration,beat=duration,...;`
/// Returns Vec of (time_us, duration_us).
fn parse_stops(content: &str, bpms: &[(i64, f32)]) -> Vec<(i64, i64)> {
    let pairs = parse_pairs(content, "#STOPS:");

    pairs
        .into_iter()
        .map(|(beat, duration_seconds)| {
            let time_us = beat_to_us(beat, bpms);
            #[allow(clippy::cast_possible_truncation)]
            let duration_us = (duration_seconds * 1_000_000.0) as i64;
            (time_us, duration_us)
        })
        .collect()
    // No explicit sort needed as parse_pairs sorts by beat
}

/// Parse comma-separated pairs like `beat=value,beat=value`.
fn parse_pairs(content: &str, tag: &str) -> Vec<(f64, f64)> {
    let Some(value_str) = parse_string_field(content, tag) else {
        return Vec::new();
    };

    let mut result = Vec::new();
    for pair in value_str.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let parts: Vec<&str> = pair.split('=').collect();
        if parts.len() == 2 {
            match (
                parts[0].trim().parse::<f64>(),
                parts[1].trim().parse::<f64>(),
            ) {
                (Ok(beat), Ok(value)) => result.push((beat, value)),
                _ => tracing::warn!("Malformed pair in {}: '{}'", tag, pair),
            }
        }
    }

    // Sort by beat
    result.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    result
}

/// Convert beat position to microseconds using BPM list.
fn beat_to_us(target_beat: f64, bpms: &[(i64, f32)]) -> i64 {
    if bpms.is_empty() {
        // Default 120 BPM
        let rows = target_beat * timing::ROWS_PER_BEAT;
        return timing::rows_to_us(rows, 120.0);
    }

    let mut current_time_us: i64 = 0;
    let mut current_beat: f64 = 0.0;
    let mut current_bpm = bpms[0].1;
    let mut bpm_idx = 0;

    // Find the BPM at beat 0
    while bpm_idx < bpms.len() && bpms[bpm_idx].0 == 0 {
        current_bpm = bpms[bpm_idx].1;
        bpm_idx += 1;
    }

    // Convert BPM times to beats for comparison
    // This is a simplified approach - iterate through BPMs
    for i in 1..bpms.len() {
        let (bpm_time_us, new_bpm) = bpms[i];

        // Calculate what beat this BPM change is at
        let rows_elapsed = timing::us_to_rows(bpm_time_us - current_time_us, current_bpm);
        let bpm_beat = current_beat + rows_elapsed / timing::ROWS_PER_BEAT;

        if bpm_beat >= target_beat {
            // Target is before this BPM change
            break;
        }

        // Move to this BPM change point
        current_time_us = bpm_time_us;
        current_beat = bpm_beat;
        current_bpm = new_bpm;
    }

    // Calculate remaining time to target beat
    if target_beat > current_beat {
        let remaining_beats = target_beat - current_beat;
        let remaining_rows = remaining_beats * timing::ROWS_PER_BEAT;
        current_time_us += timing::rows_to_us(remaining_rows, current_bpm);
    }

    current_time_us
}

/// Parse all chart sections.
fn parse_charts(
    content: &str,
    charts: &mut Vec<SmChart>,
    bpms: &[(i64, f32)],
    stops: &[(i64, i64)],
) {
    // Split by #NOTES: to find each chart
    let sections: Vec<&str> = content.split("#NOTES:").skip(1).collect();

    for section in sections {
        // Find end of this chart (next tag or EOF)
        let end = section.find('#').unwrap_or(section.len());
        let chart_content = &section[..end];

        if let Some(chart) = parse_chart(chart_content, bpms, stops) {
            charts.push(chart);
        }
    }
}

fn parse_chart(content: &str, bpms: &[(i64, f32)], _stops: &[(i64, i64)]) -> Option<SmChart> {
    let lines: Vec<&str> = content.lines().map(str::trim).collect();
    let mut chart = SmChart::default();

    // Parse header (5 fields separated by colons on separate lines)
    // stepstype:
    // description:
    // difficulty:
    // meter:
    // radarvalues:
    let mut idx = 0;

    // Skip empty lines
    while idx < lines.len() && lines[idx].is_empty() {
        idx += 1;
    }

    // Parse header fields
    let mut header_fields = Vec::new();
    while idx < lines.len() && header_fields.len() < 5 {
        let line = lines[idx];
        if line.is_empty() {
            idx += 1;
            continue;
        }

        // Remove trailing colon and store
        let field = line.trim_end_matches(':').to_string();
        header_fields.push(field);
        idx += 1;
    }

    if header_fields.len() < 5 {
        tracing::warn!("Invalid chart header: missing fields");
        return None;
    }

    chart.stepstype.clone_from(&header_fields[0]);
    chart.description.clone_from(&header_fields[1]);
    chart.difficulty.clone_from(&header_fields[2]);
    chart.meter = match header_fields[3].parse() {
        Ok(v) => v,
        Err(_) => {
            tracing::warn!(
                "Failed to parse meter: '{}', defaulting to 1",
                header_fields[3]
            );
            1
        }
    };

    // Parse radar values
    for val in header_fields[4].split(',') {
        if let Ok(v) = val.trim().parse() {
            chart.radar_values.push(v);
        }
    }

    // Determine column count
    chart.column_count = SmChart::column_count_from_stepstype(&chart.stepstype);

    // Parse measures
    let mut measure_num = 0;
    let mut current_row: f64 = 0.0;

    // Collect all note lines per measure
    let mut current_measure_lines: Vec<&str> = Vec::new();

    while idx < lines.len() {
        let line = lines[idx];

        // Skip comments
        let line = if let Some(pos) = line.find("//") {
            &line[..pos]
        } else {
            line
        }
        .trim();

        if line.is_empty() {
            idx += 1;
            continue;
        }

        // End of notes section
        if line == ";" {
            // Process final measure
            if !current_measure_lines.is_empty() {
                parse_measure_notes(
                    &current_measure_lines,
                    measure_num,
                    &mut current_row,
                    bpms,
                    chart.column_count,
                    &mut chart.notes,
                );
            }
            break;
        }

        // Measure separator
        if line == "," {
            // Process current measure
            parse_measure_notes(
                &current_measure_lines,
                measure_num,
                &mut current_row,
                bpms,
                chart.column_count,
                &mut chart.notes,
            );
            current_measure_lines.clear();
            measure_num += 1;
            current_row = (measure_num as f64) * timing::ROWS_PER_MEASURE;
            idx += 1;
            continue;
        }

        // Note line
        if is_note_line(line) {
            // Update column count if we see more columns
            #[allow(clippy::cast_possible_truncation)]
            {
                if line.len() as u8 > chart.column_count {
                    chart.column_count = line.len() as u8;
                }
            }
            current_measure_lines.push(line);
        }

        idx += 1;
    }

    Some(chart)
}

/// Check if a line contains only valid note characters.
fn is_note_line(line: &str) -> bool {
    !line.is_empty()
        && line.chars().all(|c| {
            matches!(
                c,
                '0' | '1' | '2' | '3' | '4' | 'M' | 'm' | 'L' | 'l' | 'F' | 'f'
            )
        })
}

/// Parse notes from measure lines.
fn parse_measure_notes(
    lines: &[&str],
    measure_num: usize,
    current_row: &mut f64,
    bpms: &[(i64, f32)],
    _column_count: u8,
    notes: &mut Vec<SmNote>,
) {
    if lines.is_empty() {
        *current_row += timing::ROWS_PER_MEASURE;
        return;
    }

    let num_lines = lines.len();
    let rows_per_line = timing::ROWS_PER_MEASURE / (num_lines as f64);

    for (line_idx, line) in lines.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let row =
            (measure_num as f64) * timing::ROWS_PER_MEASURE + (line_idx as f64) * rows_per_line;
        let time_us = row_to_us(row, bpms);

        for (col, ch) in line.chars().enumerate() {
            let note_type = SmNoteType::from_char(ch);

            if note_type.is_note() {
                #[allow(clippy::cast_possible_truncation)]
                notes.push(SmNote {
                    time_us,
                    column: col as u8,
                    note_type,
                });
            }
        }
    }

    *current_row += timing::ROWS_PER_MEASURE;
}

/// Convert row position to microseconds using BPM list.
fn row_to_us(row: f64, bpms: &[(i64, f32)]) -> i64 {
    if bpms.is_empty() {
        return timing::rows_to_us(row, 120.0);
    }

    let mut current_time_us: i64 = 0;
    let mut current_row: f64 = 0.0;
    let mut current_bpm = bpms[0].1;

    for i in 1..bpms.len() {
        let (bpm_time_us, new_bpm) = bpms[i];

        // Calculate row at this BPM change
        let bpm_row = current_row + timing::us_to_rows(bpm_time_us - current_time_us, current_bpm);

        if bpm_row >= row {
            // Target row is before this BPM change
            break;
        }

        current_time_us = bpm_time_us;
        current_row = bpm_row;
        current_bpm = new_bpm;
    }

    // Calculate time for remaining rows
    current_time_us + timing::rows_to_us(row - current_row, current_bpm)
}
