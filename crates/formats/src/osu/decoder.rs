#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap;
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Hitsound, Metadata, Note, RoxChart, TimingPoint};
use rox_macros::Format;

use super::parser;

/// Options for [`OsuDecoder::decode_with_options`].
#[derive(Debug, Clone, Default)]
pub struct OsuDecodeOptions {
    /// Re-arrange the first BPM timing point to 1µs before the first note when
    /// it would otherwise fail the `BpmAfterFirstNote` validation.
    ///
    /// **Warning:** this silently repairs broken beatmaps. Prefer fixing the
    /// source file. Emits a `tracing::warn!` when the repair is applied.
    pub re_arrange_bpm: bool,
}

/// Decoder for osu!mania beatmaps.
#[derive(Format)]
#[format(extensions = ["osu"])]
pub struct OsuDecoder;

impl OsuDecoder {
    /// Decode with explicit options.
    ///
    /// # Errors
    /// Returns an error if parsing or validation fails. With
    /// [`OsuDecodeOptions::re_arrange_bpm`] enabled, `BpmAfterFirstNote` is
    /// repaired instead of surfaced.
    pub fn decode_with_options(data: &[u8], opts: &OsuDecodeOptions) -> RoxResult<RoxChart> {
        let beatmap = parser::parse(data)?;
        if beatmap.general.mode != 3 {
            return Err(RoxError::InvalidFormat(
                format!("Not a mania beatmap (mode={}, expected 3)", beatmap.general.mode),
            ));
        }
        let mut chart = from_beatmap(&beatmap);
        if opts.re_arrange_bpm {
            re_arrange_bpm_if_needed(&mut chart);
        }
        chart.validate()?;
        Ok(chart)
    }
}

impl Decoder for OsuDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        let beatmap = parser::parse(data)?;
        if beatmap.general.mode != 3 {
            return Err(RoxError::InvalidFormat(
                format!("Not a mania beatmap (mode={}, expected 3)", beatmap.general.mode)
            ));
        }
        Ok(from_beatmap(&beatmap))
    }
}

/// Shifts the first BPM timing point to 1µs before the first note when it
/// would fail `BpmAfterFirstNote` validation.
fn re_arrange_bpm_if_needed(chart: &mut RoxChart) {
    let Some(first_note_time) = chart.notes.iter().map(|n| n.time_us).min() else {
        return;
    };
    let Some(tp) = chart.timing_points.iter_mut().find(|tp| tp.is_bpm()) else {
        return;
    };
    if let TimingPoint::Bpm { time_us, .. } = tp {
        if *time_us <= first_note_time { return; }
        tracing::warn!(
            bpm_time_us = *time_us,
            note_time_us = first_note_time,
            "BPM timing point is after first note — re-arranging to {}µs (re_arrange_bpm mode)",
            first_note_time - 1,
        );
        *time_us = first_note_time - 1;
    }
}

pub(crate) fn from_beatmap(beatmap: &super::types::OsuBeatmap) -> RoxChart {
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // value is non-negative in valid input; key count fits in u8
    let key_count = beatmap.difficulty.circle_size as u8;
    let mut chart = RoxChart::new(key_count);
    chart.metadata = build_metadata(beatmap);
    build_timing_points(beatmap, &mut chart);
    build_notes(beatmap, key_count, &mut chart);
    chart
}

fn build_metadata(beatmap: &super::types::OsuBeatmap) -> Metadata {
    let title = beatmap.metadata.title_unicode.clone()
        .unwrap_or_else(|| beatmap.metadata.title.clone());
    let artist = beatmap.metadata.artist_unicode.clone()
        .unwrap_or_else(|| beatmap.metadata.artist.clone());
    Metadata {
        #[allow(clippy::cast_sign_loss)] // value is non-negative in valid input
        chart_id: beatmap.metadata.beatmap_id.map(|id| id as u64),
        #[allow(clippy::cast_sign_loss)] // value is non-negative in valid input
        chartset_id: beatmap.metadata.beatmap_set_id.map(|id| id as u64),
        title: title.into(),
        artist: artist.into(),
        creator: beatmap.metadata.creator.clone().into(),
        difficulty_name: beatmap.metadata.version.clone().into(),
        difficulty_value: Some(beatmap.difficulty.overall_difficulty),
        audio_file: beatmap.general.audio_filename.clone().into(),
        background_file: beatmap.background.clone().map(Into::into),
        audio_offset_us: i64::from(beatmap.general.audio_lead_in) * 1000,
        preview_time_us: i64::from(beatmap.general.preview_time).max(0) * 1000,
        source: beatmap.metadata.source.clone().map(Into::into),
        tags: beatmap.metadata.tags.iter().map(|s| s.clone().into()).collect(),
        ..Metadata::default()
    }
}

