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

        // Determine Sync Point (Beat 0 location)
        // SM expects Offset to be the time of the first beat.
        // We use the time of the first uninherited timing point.
        let first_bpm_time = chart
            .timing_points
            .iter()
            .find(|tp| !tp.is_inherited)
            .map(|tp| tp.time_us)
            .unwrap_or(0);

        // Offset (SM uses "Time where Beat 0 begins" in seconds)
        // So if beat 0 is at -0.030s, Offset should be -0.030.
        let offset_seconds = first_bpm_time as f64 / 1_000_000.0;
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
            // Calculate beat relative to the sync point (first_bpm_time)
            // Note: Since we set offset based on first_bpm_time, beat 0 matches that time.
            let beat = us_to_beat(tp.time_us, &bpm_points, first_bpm_time);
            if i > 0 {
                output.push(',');
            }
            // Format beat: if integer, use integer format, else float
            if (beat - beat.round()).abs() < 0.001 {
                let _ = write!(output, "{:.0}={:.3}", beat, tp.bpm);
            } else {
                let _ = write!(output, "{:.3}={:.3}", beat, tp.bpm);
            }
        }
        let _ = writeln!(output, ";");

        // Stops (empty for now)
        let _ = writeln!(output, "#STOPS:;");
        let _ = writeln!(output);

        // Notes section
        let stepstype = match chart.key_count() {
            4 => "dance-single",
            6 => "dance-solo",
            8 => "dance-double",
            _ => "dance-single",
        };

        let _ = writeln!(output, "#NOTES:");
        let _ = writeln!(output, "     {stepstype}:");
        let _ = writeln!(output, "     :");
        // Force Difficulty to "Hard" or "Challenge" to ensure Etterna/SM sees it validly.
        // "1.0x" is not a standard difficulty name.
        let difficulty_name = match chart.metadata.difficulty_name.as_str() {
            "Beginner" | "Easy" | "Medium" | "Hard" | "Challenge" | "Edit" => {
                &chart.metadata.difficulty_name
            }
            _ => "Hard", // Fallback for numeric versions like "1.0x"
        };
        let _ = writeln!(output, "     {}:", difficulty_name);
        let _ = writeln!(
            output,
            "     {}:",
            chart.metadata.difficulty_value.unwrap_or(1.0) as u32
        );
        // Correct format for radar values
        // Revert to simple integer format as per working 4k.sm example
        let _ = writeln!(output, "     0,0,0,0,0:");

        // Generate measures
        let bpms_tuple: Vec<_> = chart
            .timing_points
            .iter()
            .filter(|tp| !tp.is_inherited)
            .map(|tp| (tp.time_us, tp.bpm))
            .collect();

        encode_measures(&mut output, chart, &bpms_tuple, first_bpm_time);

        let _ = writeln!(output, ";");

        Ok(output.into_bytes())
    }
}

