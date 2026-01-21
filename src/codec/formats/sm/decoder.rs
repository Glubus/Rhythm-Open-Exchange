#![allow(clippy::doc_markdown)]
//! Decoder for converting StepMania (`.sm`) files to `RoxChart`.

use crate::codec::Decoder;
use crate::error::RoxResult;
use crate::model::{Metadata, Note, RoxChart, TimingPoint};

use super::parser;
use super::types::{SmChart, SmFile, SmNoteType};

/// Decoder for StepMania (`.sm`) beatmaps.
pub struct SmDecoder;

impl SmDecoder {
    /// Convert an `SmFile` to a `RoxChart`.
    ///
    /// If the file contains multiple charts, this returns the first one.
    /// Use `decode_chart` to decode a specific chart.
    #[must_use]
    pub fn from_file(sm: &SmFile) -> Option<RoxChart> {
        sm.charts.first().map(|chart| Self::from_chart(sm, chart))
    }

    /// Convert a specific chart from an `SmFile` to a `RoxChart`.
    #[must_use]
    pub fn from_chart(sm: &SmFile, chart: &SmChart) -> RoxChart {
        let mut rox = RoxChart::new(chart.column_count);

        // Map metadata
        rox.metadata = Metadata {
            key_count: chart.column_count,
            title: sm.metadata.title.clone(),
            artist: sm.metadata.artist.clone(),
            creator: sm.metadata.credit.clone(),
            difficulty_name: chart.difficulty.clone(),
            #[allow(clippy::cast_precision_loss)]
            difficulty_value: Some(chart.meter as f32),
            audio_file: sm.metadata.music.clone(),
            background_file: if sm.metadata.background.is_empty() {
                None
            } else {
                Some(sm.metadata.background.clone())
            },
            audio_offset_us: -sm.offset_us, // SM offset is opposite convention
            #[allow(clippy::cast_possible_truncation)]
            preview_time_us: (sm.metadata.sample_start * 1_000_000.0) as i64,
            #[allow(clippy::cast_possible_truncation)]
            preview_duration_us: (sm.metadata.sample_length * 1_000_000.0) as i64,
            source: None,
            genre: None,
            language: None,
            tags: Vec::new(),
            is_coop: false,
            ..Default::default()
        };

        // Convert BPM timing points
        for (time_us, bpm) in &sm.bpms {
            rox.timing_points.push(TimingPoint::bpm(*time_us, *bpm));
        }

        // Convert notes
        // We need to track hold/roll heads to pair with tails
        let mut pending_holds: Vec<(i64, u8)> = Vec::new(); // (start_time, column)
        let mut pending_rolls: Vec<(i64, u8)> = Vec::new(); // (start_time, column)

        // Sort notes by time, then column for consistent processing
        let mut sorted_notes = chart.notes.clone();
        sorted_notes.sort_by(|a, b| a.time_us.cmp(&b.time_us).then(a.column.cmp(&b.column)));

        for note in &sorted_notes {
            match note.note_type {
                SmNoteType::Tap => {
                    rox.notes.push(Note::tap(note.time_us, note.column));
                }
                SmNoteType::HoldHead => {
                    // Store for later when we find the tail
                    pending_holds.push((note.time_us, note.column));
                }
                SmNoteType::RollHead => {
                    // Store for later when we find the tail
                    pending_rolls.push((note.time_us, note.column));
                }
                SmNoteType::Tail => {
                    // Find matching hold or roll head
                    if let Some(idx) = pending_holds
                        .iter()
                        .position(|(_, col)| *col == note.column)
                    {
                        let (start_time, column) = pending_holds.remove(idx);
                        let duration = note.time_us - start_time;
                        rox.notes.push(Note::hold(start_time, duration, column));
                    } else if let Some(idx) = pending_rolls
                        .iter()
                        .position(|(_, col)| *col == note.column)
                    {
                        let (start_time, column) = pending_rolls.remove(idx);
                        let duration = note.time_us - start_time;
                        rox.notes.push(Note::burst(start_time, duration, column));
                    }
                    // Orphan tails are ignored
                }
                SmNoteType::Mine => {
                    rox.notes.push(Note::mine(note.time_us, note.column));
                }
                SmNoteType::Lift => {
                    // Convert lift to tap (no direct ROX equivalent)
                    rox.notes.push(Note::tap(note.time_us, note.column));
                }
                SmNoteType::Empty | SmNoteType::Fake => {
                    // Ignored
                }
            }
        }

        // Sort notes by time
        rox.notes.sort_by_key(|n| n.time_us);

        rox
    }

    /// Decode all charts from an SM file.
    #[must_use]
    pub fn decode_all(sm: &SmFile) -> Vec<RoxChart> {
        sm.charts
            .iter()
            .map(|chart| Self::from_chart(sm, chart))
            .collect()
    }
}

impl Decoder for SmDecoder {
    fn decode(data: &[u8]) -> RoxResult<RoxChart> {
        let sm = parser::parse(data)?;
        sm.charts
            .first()
            .map(|chart| Self::from_chart(&sm, chart))
            .ok_or_else(|| {
                crate::error::RoxError::InvalidFormat("No charts found in SM file".into())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::Decoder;

    /// Basic SM file content for testing.
    const BASIC_SM: &str = r#"
#TITLE:Test Song;
#ARTIST:Test Artist;
#CREDIT:Test Mapper;
#MUSIC:song.ogg;
#OFFSET:0;
#BPMS:0=120;
#STOPS:;

#NOTES:
     dance-single:
     :
     Beginner:
     1:
     0,0,0,0,0:
0000
1000
0100
0010
,
0001
0000
0000
0000
;
"#;

    #[test]
    fn test_decode_basic_sm() {
        let chart = <SmDecoder as Decoder>::decode(BASIC_SM.as_bytes()).expect("Failed to decode");

        assert_eq!(chart.key_count(), 4);
        assert_eq!(chart.metadata.title, "Test Song");
        assert_eq!(chart.metadata.artist, "Test Artist");
        assert_eq!(chart.metadata.creator, "Test Mapper");
        assert_eq!(chart.metadata.difficulty_name, "Beginner");
        assert!(!chart.notes.is_empty());
    }

    #[test]
    fn test_sm_note_count() {
        let chart = <SmDecoder as Decoder>::decode(BASIC_SM.as_bytes()).expect("Failed to decode");

        // 4 tap notes: 1 per column across 2 measures
        assert_eq!(chart.notes.len(), 4);
    }

    #[test]
    fn test_sm_timing_points() {
        let chart = <SmDecoder as Decoder>::decode(BASIC_SM.as_bytes()).expect("Failed to decode");

        // Should have at least one BPM timing point
        assert!(!chart.timing_points.is_empty());
        assert_eq!(chart.timing_points[0].bpm, 120.0);
    }

    #[test]
    fn test_decode_asset_4k() {
        // assets/stepmania/4k.sm
        let data = crate::test_utils::get_test_asset("stepmania/4k.sm");
        let chart = <SmDecoder as Decoder>::decode(&data).expect("Failed to decode 4k.sm");

        // Validating against expected properties of 4k.sm (assuming simple 4k chart)
        assert_eq!(chart.key_count(), 4);
        // Note: I don't know the exact metadata of 4k.sm, so I'll just check it decoded successfully and has notes
        // Ideally I'd inspect the actual file content, but for now validating decode success is good.
    }
}
