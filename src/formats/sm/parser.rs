#![warn(clippy::pedantic)]
//! `.sm` (StepMania 3/4/5) parser. A `.sm` file is an `.ssc` file whose charts are `#NOTES:` tags
//! of six `:`-separated parameters (step type, description, difficulty, meter, radar values,
//! measures) and whose timing is the song's alone, so it goes through the same pipeline as
//! [`super::ssc_parser`]: the same tokenizer, the same [`super::timing_data::TimingData`]
//! (BPM changes, stops, delays, warps, fakes) and the same refusals (see there). Without that,
//! every note after the first `#STOPS` of a file would be early.
#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use rox::error::{RoxError, RoxResult};

use super::msd::{Tag, tokenize};
use super::ssc_parser::{SongTiming, chart, metadata, offset_us};
use super::types::SmFile;

const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Parse an SM file from raw bytes. Charts that can not be played (step type this crate does not
/// read, rows of the wrong width, timing it can not honour) are left out and described in
/// [`SmFile::skipped`].
///
/// # Errors
/// Returns an error if data exceeds 100MB or is invalid UTF-8.
pub fn parse(data: &[u8]) -> RoxResult<SmFile> {
    if data.len() > MAX_FILE_SIZE {
        return Err(RoxError::InvalidFormat(format!(
            "File too large: {} bytes (max 100MB)",
            data.len()
        )));
    }
    let content = core::str::from_utf8(data)
        .map_err(|e| RoxError::InvalidFormat(format!("Invalid UTF-8: {e}")))?;
    let tags = tokenize(content);
    let mut sm = SmFile {
        metadata: metadata(&tags),
        ..SmFile::default()
    };
    sm.offset_us = offset_us(&tags).unwrap_or(0);
    let song_timing = SongTiming::of(&tags);
    sm.bpms = song_timing.bpm_points();
    for (index, notes) in tags.iter().filter(|tag| tag.name == "NOTES").enumerate() {
        match chart_tags(notes)
            .ok_or_else(|| "malformed #NOTES (expected 6 parameters)".to_string())
            .and_then(|tags| chart(&tags, &song_timing))
        {
            Ok(chart) => sm.charts.push(chart),
            Err(reason) => sm.skipped.push(format!("chart {}: {reason}", index + 1)),
        }
    }
    Ok(sm)
}

/// The `#STEPSTYPE`, `#DESCRIPTION`, `#DIFFICULTY`, `#METER` and `#NOTES` tags a `.ssc` chart
/// would have, from the parameters of a `.sm` `#NOTES:` tag.
fn chart_tags(notes: &Tag) -> Option<Vec<Tag>> {
    let [
        stepstype,
        description,
        difficulty,
        meter,
        _radar,
        measures @ ..,
    ] = notes.params.as_slice()
    else {
        return None;
    };
    let tag = |name: &str, params: Vec<String>| Tag {
        name: name.to_string(),
        params,
    };
    Some(vec![
        tag("STEPSTYPE", vec![stepstype.clone()]),
        tag("DESCRIPTION", vec![description.clone()]),
        tag("DIFFICULTY", vec![difficulty.clone()]),
        tag("METER", vec![meter.clone()]),
        tag("NOTES", measures.to_vec()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASIC_SM: &[u8] = b"
#TITLE:Test Song;
#ARTIST:Test Artist;
#CREDIT:Mapper;
#MUSIC:song.ogg;
#OFFSET:0;
#BPMS:0=120;
#STOPS:;

#NOTES:
     dance-single:
     :
     Hard:
     8:
     0,0,0,0,0:
0000
1000
0100
0010
,
0001
0000
1000
0100
;
";

    #[test]
    fn test_parse_basic_sm() {
        let sm = parse(BASIC_SM).expect("parse failed");
        assert_eq!(sm.metadata.title, "Test Song");
        assert_eq!(sm.metadata.artist, "Test Artist");
        assert!(!sm.bpms.is_empty());
        assert!(!sm.charts.is_empty());
        assert_eq!(sm.charts[0].column_count, 4);
        assert!(!sm.charts[0].notes.is_empty());
    }

    #[test]
    fn test_parse_4k_asset() {
        let data = rox_test_utils::get_test_asset("stepmania/4k.sm");
        let sm = parse(&data).expect("parse failed");
        assert!(!sm.charts.is_empty());
        assert_eq!(sm.charts[0].column_count, 4);
    }

    #[test]
    fn test_file_too_large() {
        let big = vec![0u8; 100 * 1024 * 1024 + 1];
        assert!(parse(&big).is_err());
    }
}
