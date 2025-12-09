#![allow(
    clippy::doc_markdown,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::needless_range_loop,
    clippy::match_same_arms,
    clippy::redundant_closure_for_method_calls,
    clippy::collapsible_if
)]
//! Encoder for converting `RoxChart` to StepMania (`.sm`) format.

use std::fmt::Write;

use crate::codec::Encoder;
use crate::error::RoxResult;
use crate::model::{NoteType, RoxChart};

/// Encoder for StepMania (`.sm`) beatmaps.
pub struct SmEncoder;

impl Encoder for SmEncoder {
    fn encode(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        let mut output = String::new();

        // Metadata
        let _ = writeln!(output, "#TITLE:{};", chart.metadata.title);
        let _ = writeln!(output, "#SUBTITLE:;");
        let _ = writeln!(output, "#ARTIST:{};", chart.metadata.artist);
        let _ = writeln!(output, "#TITLETRANSLIT:;");
        let _ = writeln!(output, "#ARTISTTRANSLIT:;");
        let _ = writeln!(output, "#GENRE:;");
        let _ = writeln!(output, "#CREDIT:{};", chart.metadata.creator);
        let _ = writeln!(output, "#BANNER:;");
        if let Some(bg) = &chart.metadata.background_file {
            let _ = writeln!(output, "#BACKGROUND:{bg};");
        } else {
            let _ = writeln!(output, "#BACKGROUND:;");
        }
        let _ = writeln!(output, "#LYRICSPATH:;");
        let _ = writeln!(output, "#CDTITLE:;");
        let _ = writeln!(output, "#MUSIC:{};", chart.metadata.audio_file);

        // Offset (SM uses opposite sign convention, in seconds)
        #[allow(clippy::cast_precision_loss)]
        let offset_seconds = -chart.metadata.audio_offset_us as f64 / 1_000_000.0;
        let _ = writeln!(output, "#OFFSET:{offset_seconds:.6};");

        // Sample start/length
        #[allow(clippy::cast_precision_loss)]
        let sample_start = chart.metadata.preview_time_us as f64 / 1_000_000.0;
        #[allow(clippy::cast_precision_loss)]
        let sample_length = chart.metadata.preview_duration_us as f64 / 1_000_000.0;
        let _ = writeln!(output, "#SAMPLESTART:{sample_start:.3};");
        let _ = writeln!(output, "#SAMPLELENGTH:{sample_length:.3};");

        let _ = writeln!(output, "#SELECTABLE:YES;");

        // BPMs
        output.push_str("#BPMS:");
        let bpm_points: Vec<_> = chart
            .timing_points
            .iter()
            .filter(|tp| !tp.is_inherited)
            .collect();

        for (i, tp) in bpm_points.iter().enumerate() {
            let beat = us_to_beat(tp.time_us, &bpm_points);
            if i > 0 {
                output.push(',');
            }
            let _ = write!(output, "{beat:.3}={:.3}", tp.bpm);
        }
        let _ = writeln!(output, ";");

        // Stops (empty for now)
        let _ = writeln!(output, "#STOPS:;");
        let _ = writeln!(output);

        // Notes section
        let stepstype = match chart.key_count {
            4 => "dance-single",
            6 => "dance-solo",
            8 => "dance-double",
            _ => "dance-single",
        };

        let _ = writeln!(output, "#NOTES:");
        let _ = writeln!(output, "     {stepstype}:");
        let _ = writeln!(output, "     :");
        let _ = writeln!(output, "     {}:", chart.metadata.difficulty_name);
        let _ = writeln!(
            output,
            "     {}:",
            chart.metadata.difficulty_value.unwrap_or(1.0) as u32
        );
        let _ = writeln!(output, "     0,0,0,0,0:");

        // Generate measures
        let bpms: Vec<_> = chart
            .timing_points
            .iter()
            .filter(|tp| !tp.is_inherited)
            .map(|tp| (tp.time_us, tp.bpm))
            .collect();

        encode_measures(&mut output, chart, &bpms);

        let _ = writeln!(output, ";");

        Ok(output.into_bytes())
    }
}

/// Convert microseconds to beat position.
fn us_to_beat(time_us: i64, bpm_points: &[&crate::model::TimingPoint]) -> f64 {
    if bpm_points.is_empty() || time_us == 0 {
        return 0.0;
    }

    let mut current_time_us: i64 = 0;
    let mut current_beat: f64 = 0.0;
    let mut current_bpm = bpm_points[0].bpm;

    for i in 1..bpm_points.len() {
        let tp = bpm_points[i];
        if tp.time_us > time_us {
            break;
        }

        // Calculate beats from current position to this BPM change
        let elapsed_us = tp.time_us - current_time_us;
        let elapsed_beats = us_to_beats_at_bpm(elapsed_us, current_bpm);
        current_beat += elapsed_beats;
        current_time_us = tp.time_us;
        current_bpm = tp.bpm;
    }

    // Add remaining time at current BPM
    let remaining_us = time_us - current_time_us;
    current_beat + us_to_beats_at_bpm(remaining_us, current_bpm)
}

