#![warn(clippy::pedantic)]
//! Interlude (YAVSRG) charts: the "headless" binary chart of its database and CDN, and the legacy `.yav` file around it. Decode only.
//!
//! Format notes: `vsrg-format-docs/vsrg/interlude/yav`, from Prelude's source (YAVSRG, MIT, read for facts: `Chart.fs`, `NoteRow.fs`,
//! `TimeArray.fs`, `ImportChart.fs`), checked on the corpus CDN charts.
//!
//! - **Headless chart** ([`YavDecoder::decode_headless`]): little-endian, three time arrays (`i32` count, then per item an `f32` time in
//!   ms and its payload): note rows (`u16` column mask, then one byte per set bit, low bit first: 1 tap, 2 hold head, 3 hold body, 4 hold
//!   tail), BPM points (`i32` beats per measure, `f32` ms per beat), scroll velocities (`f32` multiplier). The key count is not in it:
//!   the caller gives it (the database row has it); a note beyond it refuses the chart, as the game's reader fails.
//! - **Holds**: a head opens a hold that its column's tail closes; the bodies between are redundant. A tail or body without a head and a
//!   head never closed are dropped and counted.
//! - **`.yav` file** (the game reads them no more; the format is its last reader's, `ImportChart.legacy_from_yav_file`): a key-count
//!   byte, a .NET string (7-bit length prefix, UTF-8) holding the JSON header, then the headless chart. From the header: `Title`,
//!   `Artist`, `Creator`, `DiffName`, `PreviewTime`, `Tags`, and `AudioFile`/`BackgroundFile` when they are plain names (the encoding of
//!   their other cases is not known).

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, Note, RoxChart, TimingPoint};
use serde::Deserialize;
use serde_json::Value;

/// Decoder of Interlude charts: a `.yav` file, or a headless chart with its key count ([`YavDecoder::decode_headless`]).
pub struct YavDecoder;
crate::impl_format!(YavDecoder, ["yav"]);

/// Files above this size are refused (a chart is a few hundred kilobytes).
const MAX_FILE_SIZE: usize = 64 * 1024 * 1024;
/// Most columns a mask can hold.
const MAX_KEYS: u8 = 16;
/// Times further than this many milliseconds from the start are not read.
const MAX_MS: f32 = 1e10;

/// What the decoder dropped. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct YavReport {
    /// Hold tails and bodies without a head, heads never closed, and notes of an unknown type: dropped.
    pub broken_hold_parts: usize,
    /// Rows, BPM points or scroll velocities whose time (or value) is not a finite number in range: skipped.
    pub invalid_entries: usize,
}

impl Decoder for YavDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode_file(data).map(|(chart, _)| chart)
    }
}

impl YavDecoder {
    /// Decodes a headless chart (a database `Chart` blob, a CDN download) of `keys` columns.
    ///
    /// # Errors
    /// Returns an error if the data is cut short, a note lies beyond `keys`, or the chart has no note or no BPM.
    pub fn decode_headless(data: &[u8], keys: u8) -> RoxResult<(RoxChart, YavReport)> {
        let mut reader = Reader { data, at: 0 };
        let decoded = headless(&mut reader, keys)?;
        Ok(decoded)
    }

    /// Decodes a `.yav` file and says what was dropped.
    ///
    /// # Errors
    /// Returns an error if the file is cut short or too large, its header is not JSON, or the chart does not decode.
    pub fn decode_with_report(data: &[u8]) -> RoxResult<(RoxChart, YavReport)> {
        decode_file(data)
    }
}

fn invalid(reason: impl std::fmt::Display) -> RoxError {
    RoxError::InvalidFormat(format!("yav: {reason}"))
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn bytes(&mut self, count: usize) -> RoxResult<&[u8]> {
        let bytes = self.data.get(self.at..self.at.saturating_add(count)).ok_or_else(|| invalid("cut short"))?;
        self.at += count;
        Ok(bytes)
    }

    fn u8(&mut self) -> RoxResult<u8> {
        Ok(self.bytes(1)?[0])
    }

    fn u16(&mut self) -> RoxResult<u16> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().expect("2 bytes")))
    }

    fn i32(&mut self) -> RoxResult<i32> {
        Ok(i32::from_le_bytes(self.bytes(4)?.try_into().expect("4 bytes")))
    }

    fn f32(&mut self) -> RoxResult<f32> {
        Ok(f32::from_le_bytes(self.bytes(4)?.try_into().expect("4 bytes")))
    }

    /// A time array's count, refused when the rest of the data can not hold that many items of at least `item` bytes.
    fn count(&mut self, item: usize) -> RoxResult<usize> {
        let count = usize::try_from(self.i32()?).map_err(|_| invalid("negative count"))?;
        if count.saturating_mul(item) > self.data.len() - self.at {
            return Err(invalid(format!("{count} entries do not fit the data")));
        }
        Ok(count)
    }

    /// A .NET `BinaryWriter` string: a 7-bit variable-length byte count, then UTF-8.
    fn string(&mut self) -> RoxResult<String> {
        let mut length = 0usize;
        for shift in (0..35).step_by(7) {
            let byte = self.u8()?;
            length |= usize::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                return Ok(String::from_utf8_lossy(self.bytes(length)?).into_owned());
            }
        }
        Err(invalid("string length too long"))
    }
}

