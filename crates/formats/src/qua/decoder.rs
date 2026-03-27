#![warn(clippy::pedantic)]
use rox::codec::Decoder;
use rox::error::RoxResult;
use rox::model::{Metadata, Note, RoxChart, TimingPoint};
use rox_macros::Format;

use super::{parser, types::QuaChart};

#[derive(Format)]
#[format(extensions = ["qua"])]
pub struct QuaDecoder;

impl Decoder for QuaDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        let qua = parser::parse(data)?;
        Ok(from_qua(&qua))
    }
}

#[must_use]
pub fn from_qua(qua: &QuaChart) -> RoxChart {
    let key_count = qua.mode.key_count();
    let mut chart = RoxChart::new(key_count);
    chart.metadata = build_metadata(qua);
    build_timing_points(qua, &mut chart);
    build_notes(qua, &mut chart);
    chart.timing_points.sort_by_key(rox::model::TimingPoint::time_us);
    chart
}

fn build_metadata(qua: &QuaChart) -> Metadata {
    Metadata {
        #[allow(clippy::cast_sign_loss)]
        chart_id: if qua.map_id > 0 {
            Some(qua.map_id as u64)
        } else {
            None
        },
        #[allow(clippy::cast_sign_loss)]
        chartset_id: if qua.map_set_id > 0 {
            Some(qua.map_set_id as u64)
        } else {
            None
        },
        title: qua.title.clone().into(),
        artist: qua.artist.clone().into(),
        creator: qua.creator.clone().into(),
        difficulty_name: qua.difficulty_name.clone().into(),
        audio_file: qua.audio_file.clone().into(),
        background_file: qua.background_file.clone().map(Into::into),
        preview_time_us: i64::from(qua.preview_time) * 1000,
        source: qua.source.clone().map(Into::into),
        tags: qua
            .tags
            .as_deref()
            .unwrap_or("")
            .split_whitespace()
            .map(Into::into)
            .collect(),
        ..Metadata::default()
    }
}

fn build_timing_points(qua: &QuaChart, chart: &mut RoxChart) {
    for tp in &qua.timing_points {
        #[allow(clippy::cast_possible_truncation)]
        let time_us = (tp.start_time * 1000.0) as i64;
        let sig = tp
            .signature
            .as_ref()
            .map_or(4, super::types::TimeSignature::beats);
        chart
            .timing_points
            .push(TimingPoint::Bpm { time_us, bpm: tp.bpm, signature: sig });
    }
    for sv in &qua.slider_velocities {
        #[allow(clippy::cast_possible_truncation)]
        let time_us = (sv.start_time * 1000.0) as i64;
        #[allow(clippy::cast_possible_truncation)]
        chart
            .timing_points
            .push(TimingPoint::sv(time_us, sv.multiplier as f32));
    }
}

fn build_notes(qua: &QuaChart, chart: &mut RoxChart) {
    for ho in &qua.hit_objects {
        #[allow(clippy::cast_possible_truncation)]
        let time_us = (ho.start_time * 1000.0) as i64;
        let column = ho.lane.saturating_sub(1); // Quaver lanes are 1-indexed
        let note = if let Some(end) = ho.end_time {
            #[allow(clippy::cast_possible_truncation)]
            let end_us = (end * 1000.0) as i64;
            Note::hold(time_us, end_us - time_us, column)
        } else {
            Note::tap(time_us, column)
        };
        chart.notes.push(note);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::codec::{Decoder, Encoder};
    use rstest::rstest;

    #[rstest]
    #[case("quaver/4K.qua", 4)]
    #[case("quaver/7K.qua", 7)]
    fn test_decode_asset(#[case] asset: &str, #[case] expected_keys: u8) {
        let data = rox_test_utils::get_test_asset(asset);
        let chart = QuaDecoder::decode(&data).expect("decode failed");
        assert_eq!(chart.key_count, expected_keys);
        assert!(!chart.notes.is_empty());
        assert!(!chart.timing_points.is_empty());
    }

    #[rstest]
    fn test_roundtrip() {
        let data = rox_test_utils::get_test_asset("quaver/4K.qua");
        let chart1 = QuaDecoder::decode(&data).expect("decode failed");
        let encoded =
            super::super::encoder::QuaEncoder::encode(&chart1).expect("encode failed");
        let chart2 = QuaDecoder::decode(&encoded).expect("decode 2 failed");
        assert_eq!(chart1.key_count, chart2.key_count);
        assert_eq!(chart1.notes.len(), chart2.notes.len());
    }
}