fn build_timing_points(beatmap: &super::types::OsuBeatmap, chart: &mut RoxChart) {
    for tp in &beatmap.timing_points {
        #[allow(clippy::cast_possible_truncation)] // ms→µs i64: safe for any realistic timestamp or duration
        let time_us = (tp.time * 1000.0) as i64;
        if tp.uninherited {
            if let Some(bpm) = tp.bpm() {
                chart.timing_points.push(TimingPoint::Bpm {
                    time_us, bpm, signature: tp.meter,
                });
            }
        } else {
            chart.timing_points.push(TimingPoint::sv(time_us, tp.scroll_velocity()));
        }
    }
}

fn build_notes(beatmap: &super::types::OsuBeatmap, key_count: u8, chart: &mut RoxChart) {
    let mut hitsound_map: BTreeMap<String, u16> = BTreeMap::new();
    for ho in &beatmap.hit_objects {
        let column = ho.column(key_count);
        let time_us = i64::from(ho.time) * 1000;
        let mut note = if ho.is_hold() {
            Note::hold(time_us, i64::from(ho.duration_ms()) * 1000, column)
        } else {
            Note::tap(time_us, column)
        };
        assign_hitsound(ho, &mut note, chart, &mut hitsound_map);
        chart.notes.push(note);
    }
    chart.notes.sort_by_key(|n| n.time_us);
}

fn assign_hitsound(
    ho: &super::types::OsuHitObject,
    note: &mut Note,
    chart: &mut RoxChart,
    map: &mut BTreeMap<String, u16>,
) {
    if ho.extras.is_empty() { return; }
    let parts: Vec<&str> = ho.extras.split(':').collect();
    let filename_idx = if ho.is_hold() { 5 } else { 4 };
    let Some(&filename) = parts.get(filename_idx) else { return };
    let filename = filename.trim();
    if filename.is_empty() { return; }
    let idx = if let Some(&idx) = map.get(filename) {
        idx
    } else {
        let volume_idx = if ho.is_hold() { 4 } else { 3 };
        let volume: Option<u8> = parts.get(volume_idx)
            .and_then(|v| v.parse().ok())
            .filter(|&v: &u8| v > 0);
        let hs = match volume {
            Some(vol) => Hitsound::with_volume(filename, vol),
            None => Hitsound::new(filename),
        };
        #[allow(clippy::cast_possible_truncation)] // hitsound index fits in u16 for any realistic chart
        let idx = chart.hitsounds.len() as u16;
        chart.hitsounds.push(hs);
        map.insert(filename.to_string(), idx);
        idx
    };
    note.hitsound_index = Some(idx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::codec::Decoder;
    use rstest::rstest;

    #[rstest]
    fn test_decode_7k() {
        let data = rox_test_utils::get_test_asset("osu/mania_7k.osu");
        let chart = OsuDecoder::decode(&data).expect("decode failed");
        assert_eq!(chart.key_count, 7);
        assert!(!chart.notes.is_empty());
        assert!(!chart.timing_points.is_empty());
        assert_eq!(chart.metadata.difficulty_name, "7K Awakened");
    }

    #[rstest]
    fn test_decode_rejects_non_mania() {
        let data = b"osu file format v14\n\n[General]\nMode: 1\n\n[Difficulty]\nCircleSize:4\n";
        assert!(OsuDecoder::decode(data).is_err());
    }

    #[rstest]
    fn test_decode_hitsounds() {
        let data = rox_test_utils::get_test_asset("osu/mania_hitsound.osu");
        let chart = OsuDecoder::decode(&data).expect("decode failed");
        assert_eq!(chart.key_count, 4);
        assert_eq!(chart.hitsounds.len(), 4);
    }

    #[rstest]
    fn test_decode_50k_fails_without_re_arrange() {
        let data = rox_test_utils::get_test_asset("osu/mania_4K_50K_notes.osu");
        assert!(OsuDecoder::decode(&data).is_err());
    }

    #[rstest]
    fn test_decode_50k_succeeds_with_re_arrange() {
        let data = rox_test_utils::get_test_asset("osu/mania_4K_50K_notes.osu");
        let opts = OsuDecodeOptions { re_arrange_bpm: true };
        let chart = OsuDecoder::decode_with_options(&data, &opts).expect("decode failed");
        assert_eq!(chart.key_count, 4);
        assert!(!chart.notes.is_empty());
        // first BPM must be <= first note
        let first_note = chart.notes.iter().map(|n| n.time_us).min().unwrap();
        let first_bpm = chart.timing_points.iter().find(|tp| tp.is_bpm())
            .map(|tp| tp.time_us()).unwrap();
        assert!(first_bpm <= first_note);
    }
}
