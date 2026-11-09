#![warn(clippy::pedantic)]
use rox::codec::Decoder;
use rox::error::RoxResult;
use rox::model::{Metadata, Note, RoxChart, TimingPoint};

use super::{parser, types::QuaChart};

pub struct QuaDecoder;
crate::impl_format!(QuaDecoder, ["qua"]);

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
    chart
        .timing_points
        .sort_by_key(rox::model::TimingPoint::time_us);
    // Quaver times the notes before the first timing point with that point's BPM, so a map whose timing starts after its first
    // note plays: the first BPM point moves to the first note, as the osu! decoder does with `re_arrange_bpm`.
    let first_note = chart.notes.iter().map(|note| note.time_us).min();
    if let (Some(first_note), Some(TimingPoint::Bpm { time_us, .. })) = (
        first_note,
        chart
            .timing_points
            .iter_mut()
            .find(|point| matches!(point, TimingPoint::Bpm { .. })),
    ) {
        *time_us = (*time_us).min(first_note);
    }
    chart
}

fn build_metadata(qua: &QuaChart) -> Metadata {
    Metadata {
        #[allow(clippy::cast_sign_loss)] // value is non-negative in valid input
        chart_id: if qua.map_id > 0 {
            Some(qua.map_id as u64)
        } else {
            None
        },
        #[allow(clippy::cast_sign_loss)] // value is non-negative in valid input
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
        // ms→µs i64: safe for any realistic timestamp or duration
        let time_us = (tp.start_time * 1000.0) as i64;
        let sig = tp
            .signature
            .as_ref()
            .map_or(4, super::types::TimeSignature::beats);
        chart.timing_points.push(TimingPoint::Bpm {
            time_us,
            bpm: tp.bpm,
            signature: sig,
        });
    }
    // `InitialScrollVelocity` is the multiplier before the first slider velocity: one point from the start of the
    // map (or of the first slider velocity, when earlier) that the first entry replaces. Pushed first, so a sort that
    // keeps the order of equal times leaves an entry at that very time the winner.
    if qua.initial_scroll_velocity.is_finite()
        && (qua.initial_scroll_velocity - 1.0).abs() > f32::EPSILON
    {
        #[allow(clippy::cast_possible_truncation)]
        // ms→µs i64: safe for any realistic timestamp
        let first_us = qua
            .slider_velocities
            .iter()
            .map(|sv| (sv.start_time * 1000.0) as i64)
            .min()
            .unwrap_or(0)
            .min(0);
        chart
            .timing_points
            .push(TimingPoint::sv(first_us, qua.initial_scroll_velocity));
    }
    for sv in &qua.slider_velocities {
        #[allow(clippy::cast_possible_truncation)]
        // ms→µs i64: safe for any realistic timestamp or duration
        let time_us = (sv.start_time * 1000.0) as i64;
        #[allow(clippy::cast_possible_truncation)]
        // multiplier→f32: precision loss acceptable for SV values
        chart
            .timing_points
            .push(TimingPoint::sv(time_us, sv.multiplier as f32));
    }
}

