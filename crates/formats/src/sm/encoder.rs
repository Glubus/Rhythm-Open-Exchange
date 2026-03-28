#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec, format};

use core::fmt::Write;
use rox::codec::Encoder;
use rox::error::RoxResult;
use rox::model::{NoteType, RoxChart};
use rox_macros::Format;

#[derive(Format)]
#[format(extensions = ["sm"])]
pub struct SmEncoder;

impl Encoder for SmEncoder {
    fn encode_inner(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        let mut out = String::new();
        write_metadata_section(&mut out, chart);
        write_bpms_section(&mut out, chart);
        write_notes_section(&mut out, chart);
        let _ = writeln!(out, ";");
        Ok(out.into_bytes())
    }
}

fn write_metadata_section(out: &mut String, chart: &RoxChart) {
    let _ = writeln!(out, "#TITLE:{};", chart.metadata.title);
    let _ = writeln!(out, "#SUBTITLE:;");
    let _ = writeln!(out, "#ARTIST:{};", chart.metadata.artist);
    let _ = writeln!(out, "#TITLETRANSLIT:;#ARTISTTRANSLIT:;#GENRE:;");
    let _ = writeln!(out, "#CREDIT:{};", chart.metadata.creator);
    let _ = writeln!(out, "#BANNER:;");
    if let Some(bg) = &chart.metadata.background_file {
        let _ = writeln!(out, "#BACKGROUND:{bg};");
    } else {
        let _ = writeln!(out, "#BACKGROUND:;");
    }
    let _ = writeln!(out, "#LYRICSPATH:;#CDTITLE:;");
    let _ = writeln!(out, "#MUSIC:{};", chart.metadata.audio_file);
    let first_bpm_time = chart.timing_points.iter()
        .find(|tp| tp.is_bpm())
        .map_or(0, rox::model::TimingPoint::time_us);
    #[allow(clippy::cast_precision_loss)] // i64→f64: precision loss acceptable for timing values
    let offset = first_bpm_time as f64 / 1_000_000.0;
    let _ = writeln!(out, "#OFFSET:{offset:.6};");
    #[allow(clippy::cast_precision_loss)] // i64→f64: precision loss acceptable for timing values
    let sample_start = chart.metadata.preview_time_us as f64 / 1_000_000.0;
    #[allow(clippy::cast_precision_loss)] // i64→f64: precision loss acceptable for timing values
    let sample_length = chart.metadata.preview_duration_us as f64 / 1_000_000.0;
    let _ = writeln!(out, "#SAMPLESTART:{sample_start:.3};");
    let _ = writeln!(out, "#SAMPLELENGTH:{sample_length:.3};");
    let _ = writeln!(out, "#SELECTABLE:YES;");
}

fn write_bpms_section(out: &mut String, chart: &RoxChart) {
    out.push_str("#BPMS:");
    let bpm_pts: Vec<_> = chart.timing_points.iter().filter(|tp| tp.is_bpm()).collect();
    let first_time = bpm_pts.first().map_or(0, |tp| tp.time_us());
    let bpm_tuples = bpm_pts_as_tuples(&bpm_pts);
    for (i, tp) in bpm_pts.iter().enumerate() {
        let beat = us_to_beat_simple(tp.time_us(), &bpm_tuples, first_time);
        let bpm = tp.bpm_value().unwrap_or(120.0);
        if i > 0 { out.push(','); }
        if (beat - beat.round()).abs() < 0.001 {
            let _ = write!(out, "{beat:.0}={bpm:.3}");
        } else {
            let _ = write!(out, "{beat:.3}={bpm:.3}");
        }
    }
    let _ = writeln!(out, ";");
    let _ = writeln!(out, "#STOPS:;");
    let _ = writeln!(out);
}

fn bpm_pts_as_tuples(pts: &[&rox::model::TimingPoint]) -> Vec<(i64, f32)> {
    pts.iter().map(|tp| (tp.time_us(), tp.bpm_value().unwrap_or(120.0))).collect()
}