/// Milliseconds to microseconds; `None` for a time that is not a number in range.
fn us(ms: f32) -> Option<i64> {
    #[allow(clippy::cast_possible_truncation)] // bounded just before
    (ms.is_finite() && ms.abs() <= MAX_MS).then(|| (f64::from(ms) * 1000.0).round() as i64)
}

fn headless(reader: &mut Reader, keys: u8) -> RoxResult<(RoxChart, YavReport)> {
    if !(1..=MAX_KEYS).contains(&keys) {
        return Err(invalid(format!("{keys} keys")));
    }
    let mut report = YavReport::default();
    let mut notes = Vec::new();
    let mut open: Vec<Option<i64>> = vec![None; usize::from(keys)];
    for _ in 0..reader.count(7)? {
        let time = us(reader.f32()?);
        let mask = reader.u16()?;
        if mask >> keys != 0 {
            return Err(invalid(format!("a note beyond the {keys} columns")));
        }
        for column in (0..keys).filter(|column| mask & (1 << column) != 0) {
            let kind = reader.u8()?;
            let Some(time) = time else {
                report.invalid_entries += 1;
                continue;
            };
            let slot = &mut open[usize::from(column)];
            match (kind, *slot) {
                (1, _) => notes.push(Note::tap(time, column)),
                (2, None) => *slot = Some(time),
                (3, Some(_)) => {}
                (4, Some(start)) if time > start => {
                    notes.push(Note::hold(start, time - start, column));
                    *slot = None;
                }
                _ => report.broken_hold_parts += 1,
            }
        }
    }
    report.broken_hold_parts += open.iter().flatten().count();
    let mut timing = Vec::new();
    for _ in 0..reader.count(12)? {
        let (time, meter, ms_per_beat) = (us(reader.f32()?), reader.i32()?, reader.f32()?);
        let signature = u8::try_from(meter).ok().filter(|&meter| meter > 0);
        match (time, signature) {
            (Some(time_us), Some(signature)) if ms_per_beat.is_finite() && ms_per_beat > 0.0 => {
                timing.push(TimingPoint::Bpm { time_us, bpm: 60_000.0 / ms_per_beat, signature });
            }
            _ => report.invalid_entries += 1,
        }
    }
    for _ in 0..reader.count(8)? {
        let (time, multiplier) = (us(reader.f32()?), reader.f32()?);
        match time.filter(|_| multiplier.is_finite()) {
            Some(time_us) => timing.push(TimingPoint::sv(time_us, multiplier)),
            None => report.invalid_entries += 1,
        }
    }
    if notes.is_empty() {
        return Err(invalid("no note"));
    }
    notes.sort_by_key(|note| (note.time_us, note.column));
    timing.sort_by_key(|point| (point.time_us(), u8::from(point.is_sv())));
    let first_bpm = timing.iter().position(|point| !point.is_sv()).ok_or_else(|| invalid("no BPM"))?;
    // The model wants a BPM no later than the first note and the first point: the first BPM goes on backwards.
    let start = notes[0].time_us.min(timing[0].time_us());
    if let TimingPoint::Bpm { time_us, .. } = &mut timing[first_bpm]
        && *time_us > start
    {
        let mut first = timing.remove(first_bpm);
        if let TimingPoint::Bpm { time_us, .. } = &mut first {
            *time_us = start;
        }
        timing.insert(0, first);
    }
    let mut chart = RoxChart::new(keys);
    chart.notes = notes;
    chart.timing_points = timing;
    chart.validate()?;
    Ok((chart, report))
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "PascalCase")]
struct Header {
    title: String,
    artist: String,
    creator: String,
    diff_name: String,
    preview_time: f64,
    tags: Vec<String>,
    audio_file: Value,
    background_file: Value,
}

fn decode_file(data: &[u8]) -> RoxResult<(RoxChart, YavReport)> {
    if data.len() > MAX_FILE_SIZE {
        return Err(invalid(format!("file too large: {} bytes", data.len())));
    }
    let mut reader = Reader { data, at: 0 };
    let keys = reader.u8()?;
    let header: Header = serde_json::from_str(&reader.string()?).map_err(|e| invalid(format!("header: {e}")))?;
    let (mut chart, report) = headless(&mut reader, keys)?;
    let name = |value: &Value| value.as_str().filter(|name| !name.is_empty()).map(Into::into);
    #[allow(clippy::cast_possible_truncation)] // a preview time in range
    let preview_time_us = if header.preview_time.is_finite() { (header.preview_time * 1000.0).round() as i64 } else { 0 };
    chart.metadata = Metadata {
        title: header.title.into(),
        artist: header.artist.into(),
        creator: header.creator.into(),
        difficulty_name: header.diff_name.into(),
        audio_file: name(&header.audio_file).unwrap_or_default(),
        background_file: name(&header.background_file),
        preview_time_us,
        tags: header.tags.into_iter().map(Into::into).collect(),
        ..Metadata::default()
    };
    Ok((chart, report))
}

#[cfg(test)]
mod tests;
