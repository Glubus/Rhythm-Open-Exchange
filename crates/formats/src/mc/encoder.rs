//! Encoder for converting `RoxChart` to Malody .mc JSON.

use rox::codec::Encoder;
use rox::error::RoxResult;
use rox::model::{NoteType, RoxChart, TimingPoint};
use serde::Serialize;

pub struct McEncoder;

impl McEncoder {
    fn to_mc_json(chart: &RoxChart) -> McOutput {
        let key_count = chart.key_count;

        let mut time = Vec::new();
        let mut effect = Vec::new();
        for tp in &chart.timing_points {
            if tp.is_sv() {
                effect.push(McEffectOut {
                    beat: ms_to_beat(tp.time_us(), chart),
                    scroll: tp.scroll_speed().unwrap_or(1.0) as f64,
                });
            } else {
                time.push(McTimingOut {
                    beat: ms_to_beat(tp.time_us(), chart),
                    bpm: tp.bpm_value().unwrap_or(120.0) as f64,
                    sign: if let TimingPoint::Bpm { signature, .. } = tp { *signature } else { 4 },
                });
            }
        }

        let mut notes: Vec<McNoteOut> = Vec::new();
        notes.push(McNoteOut {
            note_type: 1,
            sound: Some(chart.metadata.audio_file.to_string()),
            offset: Some((-chart.metadata.audio_offset_us / 1000) as i64),
            vol: Some(100.0),
            beat: None,
            column: None,
            endbeat: None,
        });

        for note in &chart.notes {
            let beat = ms_to_beat(note.time_us, chart);
            let col = note.column;
            match note.note_type {
                NoteType::Tap | NoteType::Mine => notes.push(McNoteOut {
                    note_type: 0,
                    beat: Some(beat),
                    column: Some(col),
                    endbeat: None,
                    sound: None,
                    offset: None,
                    vol: None,
                }),
                NoteType::Hold { duration_us } | NoteType::Burst { duration_us } => {
                    let end_us = note.time_us + duration_us;
                    notes.push(McNoteOut {
                        note_type: 0,
                        beat: Some(beat),
                        column: Some(col),
                        endbeat: Some(ms_to_beat(end_us, chart)),
                        sound: None,
                        offset: None,
                        vol: None,
                    });
                }
            }
        }

        let t = &chart.metadata.title;
        let a = &chart.metadata.artist;
        McOutput {
            meta: McMetaOut {
                mode: 0,
                song: McSongOut {
                    title: t.to_string(),
                    titleorg: Some(t.to_string()),
                    artist: a.to_string(),
                    artistorg: Some(a.to_string()),
                },
                mode_ext: McModeExtOut { column: key_count },
                background: chart
                    .metadata
                    .background_file
                    .as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                creator: chart.metadata.creator.to_string(),
                version: chart.metadata.difficulty_name.to_string(),
                preview: (chart.metadata.preview_time_us >= 0)
                    .then(|| chart.metadata.preview_time_us / 1000),
            },
            time,
            note: notes,
            effect,
        }
    }
}

const BEAT_DENOM: i128 = 60_000_000_000; // 60_000_000 us/beat * 1000 for BPM precision

fn ms_to_beat(time_us: i64, chart: &RoxChart) -> [i32; 3] {
    let time_us = time_us.max(0);
    // Accumulate exact beats as a rational: total_num / BEAT_DENOM
    let total_num = cumulative_beats_num(time_us, chart);
    rational_to_beat_triple_i128(total_num, BEAT_DENOM)
}

/// Compute cumulative beats numerator up to time_us.
/// Each BPM section contributes (delta_us * bpm * 1000) to the numerator.
fn cumulative_beats_num(time_us: i64, chart: &RoxChart) -> i128 {
    let mut num: i128 = 0;
    let mut prev_us: i64 = 0;
    let mut prev_bpm: f64 = 120.0;
    let mut reached_target = false;

    for tp in &chart.timing_points {
        if tp.is_sv() { continue; }
        let end_us = if tp.time_us() <= time_us { tp.time_us() } else { time_us };
        if end_us > prev_us {
            let delta = (end_us - prev_us) as i128;
            let bpm_scaled = (prev_bpm * 1000.0) as i128;
            num += delta * bpm_scaled;
        }
        if tp.time_us() > time_us { reached_target = true; break; }
        prev_us = tp.time_us();
        prev_bpm = tp.bpm_value().unwrap_or(120.0) as f64;
    }
    // Tail: from last BPM point to time_us
    if !reached_target && time_us > prev_us {
        let delta = (time_us - prev_us) as i128;
        let bpm_scaled = (prev_bpm * 1000.0) as i128;
        num += delta * bpm_scaled;
    }
    num
}

/// Convert exact rational beats (num/denom) to [measure, num, den] triple.
/// No floating-point arithmetic — mathematically exact.
fn rational_to_beat_triple_i128(num: i128, denom: i128) -> [i32; 3] {
    let measure = (num / denom) as i32;
    let frac_num = num % denom;
    if frac_num == 0 { return [measure, 0, 1]; }

    // GCD-simplify for compact representation
    let g = gcd_i128(frac_num, denom);
    let n = (frac_num / g) as i32;
    let d = (denom / g) as i32;
    [measure, n, d]
}

pub(crate) fn gcd_i128(a: i128, b: i128) -> i128 {
    if b == 0 { a } else { gcd_i128(b, a % b) }
}

impl Encoder for McEncoder {
    fn encode_inner(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        serde_json::to_vec_pretty(&Self::to_mc_json(chart))
            .map_err(|e| rox::error::RoxError::InvalidFormat(format!("Malody JSON encode: {e}")))
    }
}

#[derive(Serialize)]
struct McOutput {
    meta: McMetaOut,
    time: Vec<McTimingOut>,
    note: Vec<McNoteOut>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    effect: Vec<McEffectOut>,
}
#[derive(Serialize)]
struct McMetaOut {
    mode: u8,
    song: McSongOut,
    mode_ext: McModeExtOut,
    background: String,
    creator: String,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    preview: Option<i64>,
}
#[derive(Serialize)]
struct McSongOut {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    titleorg: Option<String>,
    artist: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    artistorg: Option<String>,
}
#[derive(Serialize)]
struct McModeExtOut {
    column: u8,
}
#[derive(Serialize)]
struct McTimingOut {
    beat: [i32; 3],
    bpm: f64,
    #[serde(skip_serializing_if = "is_default_sign")]
    sign: u8,
}
fn is_default_sign(s: &u8) -> bool {
    *s == 4
}
#[derive(Serialize)]
struct McNoteOut {
    #[serde(rename = "type")]
    note_type: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    sound: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    offset: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vol: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    beat: Option<[i32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    endbeat: Option<[i32; 3]>,
}
#[derive(Serialize)]
struct McEffectOut {
    beat: [i32; 3],
    scroll: f64,
}