/// Convert microseconds to beat position.
/// `start_time_us` is the time where beat count starts (beat 0).
fn us_to_beat(time_us: i64, bpm_points: &[&crate::model::TimingPoint], start_time_us: i64) -> f64 {
    if bpm_points.is_empty() {
        return 0.0;
    }

    let mut current_time_us = start_time_us;
    let mut current_beat: f64 = 0.0;
    let mut current_bpm = bpm_points[0].bpm; // Default to first BPM

    for i in 1..bpm_points.len() {
        let tp = bpm_points[i];
        if tp.time_us > time_us {
            break;
        }

        let elapsed_us = tp.time_us - current_time_us;
        let elapsed_beats = us_to_beats_at_bpm(elapsed_us, current_bpm);
        current_beat += elapsed_beats;
        current_time_us = tp.time_us;
        current_bpm = tp.bpm;
    }

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
fn encode_measures(output: &mut String, chart: &RoxChart, bpms: &[(i64, f32)], start_time_us: i64) {
    if chart.notes.is_empty() {
        // Empty chart - just one empty measure
        for _ in 0..4 {
            let _ = writeln!(output, "{}", "0".repeat(chart.key_count() as usize));
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
    let total_beats = us_to_beat_simple(max_time, bpms, start_time_us);

    let total_measures = if total_beats > 0.0 {
        (total_beats / 4.0).ceil() as usize + 1
    } else {
        1
    };

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

    // Resolve collisions: Abutting notes (Tail overwrites Next Note or vice versa)
    // If a Tail ('3') is at the same time/col as a Start ('1', '2', '4', 'M'),
    // StepMania cannot handle it (Tail requires release, Start requires press).
    // We convert the Hold into a Tap to prevent "hanging head" crashes.
    let len = events.len();

    // We iterate and mark modifications.
    // Note: events is sorted by time.
    // If times are equal, stable sort keeps relative order?
    // We generated Heads then Tails.
    // If Note A (Hold) ends at T, and Note B (Tap) starts at T.
    // events: [..., (T, A, 3), (T, B, 1), ...] (assuming A processed before B in chart? No)
    // chart.notes is sorted by time. A starts before B.
    // So (A_start) < (B_start).
    // So A processed first.
    // So (T, A, 3) pushed, then (T, B, 1) pushed.
    // Sort stable maintains order.
    // So events[i] = 3, events[i+1] = 1.

    for i in 0..len.saturating_sub(1) {
        let (t1, c1, ch1) = events[i];
        let (t2, c2, ch2) = events[i + 1];

        // Check for collision
        if t1 == t2 && c1 == c2 {
            // Case: Tail ('3') followed by Start ('1', '2', '4', 'M')
            if ch1 == '3' && (ch2 == '1' || ch2 == '2' || ch2 == '4' || ch2 == 'M') {
                // Collision!
                // Convert the Hold (Head at '2'/'4') to Tap ('1').
                // Mark Tail ('3') for removal.

                // Find the corresponding Head
                let mut head_found = false;
                for j in (0..i).rev() {
                    if events[j].1 == c1 {
                        if events[j].2 == '2' || events[j].2 == '4' {
                            // Found head. Convert to Tap.
                            events[j].2 = '1';
                            head_found = true;
                            break;
                        } else if events[j].2 == '3' {
                            // Another tail? Nested holds? Shouldn't happen in flat list unless logic wrong.
                            // Stop if we hit another tail, it means we missed the head or interleaved.
                            break;
                        }
                    }
                }

                if head_found {
                    events[i].2 = '0'; // Mark tail for removal (we'll filter '0' out output logic or now)
                }
            }
        }
    }

    // Group events by measure
    // Map: measure_index -> Vec<(beat_in_measure, col, char)>
    let mut measure_events: Vec<Vec<(f64, u8, char)>> = vec![Vec::new(); total_measures];

    for (time_us, col, ch) in events {
        if ch == '0' {
            continue;
        } // Skip removed tails

        let raw_beat = us_to_beat_simple(time_us, bpms, start_time_us);

        // If beat is negative, it's before the start. Skip or warn?
        if raw_beat < 0.0 {
            continue; // Cannot represent in SM M0
        }

        // Snap to grid (48th notes / 192 per measure) to handle floating point jitter
        const GRID_RESOLUTION: f64 = 48.0;
        let mut beat = (raw_beat * GRID_RESOLUTION).round() / GRID_RESOLUTION;

        // If the snapped beat is effectively an integer + epsilon, make sure it behaves
        if (beat - beat.round()).abs() < 1e-6 {
            beat = beat.round();
        }

        let measure_idx = (beat / 4.0).floor() as usize;
        let beat_in_measure = beat % 4.0;

        if measure_idx < measure_events.len() {
            measure_events[measure_idx].push((beat_in_measure, col, ch));
        } else {
            // Extend if needed
            if measure_idx >= measure_events.len() {
                measure_events.resize(measure_idx + 1, Vec::new());
            }
            measure_events[measure_idx].push((beat_in_measure, col, ch));
        }
    }

    // Generate each measure
    for (measure_num, events) in measure_events.iter().enumerate() {
        if measure_num > 0 {
            let _ = writeln!(output, ",");
        }

        // Try standard SM divisors
        let divisors = [4, 8, 12, 16, 24, 32, 48, 64, 96, 192];
        let mut best_divisor = 192;

        'divisor_loop: for &div in &divisors {
            // Check if all events align with this divisor
            for (beat_in_measure, _, _) in events {
                // Ideal position in lines for this divisor
                let ideal_line = beat_in_measure * (div as f64) / 4.0;
                let snapped_line = ideal_line.round();

                // If deviation is too high, this divisor is invalid
                if (ideal_line - snapped_line).abs() > 0.001 {
                    continue 'divisor_loop;
                }
            }

            // If we get here, all events aligned
            best_divisor = div;
            break;
        }

        // If measure is empty, force 4 lines to save space
        if events.is_empty() {
            best_divisor = 4;
        }

        let lines_per_measure = best_divisor;
        for i in 0..lines_per_measure {
            // Collect events on this line
            let mut line_chars: Vec<char> = vec!['0'; chart.key_count() as usize];

            for (beat_in_measure, col, ch) in events {
                // Check if this event belongs to this line
                // We use the same epsilon logic as above to match
                let event_line_pos = beat_in_measure * (lines_per_measure as f64) / 4.0;
                if (event_line_pos - i as f64).abs() < 0.001 {
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

fn us_to_beat_simple(time_us: i64, bpms: &[(i64, f32)], start_time_us: i64) -> f64 {
    if bpms.is_empty() {
        return (time_us - start_time_us) as f64 / 1_000_000.0 * 120.0 / 60.0;
    }

    let mut current_time_us = start_time_us;
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