fn us_to_beats_at_bpm(us: i64, bpm: f32) -> f64 {
    let seconds = us as f64 / 1_000_000.0;
    seconds * f64::from(bpm) / 60.0
}

/// Encode all notes into SM measure format.
///
#[allow(clippy::cast_possible_truncation)]
fn encode_measures(output: &mut String, chart: &RoxChart, bpms: &[(i64, f32)]) {
    if chart.notes.is_empty() {
        // Empty chart - just one empty measure
        for _ in 0..4 {
            let _ = writeln!(output, "{}", "0".repeat(chart.key_count as usize));
        }
        return;
    }

    // Find the total duration
    let max_time = chart
        .notes
        .iter()
        .map(|n| n.end_time_us())
        .max()
        .unwrap_or(0);

    // Calculate number of measures needed
    let total_beats = us_to_beat_simple(max_time, bpms);
    let total_measures = (total_beats / 4.0).ceil() as usize + 1;

    // Create note events: (time_us, column, char)
    let mut events: Vec<(i64, u8, char)> = Vec::new();

    for note in &chart.notes {
        match &note.note_type {
            NoteType::Tap => {
                events.push((note.time_us, note.column, '1'));
            }
            NoteType::Hold { duration_us } => {
                events.push((note.time_us, note.column, '2'));
                events.push((note.time_us + duration_us, note.column, '3'));
            }
            NoteType::Burst { duration_us } => {
                events.push((note.time_us, note.column, '4'));
                events.push((note.time_us + duration_us, note.column, '3'));
            }
            NoteType::Mine => {
                events.push((note.time_us, note.column, 'M'));
            }
        }
    }

    // Sort events by time
    events.sort_by_key(|(t, _, _)| *t);

    // Generate each measure
    for measure_num in 0..total_measures.max(1) {
        if measure_num > 0 {
            let _ = writeln!(output, ",");
        }

        // Use 16th note quantization (16 lines per measure)
        let lines_per_measure = 16;
        let measure_start_beat = measure_num as f64 * 4.0;

        for line_idx in 0..lines_per_measure {
            let line_beat = measure_start_beat + (line_idx as f64 * 4.0 / lines_per_measure as f64);
            let line_time_us = beat_to_us_simple(line_beat, bpms);
            let next_line_time_us = beat_to_us_simple(
                measure_start_beat + ((line_idx + 1) as f64 * 4.0 / lines_per_measure as f64),
                bpms,
            );

            // Find events in this time window
            let mut line_chars: Vec<char> = vec!['0'; chart.key_count as usize];

            for (event_time, col, ch) in &events {
                if *event_time >= line_time_us && *event_time < next_line_time_us {
                    if (*col as usize) < line_chars.len() {
                        line_chars[*col as usize] = *ch;
                    }
                }
            }

            let line_str: String = line_chars.into_iter().collect();
            let _ = writeln!(output, "{line_str}");
        }
    }
}

fn us_to_beat_simple(time_us: i64, bpms: &[(i64, f32)]) -> f64 {
    if bpms.is_empty() {
        return time_us as f64 / 1_000_000.0 * 120.0 / 60.0;
    }

    let mut current_time_us: i64 = 0;
    let mut current_beat: f64 = 0.0;
    let mut current_bpm = bpms[0].1;

    for i in 1..bpms.len() {
        let (bpm_time, new_bpm) = bpms[i];
        if bpm_time > time_us {
            break;
        }

        let elapsed = bpm_time - current_time_us;
        current_beat += us_to_beats_at_bpm(elapsed, current_bpm);
        current_time_us = bpm_time;
        current_bpm = new_bpm;
    }

    current_beat + us_to_beats_at_bpm(time_us - current_time_us, current_bpm)
}

fn beat_to_us_simple(beat: f64, bpms: &[(i64, f32)]) -> i64 {
    if bpms.is_empty() {
        return (beat * 60.0 / 120.0 * 1_000_000.0) as i64;
    }

    let mut current_time_us: i64 = 0;
    let mut current_beat: f64 = 0.0;
    let mut current_bpm = bpms[0].1;

    for i in 1..bpms.len() {
        let (bpm_time, new_bpm) = bpms[i];

        let elapsed = bpm_time - current_time_us;
        let beats_at_current = us_to_beats_at_bpm(elapsed, current_bpm);

        if current_beat + beats_at_current >= beat {
            break;
        }

        current_beat += beats_at_current;
        current_time_us = bpm_time;
        current_bpm = new_bpm;
    }

    let remaining_beats = beat - current_beat;
    let remaining_us = (remaining_beats * 60.0 / f64::from(current_bpm) * 1_000_000.0) as i64;
    current_time_us + remaining_us
}