fn write_notes_section(out: &mut String, chart: &RoxChart) {
    let stepstype = match chart.key_count {
        6 => "dance-solo", 8 => "dance-double", _ => "dance-single",
    };
    let difficulty = match chart.metadata.difficulty_name.as_str() {
        "Beginner" | "Easy" | "Medium" | "Hard" | "Challenge" | "Edit" => {
            chart.metadata.difficulty_name.as_str()
        }
        _ => "Hard",
    };
    let _ = writeln!(out, "#NOTES:");
    let _ = writeln!(out, "     {stepstype}:");
    let _ = writeln!(out, "     :");
    let _ = writeln!(out, "     {difficulty}:");
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // value is non-negative in valid input; meter fits in u32
    let meter = chart.metadata.difficulty_value.unwrap_or(1.0) as u32;
    let _ = writeln!(out, "     {meter}:");
    let _ = writeln!(out, "     0,0,0,0,0:");
    let first_time = chart.timing_points.iter()
        .find(|tp| tp.is_bpm()).map_or(0, rox::model::TimingPoint::time_us);
    let bpms: Vec<(i64, f32)> = chart.timing_points.iter()
        .filter(|tp| tp.is_bpm())
        .map(|tp| (tp.time_us(), tp.bpm_value().unwrap_or(120.0)))
        .collect();
    encode_measures(out, chart, &bpms, first_time);
}

fn us_to_beats_at_bpm(us: i64, bpm: f32) -> f64 {
    #[allow(clippy::cast_precision_loss)] // i64→f64: precision loss acceptable for timing values
    let seconds = us as f64 / 1_000_000.0;
    seconds * f64::from(bpm) / 60.0
}

fn us_to_beat_simple(time_us: i64, bpms: &[(i64, f32)], start_time_us: i64) -> f64 {
    if bpms.is_empty() {
        #[allow(clippy::cast_precision_loss)] // i64→f64: precision loss acceptable for timing values
        return (time_us - start_time_us) as f64 / 1_000_000.0 * 120.0 / 60.0;
    }
    let mut current_time = start_time_us;
    let mut current_beat = 0.0;
    let mut current_bpm = bpms[0].1;
    for &(bpm_time, new_bpm) in &bpms[1..] {
        if bpm_time > time_us { break; }
        current_beat += us_to_beats_at_bpm(bpm_time - current_time, current_bpm);
        current_time = bpm_time;
        current_bpm = new_bpm;
    }
    current_beat + us_to_beats_at_bpm(time_us - current_time, current_bpm)
}

fn encode_measures(out: &mut String, chart: &RoxChart, bpms: &[(i64, f32)], start_time_us: i64) {
    if chart.notes.is_empty() {
        for _ in 0..4 {
            let _ = writeln!(out, "{}", "0".repeat(chart.key_count as usize));
        }
        return;
    }
    let max_time = chart.notes.iter().map(rox::model::Note::end_time_us).max().unwrap_or(0);
    let total_beats = us_to_beat_simple(max_time, bpms, start_time_us);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // value is non-negative in valid input; measure count fits in usize
    let total_measures = if total_beats > 0.0 { (total_beats / 4.0).ceil() as usize + 1 } else { 1 };
    let events = build_events(chart);
    let measure_events = group_into_measures(&events, bpms, start_time_us, total_measures);
    write_measures(out, &measure_events, chart.key_count);
}

fn build_events(chart: &RoxChart) -> Vec<(i64, u8, char)> {
    let mut events: Vec<(i64, u8, char)> = Vec::new();
    for note in &chart.notes {
        match &note.note_type {
            NoteType::Tap => events.push((note.time_us, note.column, '1')),
            NoteType::Hold { duration_us } => {
                events.push((note.time_us, note.column, '2'));
                events.push((note.time_us + duration_us, note.column, '3'));
            }
            NoteType::Burst { duration_us } => {
                events.push((note.time_us, note.column, '4'));
                events.push((note.time_us + duration_us, note.column, '3'));
            }
            NoteType::Mine => events.push((note.time_us, note.column, 'M')),
        }
    }
    events.sort_by_key(|(t, _, _)| *t);
    events
}

