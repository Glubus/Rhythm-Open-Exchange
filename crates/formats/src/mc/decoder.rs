#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, Note, RoxChart, TimingPoint};
use rox_macros::Format;

use super::types::{McChart, McTimingPoint, beat_to_f64, beats_to_ms};

/// Decoder for Malody Key mode beatmaps.
#[derive(Format)]
#[format(extensions = ["mc"])]
pub struct McDecoder;

impl McDecoder {
    /// Convert a parsed `McChart` to `RoxChart`.
    ///
    /// # Errors
    /// Returns an error for unsupported Malody modes or malformed timing data.
    pub fn from_mc(mc: &McChart) -> RoxResult<RoxChart> {
        if mc.meta.mode != 0 {
            return Err(RoxError::InvalidFormat(format!(
                "Malody mode {} is not supported (only Key mode=0)",
                mc.meta.mode
            )));
        }
        if mc.time.is_empty() {
            return Err(RoxError::InvalidFormat(
                "Malody chart has no timing points".to_string(),
            ));
        }

        let audio = audio_metadata(mc);
        let timeline = BeatTimeline::new(&mc.time, audio.offset_ms);
        let mut chart = RoxChart::new(mc.meta.mode_ext.column);
        chart.metadata = build_metadata(mc, &audio);
        build_timing_points(mc, &timeline, &mut chart);
        build_notes(mc, &timeline, &mut chart);
        chart.notes.sort_by_key(|n| n.time_us);
        Ok(chart)
    }
}

#[derive(Debug, Clone, Default)]
struct AudioMetadata {
    file: String,
    offset_ms: i64,
}

fn audio_metadata(mc: &McChart) -> AudioMetadata {
    let mut audio = AudioMetadata::default();
    for note in mc.note.iter().filter(|note| note.note_type != 0) {
        if let Some(sound) = &note.sound {
            audio.file.clone_from(sound);
        }
        audio.offset_ms = note.offset.unwrap_or_default();
    }
    audio
}

fn build_metadata(mc: &McChart, audio: &AudioMetadata) -> Metadata {
    Metadata {
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
        audio_file: audio.file.clone().into(),
        background_file: (!mc.meta.background.is_empty())
            .then(|| mc.meta.background.clone().into()),
        audio_offset_us: -audio.offset_ms * 1000,
        preview_time_us: mc.meta.preview.unwrap_or(-1) * 1000,
        source: Some("Malody".into()),
        ..Metadata::default()
    }
}

fn build_timing_points(mc: &McChart, timeline: &BeatTimeline, chart: &mut RoxChart) {
    for section in &timeline.sections {
        #[allow(clippy::cast_possible_truncation)] // ms→µs: safe for any realistic timestamp
        let time_us = (section.offset_ms * 1000.0) as i64;
        chart.timing_points.push(TimingPoint::Bpm {
            time_us,
            #[allow(clippy::cast_possible_truncation)] // f64→f32: precision loss acceptable for BPM values
            bpm: section.bpm as f32,
            signature: section.signature,
        });
    }

    for effect in &mc.effect {
        #[allow(clippy::cast_possible_truncation)] // ms→µs: safe for any realistic timestamp
        let time_us = (timeline.ms_at_beat(beat_to_f64(&effect.beat)) * 1000.0) as i64;
        #[allow(clippy::cast_possible_truncation)]
        // f64→f32: precision loss acceptable for SV values
        chart
            .timing_points
            .push(TimingPoint::sv(time_us, effect.scroll as f32));
    }
    chart.timing_points.sort_by_key(TimingPoint::time_us);
}

fn build_notes(mc: &McChart, timeline: &BeatTimeline, chart: &mut RoxChart) {
    for mc_note in mc.note.iter().filter(|note| note.note_type == 0) {
        let Some(beat) = &mc_note.beat else { continue };
        let column = mc_note.column.unwrap_or_default();
        let time_us = beat_to_time_us(timeline, beat);
        let note = if let Some(endbeat) = &mc_note.endbeat {
            let end_us = beat_to_time_us(timeline, endbeat);
            Note::hold(time_us, (end_us - time_us).max(1), column)
        } else {
            Note::tap(time_us, column)
        };
        chart.notes.push(note);
    }
}

fn beat_to_time_us(timeline: &BeatTimeline, beat: &super::types::Beat) -> i64 {
    #[allow(clippy::cast_possible_truncation)] // ms→µs: safe for any realistic timestamp
    let time_us = (timeline.ms_at_beat(beat_to_f64(beat)) * 1000.0).max(0.0) as i64;
    time_us
}

#[derive(Debug, Clone)]
struct BeatSection {
    beat: f64,
    bpm: f64,
    offset_ms: f64,
    signature: u8,
}

#[derive(Debug, Clone)]
struct BeatTimeline {
    sections: Vec<BeatSection>,
}

impl BeatTimeline {
    fn new(points: &[McTimingPoint], audio_offset_ms: i64) -> Self {
        let mut sections = Vec::with_capacity(points.len());
        let mut offset_ms = -audio_offset_ms as f64;
        for (idx, point) in points.iter().enumerate() {
            if let Some(prev) = idx.checked_sub(1).and_then(|prev_idx| points.get(prev_idx)) {
                let beat_diff = beat_to_f64(&point.beat) - beat_to_f64(&prev.beat);
                offset_ms += beats_to_ms(beat_diff, prev.bpm);
            }
            sections.push(BeatSection {
                beat: beat_to_f64(&point.beat),
                bpm: point.bpm,
                offset_ms,
                signature: point.sign,
            });
        }
        Self { sections }
    }

    fn ms_at_beat(&self, beat: f64) -> f64 {
        let section = self
            .sections
            .iter()
            .rev()
            .find(|section| section.beat <= beat)
            .unwrap_or(&self.sections[0]);
        section.offset_ms + beats_to_ms(beat - section.beat, section.bpm)
    }
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

    #[test]
    fn test_reject_missing_timing_points() {
        let json = r#"{
            "meta":{"mode":0,"song":{"title":"T","artist":"A"},"mode_ext":{"column":4}},
            "time":[],
            "note":[]
        }"#;
        assert!(<McDecoder as Decoder>::decode_inner(json.as_bytes()).is_err());
    }

    #[test]
    fn test_decode_scroll_effect() {
        let json = r#"{
            "meta":{"mode":0,"song":{"title":"T","artist":"A"},"mode_ext":{"column":4}},
            "time":[{"beat":[0,0,1],"bpm":120}],
            "effect":[{"beat":[1,0,1],"scroll":1.5}],
            "note":[]
        }"#;
        let chart = <McDecoder as Decoder>::decode_inner(json.as_bytes()).unwrap();
        assert!(chart.timing_points.iter().any(TimingPoint::is_sv));
    }
}
