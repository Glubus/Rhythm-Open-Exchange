#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{format, vec::Vec};

use rox::codec::Decoder;
use rox::error::RoxResult;
use rox::model::{Metadata, Note, RoxChart, TimingPoint};
use rox_macros::Format;

use super::types::{AlternationState, TaikoBeatmap};
use crate::osu::parser;

/// Decoder for osu!taiko beatmaps. Converts to 4K layout.
#[derive(Format)]
#[format(extensions = ["osu"])]
pub struct TaikoDecoder;

impl TaikoDecoder {
    /// Decode with custom alternation state (for testing).
    /// # Errors
    /// Returns an error if parsing fails.
    pub fn decode_with_state(data: &[u8], state: &mut AlternationState) -> RoxResult<RoxChart> {
        let beatmap = parse_taiko(data)?;
        Ok(build_chart(&beatmap, state))
    }
}

impl Decoder for TaikoDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        let mut state = AlternationState::default();
        Self::decode_with_state(data, &mut state)
    }
}

fn parse_taiko(data: &[u8]) -> RoxResult<TaikoBeatmap> {
    let osu_bm = parser::parse(data)?;
    if osu_bm.general.mode != 1 {
        return Err(rox::error::RoxError::InvalidFormat(
            format!("Not a taiko beatmap (mode={}, expected 1)", osu_bm.general.mode)
        ));
    }
    let hit_objects = osu_bm.hit_objects.iter().map(|ho| {
        use super::types::{TaikoHitObject, TaikoHitsound};
        TaikoHitObject {
            #[allow(clippy::cast_precision_loss)] // i64→f64: precision loss acceptable for timing values
            time_ms: f64::from(ho.time),
            hitsound: TaikoHitsound::from_bits_truncate(u32::from(ho.hit_sound)),
            object_type: u32::from(ho.object_type),
        }
    }).collect();
    Ok(TaikoBeatmap {
        format_version: osu_bm.format_version,
        general: osu_bm.general,
        metadata: osu_bm.metadata,
        difficulty: osu_bm.difficulty,
        background: osu_bm.background,
        timing_points: osu_bm.timing_points,
        hit_objects,
    })
}

fn build_chart(beatmap: &TaikoBeatmap, state: &mut AlternationState) -> RoxChart {
    let mut chart = RoxChart::new(4);
    chart.metadata = build_metadata(beatmap);
    build_timing_points(beatmap, &mut chart);
    build_notes(beatmap, state, &mut chart);
    chart
}

fn build_metadata(beatmap: &TaikoBeatmap) -> Metadata {
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
        audio_file: beatmap.general.audio_filename.clone().into(),
        background_file: beatmap.background.clone().map(Into::into),
        audio_offset_us: i64::from(beatmap.general.audio_lead_in) * 1000,
        preview_time_us: i64::from(beatmap.general.preview_time).max(0) * 1000,
        ..Metadata::default()
    }
}

fn build_timing_points(beatmap: &TaikoBeatmap, chart: &mut RoxChart) {
    for tp in &beatmap.timing_points {
        if tp.uninherited && let Some(bpm) = tp.bpm() {
            #[allow(clippy::cast_possible_truncation)] // ms→µs i64: safe for any realistic timestamp or duration
            let time_us = (tp.time * 1000.0) as i64;
            chart.timing_points.push(TimingPoint::Bpm { time_us, bpm, signature: tp.meter });
        }
    }
    if chart.timing_points.is_empty() {
        chart.timing_points.push(TimingPoint::bpm(0, 120.0));
    }
}

fn build_notes(beatmap: &TaikoBeatmap, state: &mut AlternationState, chart: &mut RoxChart) {
    for ho in &beatmap.hit_objects {
        if (ho.object_type & 8) != 0 { continue; } // skip spinners
        #[allow(clippy::cast_possible_truncation)] // ms→µs i64: safe for any realistic timestamp or duration
        let time_us = (ho.time_ms * 1000.0) as i64;
        let is_big = ho.hitsound.is_big();
        let columns = if ho.hitsound.is_kat() {
            state.next_kat_columns(is_big)
        } else {
            state.next_don_columns(is_big)
        };
        for col in columns {
            chart.notes.push(Note::tap(time_us, col));
        }
    }
    chart.notes.sort_by_key(|n| n.time_us);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    use super::super::types::{AlternationState, ColumnLayout};

    #[rstest]
    #[case(ColumnLayout::Dkkd)]
    #[case(ColumnLayout::Dkdk)]
    fn test_decode_asset(#[case] layout: ColumnLayout) {
        let data = rox_test_utils::get_test_asset("osu/taiko.osu");
        let mut state = AlternationState::new(layout);
        let chart = TaikoDecoder::decode_with_state(&data, &mut state).expect("decode failed");
        assert_eq!(chart.key_count, 4);
        assert!(!chart.notes.is_empty());
        assert!(!chart.timing_points.is_empty());
    }
}