const GRID: f64 = 48.0;

fn group_into_measures(
    events: &[(i64, u8, char)],
    bpms: &[(i64, f32)],
    start_time_us: i64,
    total_measures: usize,
) -> Vec<Vec<(f64, u8, char)>> {
    let mut measure_events: Vec<Vec<(f64, u8, char)>> = vec![Vec::new(); total_measures];
    for &(time_us, col, ch) in events {
        if ch == '0' { continue; }
        let raw_beat = us_to_beat_simple(time_us, bpms, start_time_us);
        if raw_beat < 0.0 { continue; }
        let beat = (raw_beat * GRID).round() / GRID;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // value is non-negative in valid input; measure index fits in usize
        let measure_idx = (beat / 4.0).floor() as usize;
        let beat_in_measure = beat % 4.0;
        if measure_idx >= measure_events.len() {
            measure_events.resize(measure_idx + 1, Vec::new());
        }
        measure_events[measure_idx].push((beat_in_measure, col, ch));
    }
    measure_events
}

fn write_measures(out: &mut String, measure_events: &[Vec<(f64, u8, char)>], key_count: u8) {
    for (i, events) in measure_events.iter().enumerate() {
        if i > 0 { let _ = writeln!(out, ","); }
        write_measure(out, events, key_count);
    }
}

fn write_measure(out: &mut String, events: &[(f64, u8, char)], key_count: u8) {
    const DIVISORS: [usize; 10] = [4, 8, 12, 16, 24, 32, 48, 64, 96, 192];
    let mut best = 192usize;
    'outer: for &div in &DIVISORS {
        for (beat_in_measure, _, _) in events {
            #[allow(clippy::cast_precision_loss)] // i64→f64: precision loss acceptable for timing values
            let ideal = beat_in_measure * (div as f64) / 4.0;
            if (ideal - ideal.round()).abs() > 0.001 { continue 'outer; }
        }
        best = div;
        break;
    }
    if events.is_empty() { best = 4; }
    for i in 0..best {
        let mut line: Vec<char> = vec!['0'; key_count as usize];
        for (beat_in_measure, col, ch) in events {
            #[allow(clippy::cast_precision_loss)] // i64→f64: precision loss acceptable for timing values
            let pos = beat_in_measure * (best as f64) / 4.0;
            #[allow(clippy::cast_precision_loss)] // i64→f64: precision loss acceptable for timing values
            if (pos - i as f64).abs() < 0.001 && (*col as usize) < line.len() {
                line[*col as usize] = *ch;
            }
        }
        let _ = writeln!(out, "{}", line.iter().collect::<String>());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::{codec::{Decoder, Encoder}, model::{Note, TimingPoint}};
    use rstest::rstest;

    #[rstest]
    fn test_encode_basic() {
        let mut chart = rox::model::RoxChart::new(4);
        chart.metadata.title = "Test".into();
        chart.metadata.audio_file = "song.ogg".into();
        chart.timing_points.push(TimingPoint::bpm(0, 120.0));
        chart.notes.push(Note::tap(0, 0));
        chart.notes.push(Note::tap(500_000, 1));
        let encoded = SmEncoder::encode(&chart).expect("encode failed");
        let s = core::str::from_utf8(&encoded).expect("utf8");
        assert!(s.contains("#TITLE:Test;"));
        assert!(s.contains("#BPMS:"));
        assert!(s.contains("#NOTES:"));
    }

    #[rstest]
    fn test_roundtrip_4k() {
        use crate::sm::decoder::SmDecoder;
        let data = rox_test_utils::get_test_asset("stepmania/4k.sm");
        let chart1 = SmDecoder::decode(&data).expect("decode 1 failed");
        let encoded = SmEncoder::encode(&chart1).expect("encode failed");
        let chart2 = SmDecoder::decode(&encoded).expect("decode 2 failed");
        assert_eq!(chart1.key_count, chart2.key_count);
        assert_eq!(chart1.notes.len(), chart2.notes.len());
    }
}