/// The notes as Quaver plays them: a long note whose `EndTime` is not after its start (`0` is how Quaver writes "no end", and tools
/// leave shorter ones) is a tap; a note on a lane the mode does not have is dropped; a note that starts on another note of its lane, or
/// inside a long note of its lane, is dropped (the first one stays), so one odd object never refuses the whole map.
fn build_notes(qua: &QuaChart, chart: &mut RoxChart) {
    let keys = chart.key_count;
    let mut notes: Vec<Note> = Vec::with_capacity(qua.hit_objects.len());
    for ho in &qua.hit_objects {
        // Quaver lanes are 1-indexed.
        let Some(column) = ho.lane.checked_sub(1).filter(|column| *column < keys) else {
            continue;
        };
        #[allow(clippy::cast_possible_truncation)]
        // ms→µs i64: safe for any realistic timestamp or duration
        let time_us = (ho.start_time * 1000.0) as i64;
        #[allow(clippy::cast_possible_truncation)]
        let end_us = ho.end_time.map(|end| (end * 1000.0) as i64);
        notes.push(match end_us {
            Some(end_us) if end_us > time_us => Note::hold(time_us, end_us - time_us, column),
            _ => Note::tap(time_us, column),
        });
    }
    chart.notes = notes;
    chart.drop_overlapping_notes();
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

    /// The encoder writes the chart's own mode: a 7K map comes back as 7K (it used to be written `Keys4`).
    #[rstest]
    #[case("quaver/4K.qua")]
    #[case("quaver/7K.qua")]
    fn test_roundtrip(#[case] asset: &str) {
        let data = rox_test_utils::get_test_asset(asset);
        let chart1 = QuaDecoder::decode(&data).expect("decode failed");
        let encoded = super::super::encoder::QuaEncoder::encode(&chart1).expect("encode failed");
        let chart2 = QuaDecoder::decode(&encoded).expect("decode 2 failed");
        assert_eq!(chart1.key_count, chart2.key_count);
        assert_eq!(chart1.notes.len(), chart2.notes.len());
    }

    /// Modes beyond Quaver's two (`Keys10` from editors and forks) are read on their key count.
    #[test]
    fn any_keys_mode_is_read() {
        let qua = "AudioFile: a.mp3\nMode: Keys10\nTimingPoints:\n- Bpm: 120\nHitObjects:\n- StartTime: 0\n  Lane: 10\n";
        let chart = QuaDecoder::decode(qua.as_bytes()).expect("decode failed");
        assert_eq!((chart.key_count, chart.notes[0].column), (10, 9));
        assert!(QuaDecoder::decode(b"Mode: Keys0\nHitObjects: []\n").is_err());
    }

    fn chart(signature: &str, first_note_ms: i64) -> String {
        format!(
            "AudioFile: audio.mp3\nMode: Keys4\nTitle: T\nArtist: A\nCreator: C\nDifficultyName: D\nTimingPoints:\n- StartTime: 1000\n  Bpm: 120\n  Signature: {signature}\nHitObjects:\n- StartTime: {first_note_ms}\n  Lane: 1\n- StartTime: 2000\n  Lane: 2\n"
        )
    }

    /// Real maps write the signature by name or as a number (`1`, `0`, `99999` were seen in files of Quaver.API and of
    /// players): every one decodes, a usable number as that many beats, the others as four.
    #[rstest]
    #[case("Quadruple", 4)]
    #[case("Triple", 3)]
    #[case("5", 5)]
    #[case("1", 1)]
    #[case("0", 4)]
    #[case("99999", 4)]
    fn signatures_by_name_or_number_decode(#[case] signature: &str, #[case] beats: u8) {
        let chart = QuaDecoder::decode(chart(signature, 1000).as_bytes()).expect("decode failed");
        assert!(matches!(chart.timing_points[0], TimingPoint::Bpm { signature, .. } if signature == beats), "{:?}", chart.timing_points);
    }

    /// A first note before the first timing point is timed with that point, as Quaver plays it: the point moves to the note.
    #[test]
    fn a_note_before_the_first_timing_point_takes_its_bpm() {
        let chart = QuaDecoder::decode(chart("Quadruple", 500).as_bytes()).expect("decode failed");
        assert_eq!(chart.timing_points[0].time_us(), 500_000);
        assert_eq!(chart.notes[0].time_us, 500_000);
    }

    /// Objects real maps carry (Quaver.API's test maps, editor leftovers): `EndTime: 0` or an end not after the start is a tap, a lane
    /// the mode does not have is dropped, a note on another one or inside a long note of its lane is dropped, the BOM written twice is
    /// skipped. The map is read, never refused for one of them.
    #[test]
    fn odd_objects_are_read_as_quaver_plays_them() {
        let qua = "\u{feff}\u{feff}AudioFile: a.mp3\nMode: Keys4\nTimingPoints:\n- Bpm: 120\nHitObjects:\n\
            - StartTime: 0\n  Lane: 1\n  EndTime: 0\n\
            - StartTime: 100\n  Lane: 2\n  EndTime: 50\n\
            - StartTime: 100\n  Lane: 2\n\
            - StartTime: 200\n  Lane: 3\n  EndTime: 600\n\
            - StartTime: 300\n  Lane: 3\n\
            - StartTime: 600\n  Lane: 3\n\
            - StartTime: 700\n  Lane: 9\n";
        let chart = QuaDecoder::decode(qua.as_bytes()).expect("decode failed");
        let notes: Vec<(i64, u8, i64)> = chart.notes.iter().map(|note| (note.time_us, note.column, note.duration_us())).collect();
        assert_eq!(notes, [(0, 0, 0), (100_000, 1, 0), (200_000, 2, 400_000), (600_000, 2, 0)]);
    }
}
