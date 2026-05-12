//! Decoder for converting Malody .mc to `RoxChart`.

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, Note, RoxChart, TimingPoint};

use super::types::{beat_to_f64, beats_to_ms, McChart};

/// Decoder for Malody Key mode beatmaps.
pub struct McDecoder;

impl McDecoder {
    /// Convert a parsed `McChart` to `RoxChart`.
    #[must_use]
    pub fn from_mc(mc: &McChart) -> RoxResult<RoxChart> {
        if mc.meta.mode != 0 {
            return Err(RoxError::InvalidFormat(format!(
                "Malody mode {} is not supported (only Key mode=0)",
                mc.meta.mode
            )));
        }

        let key_count = mc.meta.mode_ext.column;
        let mut chart = RoxChart::new(key_count);

        // ── Extract audio metadata from sound-meta notes ──
        let mut audio_file = String::new();
        let mut audio_offset_ms: i64 = 0;
        for n in &mc.note {
            if n.note_type != 0 {
                if let Some(ref s) = n.sound {
                    audio_file.clone_from(s);
                }
                audio_offset_ms = n.offset.unwrap_or(0);
            }
        }

        // ── Metadata ──
        chart.metadata = Metadata {
            title: mc
                .meta
                .song
                .titleorg
                .clone()
                .unwrap_or_else(|| mc.meta.song.title.clone())
                .into(),
            artist: mc
                .meta
                .song
                .artistorg
                .clone()
                .unwrap_or_else(|| mc.meta.song.artist.clone())
                .into(),
            creator: mc.meta.creator.clone().into(),
            difficulty_name: mc.meta.version.clone().into(),
            audio_file: audio_file.into(),
            background_file: if mc.meta.background.is_empty() {
                None
            } else {
                Some(mc.meta.background.clone().into())
            },
            audio_offset_us: -audio_offset_ms * 1000,
            preview_time_us: mc.meta.preview.unwrap_or(-1) * 1000,
            source: Some("Malody".into()),
            ..Default::default()
        };

        // ── BPM timeline: parallel arrays (bpm, cumulative_offset_ms) ──
        let n_bpm = mc.time.len();
        let mut bpms: Vec<f64> = Vec::with_capacity(n_bpm);
        let mut offsets_ms: Vec<f64> = Vec::with_capacity(n_bpm);
        bpms.push(mc.time[0].bpm);
        offsets_ms.push(-audio_offset_ms as f64);
        for i in 1..n_bpm {
            let beat_diff = beat_to_f64(&mc.time[i].beat) - beat_to_f64(&mc.time[i - 1].beat);
            let offset = offsets_ms[i - 1] + beats_to_ms(beat_diff, bpms[i - 1]);
            bpms.push(mc.time[i].bpm);
            offsets_ms.push(offset);
        }

        // ── BPM timing points ──
        for i in 0..n_bpm {
            let tp = &mc.time[i];
            #[allow(clippy::cast_possible_truncation)]
            let time_us = (offsets_ms[i] * 1000.0) as i64;
            let t = TimingPoint::Bpm { time_us, bpm: tp.bpm as f32, signature: tp.sign };
            chart.timing_points.push(t);
        }

        // ── SV effects ──
        for eff in &mc.effect {
            let bf = beat_to_f64(&eff.beat);
            let (_, off) = bpm_offset_at_beat(&bpms, &offsets_ms, &mc.time, bf);
            #[allow(clippy::cast_possible_truncation)]
            chart.timing_points.push(TimingPoint::sv(
                (off * 1000.0) as i64,
                if eff.scroll == 0.0 {
                    0.0
                } else {
                    eff.scroll as f32
                },
            ));
        }
        chart.timing_points.sort_by_key(|tp| tp.time_us());

        // ── Notes ──
        for n in &mc.note {
            if n.note_type != 0 {
                continue;
            }
            let Some(beat) = &n.beat else {
                continue;
            };
            let column = n.column.unwrap_or(0);
            let bf = beat_to_f64(beat);
            let (_, off) = bpm_offset_at_beat(&bpms, &offsets_ms, &mc.time, bf);
            #[allow(clippy::cast_possible_truncation)]
            let time_us = (off * 1000.0).max(0.0) as i64;

            let note = if let Some(ref endbeat) = n.endbeat {
                let ef = beat_to_f64(endbeat);
                let (_, eoff) = bpm_offset_at_beat(&bpms, &offsets_ms, &mc.time, ef);
                #[allow(clippy::cast_possible_truncation)]
                let end_us = (eoff * 1000.0).max(0.0) as i64;
                Note::hold(time_us, (end_us - time_us).max(1), column)
            } else {
                Note::tap(time_us, column)
            };
            chart.notes.push(note);
        }
        chart.notes.sort_by_key(|n| n.time_us);
        Ok(chart)
    }
}

/// Find BPM section for a beat position → (index, absolute_ms).
fn bpm_offset_at_beat(
    bpms: &[f64],
    offsets_ms: &[f64],
    time_points: &[super::types::McTimingPoint],
    beat: f64,
) -> (usize, f64) {
    let mut idx = 0usize;
    for (i, tp) in time_points.iter().enumerate() {
        if beat_to_f64(&tp.beat) <= beat {
            idx = i;
        } else {
            break;
        }
    }
    let diff = beat - beat_to_f64(&time_points[idx].beat);
    (idx, offsets_ms[idx] + beats_to_ms(diff, bpms[idx]))
}

impl Decoder for McDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        let mc: McChart = serde_json::from_slice(data)
            .map_err(|e| RoxError::InvalidFormat(format!("Malody JSON: {e}")))?;
        Self::from_mc(&mc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::codec::Decoder;

    #[test]
    fn test_decode_minimal_4k() {
        let json = r#"{
            "meta": {"mode":0,"song":{"title":"T","artist":"A"},"mode_ext":{"column":4},"background":"","creator":"c","version":"E"},
            "time":[{"beat":[0,0,1],"bpm":180}],
            "note":[{"type":1,"sound":"t.ogg","offset":0},{"beat":[1,0,4],"column":0},{"beat":[2,0,4],"endbeat":[3,0,4],"column":1}]
        }"#;
        let chart = <McDecoder as Decoder>::decode_inner(json.as_bytes()).unwrap();
        assert_eq!(chart.key_count, 4);
        assert_eq!(chart.notes.len(), 2);
        assert!(!chart.timing_points.is_empty());
    }

    #[test]
    fn test_reject_non_key_mode() {
        let json = r#"{"meta":{"mode":1,"song":{"title":"T","artist":"A"},"mode_ext":{"column":4},"background":"","creator":"c","version":"E"},"time":[{"beat":[0,0,1],"bpm":120}],"note":[{"type":1,"sound":"x.ogg","offset":0}]}"#;
        assert!(<McDecoder as Decoder>::decode_inner(json.as_bytes()).is_err());
    }
}
