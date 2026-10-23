#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use rox::codec::Decoder;
use rox::error::RoxResult;
use rox::model::{Metadata, Note, RoxChart, TimingPoint};

use super::parser;
use super::types::{SmChart, SmFile, SmNoteType};

pub struct SmDecoder;
crate::impl_format!(SmDecoder, ["sm"]);

impl Decoder for SmDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        super::ssc_decoder::first_chart(&parser::parse(data)?)
    }
}

/// The ROX chart of one playable chart of a parsed `.sm`/`.ssc` file.
///
/// The times of a ROX chart are measured from the start of its audio, while the beats of a
/// simfile are measured from beat 0, which `#OFFSET` places at `-offset` seconds of the audio
/// (`#OFFSET:-1.5` means the first beat sounds 1.5 s into the song). Notes and BPM changes are
/// moved by that, so `metadata.audio_offset_us` is 0: nothing is left for a consumer to apply.
#[must_use]
pub fn from_chart(sm: &SmFile, chart: &SmChart) -> RoxChart {
    let shift_us = -chart.offset_us.unwrap_or(sm.offset_us);
    let mut rox = RoxChart::new(chart.column_count);
    rox.metadata = build_metadata(sm, chart);
    for (time_us, bpm) in chart.bpms.as_ref().unwrap_or(&sm.bpms) {
        rox.timing_points
            .push(TimingPoint::bpm(time_us + shift_us, *bpm));
    }
    convert_notes(chart, shift_us, &mut rox);
    rox.notes.sort_by_key(|n| n.time_us);
    rox
}

fn build_metadata(sm: &SmFile, chart: &SmChart) -> Metadata {
    Metadata {
        title: sm.metadata.title.clone().into(),
        artist: sm.metadata.artist.clone().into(),
        creator: chart
            .credit
            .as_deref()
            .filter(|credit| !credit.is_empty())
            .unwrap_or(&sm.metadata.credit)
            .into(),
        difficulty_name: chart.label().into(),
        #[allow(clippy::cast_precision_loss)] // i64→f64: precision loss acceptable for timing values
        difficulty_value: Some(chart.meter as f32),
        audio_file: sm.metadata.music.clone().into(),
        background_file: if sm.metadata.background.is_empty() {
            None } else { Some(sm.metadata.background.clone().into()) },
        // The offset is applied to the times themselves, see `from_chart`.
        audio_offset_us: 0,
        #[allow(clippy::cast_possible_truncation)] // ms→µs i64: safe for any realistic timestamp or duration
        preview_time_us: (sm.metadata.sample_start * 1_000_000.0) as i64,
        #[allow(clippy::cast_possible_truncation)] // ms→µs i64: safe for any realistic timestamp or duration
        preview_duration_us: (sm.metadata.sample_length * 1_000_000.0) as i64,
        ..Metadata::default()
    }
}

fn convert_notes(chart: &SmChart, shift_us: i64, rox: &mut RoxChart) {
    let mut sorted = chart.notes.clone();
    sorted.sort_by(|a, b| a.time_us.cmp(&b.time_us).then(a.column.cmp(&b.column)));
    for note in &mut sorted {
        note.time_us += shift_us;
    }
    let mut pending_holds: Vec<(i64, u8)> = Vec::new();
    let mut pending_rolls: Vec<(i64, u8)> = Vec::new();
    for note in &sorted {
        match note.note_type {
            #[allow(clippy::match_same_arms)]
            // Tap and Lift both map to tap in the ROX model; kept separate for forward-compatibility
            SmNoteType::Tap | SmNoteType::Lift => {
                rox.notes.push(Note::tap(note.time_us, note.column))
            }
            SmNoteType::HoldHead => pending_holds.push((note.time_us, note.column)),
            SmNoteType::RollHead => pending_rolls.push((note.time_us, note.column)),
            SmNoteType::Tail => resolve_tail(
                note.time_us,
                note.column,
                &mut pending_holds,
                &mut pending_rolls,
                rox,
            ),
            SmNoteType::Mine => rox.notes.push(Note::mine(note.time_us, note.column)),
            SmNoteType::Empty | SmNoteType::Fake => {}
        }
    }
}

fn resolve_tail(
    time_us: i64,
    col: u8,
    holds: &mut Vec<(i64, u8)>,
    rolls: &mut Vec<(i64, u8)>,
    rox: &mut RoxChart,
) {
    if let Some(idx) = holds.iter().position(|(_, c)| *c == col) {
        let (start, column) = holds.remove(idx);
        rox.notes.push(Note::hold(start, time_us - start, column));
    } else if let Some(idx) = rolls.iter().position(|(_, c)| *c == col) {
        let (start, column) = rolls.remove(idx);
        rox.notes.push(Note::burst(start, time_us - start, column));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::codec::Decoder;
    use rstest::rstest;

    #[rstest]
    fn test_decode_4k_asset() {
        let data = rox_test_utils::get_test_asset("stepmania/4k.sm");
        let chart = SmDecoder::decode(&data).expect("decode failed");
        assert_eq!(chart.key_count, 4);
        assert!(!chart.notes.is_empty());
        assert!(!chart.timing_points.is_empty());
    }

    #[rstest]
    fn test_decode_no_charts_error() {
        let data = b"#TITLE:Empty;\n#BPMS:0=120;\n";
        assert!(SmDecoder::decode(data).is_err());
    }
}
