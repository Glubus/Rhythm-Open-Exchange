#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{string::String, string::ToString, vec::Vec};

use rox::codec::Encoder;
use rox::error::RoxResult;
use rox::model::{NoteType, RoxChart, TimingPoint};
use serde::Serialize;

pub struct McEncoder;
crate::impl_format!(McEncoder, ["mc"]);

impl McEncoder {
    fn to_mc_json(chart: &RoxChart) -> McOutput {
        McOutput {
            effect: build_effects(chart),
            extra: serde_json::Value::Object(serde_json::Map::new()),
            meta: build_meta(chart),
            time: build_timing_points(chart),
            note: build_notes(chart),
        }
    }
}

const BEAT_DENOM: i128 = 60_000_000_000; // 60_000_000 us/beat * 1000 for BPM precision

fn ms_to_beat(time_us: i64, chart: &RoxChart) -> [i32; 3] {
    let time_us = time_us.max(0);
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
        if tp.is_sv() {
            continue;
        }
        let end_us = if tp.time_us() <= time_us {
            tp.time_us()
        } else {
            time_us
        };
        if end_us > prev_us {
            let delta = (end_us - prev_us) as i128;
            #[allow(clippy::cast_possible_truncation)]
            // scaled BPM remains small for realistic charts
            let bpm_scaled = (prev_bpm * 1000.0) as i128;
            num += delta * bpm_scaled;
        }
        if tp.time_us() > time_us {
            reached_target = true;
            break;
        }
        prev_us = tp.time_us();
        prev_bpm = tp.bpm_value().unwrap_or(120.0) as f64;
    }
    if !reached_target && time_us > prev_us {
        let delta = (time_us - prev_us) as i128;
        #[allow(clippy::cast_possible_truncation)] // scaled BPM remains small for realistic charts
        let bpm_scaled = (prev_bpm * 1000.0) as i128;
        num += delta * bpm_scaled;
    }
    num
}

/// Convert exact rational beats (num/denom) to [measure, num, den] triple.
/// No floating-point arithmetic — mathematically exact.
fn rational_to_beat_triple_i128(num: i128, denom: i128) -> [i32; 3] {
    #[allow(clippy::cast_possible_truncation)] // beat index fits in i32 for realistic charts
    let measure = (num / denom) as i32;
    let frac_num = num % denom;
    if frac_num == 0 {
        return [measure, 0, 1];
    }

    let g = gcd_i128(frac_num, denom);
    #[allow(clippy::cast_possible_truncation)]
    // reduced beat fraction fits in i32 for realistic charts
    let n = (frac_num / g) as i32;
    #[allow(clippy::cast_possible_truncation)]
    // reduced beat fraction fits in i32 for realistic charts
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

fn build_meta(chart: &RoxChart) -> McMetaOut {
    let title = chart.metadata.title.to_string();
    let artist = chart.metadata.artist.to_string();
    McMetaOut {
        mode: 0,
        song: McSongOut {
            title: title.clone(),
            titleorg: Some(title),
            artist: artist.clone(),
            artistorg: Some(artist),
            id: String::new(),
            source: chart.metadata.source.as_ref().map(ToString::to_string),
            org: McSongOrg::default(),
        },
        mode_ext: McModeExtOut {
            column: chart.key_count,
        },
        background: chart
            .metadata
            .background_file
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default(),
        creator: chart.metadata.creator.to_string(),
        version: chart.metadata.difficulty_name.to_string(),
        preview: (chart.metadata.preview_time_us >= 0)
            .then(|| chart.metadata.preview_time_us / 1000),
        id: String::new(),
        cover: String::new(),
        time: 0,
    }
}

fn build_timing_points(chart: &RoxChart) -> Vec<McTimingOut> {
    chart
        .timing_points
        .iter()
        .filter_map(|tp| {
            let TimingPoint::Bpm {
                time_us,
                bpm,
                signature,
            } = *tp
            else {
                return None;
            };
            Some(McTimingOut {
                beat: ms_to_beat(time_us, chart),
                bpm: f64::from(bpm),
                sign: signature,
            })
        })
        .collect()
}

fn build_effects(chart: &RoxChart) -> Vec<McEffectOut> {
    chart
        .timing_points
        .iter()
        .filter_map(|tp| {
            let TimingPoint::Sv {
                time_us,
                scroll_speed,
            } = *tp
            else {
                return None;
            };
            Some(McEffectOut {
                beat: ms_to_beat(time_us, chart),
                scroll: f64::from(scroll_speed),
            })
        })
        .collect()
}

fn build_notes(chart: &RoxChart) -> Vec<McNoteOut> {
    let mut notes = Vec::with_capacity(chart.notes.len() + 1);
    notes.push(McNoteOut {
        note_type: 1,
        sound: Some(chart.metadata.audio_file.to_string()),
        offset: Some(-chart.metadata.audio_offset_us / 1000),
        vol: Some(-1.0),
        beat: Some([0, 0, 1]),
        column: None,
        endbeat: None,
    });

    notes.extend(chart.notes.iter().map(|note| {
        let beat = ms_to_beat(note.time_us, chart);
        let endbeat = match note.note_type {
            NoteType::Tap | NoteType::Mine => None,
            NoteType::Hold { duration_us } | NoteType::Burst { duration_us } => {
                Some(ms_to_beat(note.time_us + duration_us, chart))
            }
        };
        McNoteOut {
            note_type: 0,
            beat: Some(beat),
            column: Some(note.column),
            endbeat,
            sound: None,
            offset: None,
            vol: None,
        }
    }));
    notes
}

#[derive(Serialize)]
struct McOutput {
    meta: McMetaOut,
    time: Vec<McTimingOut>,
    note: Vec<McNoteOut>,
    effect: Vec<McEffectOut>,
    extra: serde_json::Value,
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
    id: String,
    cover: String,
    time: i64,
}
#[derive(Debug, Clone, Default, Serialize)]
struct McSongOrg {
    title: String,
    artist: String,
    source: String,
}

#[derive(Serialize)]
struct McSongOut {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    titleorg: Option<String>,
    artist: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    artistorg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    id: String,
    org: McSongOrg,
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

#[cfg(test)]
mod tests {
    use super::*;
    use rox::{codec::Encoder, model::Note};

    #[test]
    fn test_encode_preserves_scroll_effects() {
        let mut chart = RoxChart::new(4);
        chart.metadata.audio_file = "audio.ogg".into();
        chart.timing_points.push(TimingPoint::bpm(0, 120.0));
        chart.timing_points.push(TimingPoint::sv(500_000, 1.5));
        let encoded = McEncoder::encode(&chart).expect("encode failed");
        let json: serde_json::Value = serde_json::from_slice(&encoded).expect("json");
        assert_eq!(json["effect"].as_array().unwrap().len(), 1);
        assert_eq!(json["effect"][0]["scroll"], 1.5);
    }

    #[test]
    fn test_encode_hold_endbeat() {
        let mut chart = RoxChart::new(4);
        chart.timing_points.push(TimingPoint::bpm(0, 120.0));
        chart.notes.push(Note::hold(1_000_000, 500_000, 2));
        let encoded = McEncoder::encode(&chart).expect("encode failed");
        let json: serde_json::Value = serde_json::from_slice(&encoded).expect("json");
        assert!(json["note"][1]["endbeat"].is_array());
    }
}
