//! Rhythm Master "imdjson" (`.json` with `"signature": "BNDQ"`): the JSON form of an `.imd` of the newer clients. Decode only.
//!
//! Format notes: `vsrg-format-docs/vsrg/rhythm-master/json`, from `RMEssentials` (`rmechart.cpp`: `fromJsonNote`, `toImdNote`; read for
//! facts). The reader turns the file into the `.imd` it stands for, the way `RMEssentials` converts it, and reads that with [`ImdDecoder`]:
//! every decision of the `.imd` reader (lanes from the file name, chains, holds) applies.
//!
//! - **Tempo**: one `tempo` (BPM); no tempo change exists in this format.
//! - **Notes**: per `tracks[].track` (3 to 8: lane `track - 3`), each note's time is `time` (ms, 1.2.3 and later) or `tick × 1250 / tempo`
//!   (48 ticks a beat). `attr` 0 is a tap; `attr` 3 or 4 with a `dur` is a hold segment (its length `time_dur` ms, else from the ticks)
//!   and without one a slide to `toTrack`; `isEnd` tells the `.imd` type apart (3: single slide `0x01` / hold `0x02` when it ends, chain
//!   start `0x61` / `0x62` otherwise; 4: chain end `0xA1` / `0xA2`, joint `0x21` / `0x22`).

use serde::Deserialize;

use rox::error::{RoxError, RoxResult};
use rox::model::RoxChart;

use super::{ImdDecoder, ImdReport};

/// Files above this size are refused (a chart is a few hundred kilobytes).
const MAX_FILE_SIZE: usize = 64 * 1024 * 1024;

#[derive(Deserialize)]
struct File {
    signature: String,
    tempo: f64,
    #[serde(default)]
    tracks: Vec<Track>,
}

#[derive(Deserialize)]
struct Track {
    track: i64,
    #[serde(default)]
    note: Vec<JsonNote>,
}

#[derive(Deserialize)]
struct JsonNote {
    #[serde(default)]
    tick: Option<f64>,
    #[serde(default)]
    time: Option<f64>,
    #[serde(default)]
    dur: f64,
    #[serde(default)]
    time_dur: Option<f64>,
    #[serde(default, rename = "isEnd")]
    is_end: i64,
    #[serde(default, rename = "toTrack")]
    to_track: i64,
    #[serde(default)]
    attr: i64,
}

fn invalid(reason: impl std::fmt::Display) -> RoxError {
    RoxError::InvalidFormat(format!("imdjson: {reason}"))
}

/// Whether the start of a file is an imdjson chart: a JSON object whose `signature` is `BNDQ`.
#[must_use]
pub fn looks_like_imdjson(head: &[u8]) -> bool {
    let text = String::from_utf8_lossy(head);
    let text = text.trim_start_matches('\u{feff}').trim_start();
    text.starts_with('{') && text.split_once("\"signature\"").is_some_and(|(_, rest)| rest.trim_start().strip_prefix(':').is_some_and(|value| value.trim_start().starts_with("\"BNDQ\"")))
}

/// Whether the file at `path` is an imdjson chart: a `.json` whose start [`looks_like_imdjson`]. The start of the file is read.
#[must_use]
pub fn is_imdjson_chart_file(path: &std::path::Path) -> bool {
    use std::io::Read;
    if !path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("json")) {
        return false;
    }
    let mut head = Vec::new();
    std::fs::File::open(path).and_then(|file| file.take(256).read_to_end(&mut head)).is_ok_and(|_| looks_like_imdjson(&head))
}

/// Decoder of imdjson charts.
pub struct ImdJsonDecoder;

impl ImdJsonDecoder {
    /// Decodes a chart read from a file named `file_name` (`<song>_<4|5|6>k_<ez|nm|hd>.imd.json` gives the lanes and the difficulty, as
    /// for an `.imd`) and says what the `.imd` reader dropped.
    ///
    /// # Errors
    /// Returns an error if the file is too large, not imdjson, has no valid tempo, or the `.imd` it stands for does not decode.
    pub fn decode_with_report(data: &[u8], file_name: &str) -> RoxResult<(RoxChart, ImdReport)> {
        let imd = to_imd(data)?;
        let name = file_name.strip_suffix(".json").or_else(|| file_name.strip_suffix(".JSON")).unwrap_or(file_name);
        ImdDecoder::decode_with_report(&imd, name)
    }
}

/// The `.imd` bytes a file stands for: one BPM entry, then the notes in the order of the file.
fn to_imd(data: &[u8]) -> RoxResult<Vec<u8>> {
    if data.len() > MAX_FILE_SIZE {
        return Err(invalid(format!("file too large: {} bytes", data.len())));
    }
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let file: File = serde_json::from_slice(data).map_err(|e| invalid(e))?;
    if file.signature != "BNDQ" {
        return Err(invalid(format!("signature {:?}", file.signature)));
    }
    if !(file.tempo.is_finite() && file.tempo > 0.0) {
        return Err(invalid(format!("tempo {}", file.tempo)));
    }
    let ms_of_tick = |tick: f64| tick * 1250.0 / file.tempo;
    let mut records = Vec::new();
    for track in &file.tracks {
        let Some(lane) = track.track.checked_sub(3).and_then(|lane| u8::try_from(lane).ok()) else { continue };
        for note in &track.note {
            let Some(time) = note.time.or_else(|| note.tick.map(ms_of_tick)).filter(|time| time.is_finite() && *time >= 0.0) else { continue };
            let length = if note.dur > 0.0 {
                note.time_dur.or_else(|| note.tick.map(|tick| ms_of_tick(tick + note.dur) - ms_of_tick(tick))).unwrap_or(0.0)
            } else {
                0.0
            };
            let end = note.is_end != 0;
            let slide = i32::try_from(note.to_track - track.track).unwrap_or(0);
            #[allow(clippy::cast_possible_truncation)] // a length in ms
            let (kind, action): (u16, i32) = match (note.attr, note.dur > 0.0) {
                (3, false) => (if end { 0x01 } else { 0x61 }, slide),
                (3, true) => (if end { 0x02 } else { 0x62 }, length.round() as i32),
                (4, false) => (if end { 0xA1 } else { 0x21 }, slide),
                (4, true) => (if end { 0xA2 } else { 0x22 }, length.round() as i32),
                _ => (0, 0),
            };
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // a time in ms, not negative
            records.push((kind, time.round().min(f64::from(u32::MAX)) as u32, lane, action));
        }
    }
    let mut imd = Vec::with_capacity(20 + 11 * records.len());
    imd.extend(0u32.to_le_bytes());
    imd.extend(1u32.to_le_bytes());
    imd.extend(0u32.to_le_bytes());
    imd.extend(file.tempo.to_le_bytes());
    imd.extend(0x0303u16.to_le_bytes());
    imd.extend(u32::try_from(records.len()).map_err(|_| invalid("too many notes"))?.to_le_bytes());
    for (kind, time, lane, action) in records {
        imd.extend(kind.to_le_bytes());
        imd.extend(time.to_le_bytes());
        imd.push(lane);
        imd.extend(action.to_le_bytes());
    }
    Ok(imd)
}
