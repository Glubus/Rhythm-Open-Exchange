#![warn(clippy::pedantic)]
//! R2Beat obstacle charts (`.xml`). Decode only.
//!
//! Format notes: `vsrg-format-docs/vsrg/r2beat/xml`, from the r2beat-note editor (`note.ts`, `MusicPlayer.vue`; read for facts) and the
//! Newton wiki, checked on the corpus charts.
//!
//! - **File**: XML in EUC-KR or GB18030 (as the declaration says): a `TITLE` root (`Name`), `BPM` entries (`Frame`, `BPM`), `BGM`
//!   (`Name`, under the game's `rnr_music`), `DELAY` (frames), then one `AREA` per obstacle (`Coord`, `Kind`). Told by its content
//!   ([`looks_like_r2beat`]): `.xml` is any XML file's extension.
//! - **Time** (as the editor's player): 60 frames a second; BPM entry `i` takes over at `(Frame + DELAY) / 60` s, the first one also holds
//!   from 0; `Coord` grows by `BPM / 5` a second from 0 (12 a beat).
//! - **Columns**: the four arrow keys, left, down, up, right. Kind 18 left, 17 down, 16 up, 19 right; the diagonals press two keys (20
//!   up-left, 21 up-right, 22 down-left, 23 down-right). Long obstacles are holds from their start to their end kind (up 134…136, down
//!   137…139, left 140…142, right 143…145, up-left 128…130, up-right 131…133). Platforms (24), stars (26, 27), the editor's random kind
//!   (161) and any other kind are no key: dropped and counted.
//! - **Metadata**: the title, the music (the file name of `BGM`, which sits beside the chart in the game's folders). The obstacle
//!   key sounds (`EFFECT`) are not read.

use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, Note, RoxChart, TimingPoint, drop_stacked};

/// Decoder of R2Beat `.xml` charts: one chart per file.
pub struct R2BeatDecoder;

/// Files above this size are refused (a chart is a few hundred kilobytes).
const MAX_FILE_SIZE: usize = 32 * 1024 * 1024;
const LEFT: u8 = 0;
const DOWN: u8 = 1;
const UP: u8 = 2;
const RIGHT: u8 = 3;
/// Times further than this many seconds from the start are not read.
const MAX_SECONDS: f64 = 1e7;

/// What the decoder dropped. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct R2BeatReport {
    /// Obstacles that are no key (platforms, stars, random and unknown kinds): dropped.
    pub no_key_obstacles: usize,
    /// Long-obstacle parts without their start or end: dropped.
    pub broken_holds: usize,
    /// Notes on another note of their column (same time, or inside a hold): dropped.
    pub overlapping_notes_dropped: usize,
}

/// Whether the file at `path` is an R2Beat chart: an `.xml` whose start [`looks_like_r2beat`]. The start of the file is read.
#[must_use]
pub fn is_r2beat_chart_file(path: &std::path::Path) -> bool {
    use std::io::Read;
    if !path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("xml")) {
        return false;
    }
    let mut head = Vec::new();
    std::fs::File::open(path).and_then(|file| file.take(4096).read_to_end(&mut head)).is_ok_and(|_| looks_like_r2beat(&head))
}

/// Whether the start of a file is an R2Beat chart: XML whose root is `TITLE` with a `BPM Frame` entry.
#[must_use]
pub fn looks_like_r2beat(head: &[u8]) -> bool {
    let text = String::from_utf8_lossy(head);
    let body = text.trim_start_matches('\u{feff}').trim_start();
    let body = if body.starts_with("<?xml") { body.split_once("?>").map_or("", |(_, rest)| rest).trim_start() } else { body };
    body.starts_with("<TITLE") && body.contains("<BPM Frame=")
}

fn invalid(reason: impl std::fmt::Display) -> RoxError {
    RoxError::InvalidFormat(format!("r2beat: {reason}"))
}

/// The opening tags of a flat XML file: `(name, attributes)`.
fn tags(text: &str) -> impl Iterator<Item = (&str, Vec<(&str, &str)>)> {
    text.split('<').skip(1).filter_map(|piece| {
        let tag = piece.split_once('>').map_or(piece, |(tag, _)| tag);
        if tag.starts_with(['/', '?', '!']) {
            return None;
        }
        let (name, mut rest) = tag.split_once(char::is_whitespace).unwrap_or((tag.trim_end_matches('/'), ""));
        let mut attributes = Vec::new();
        while let Some((key, after)) = rest.split_once("=\"") {
            let Some((value, after)) = after.split_once('"') else { break };
            attributes.push((key.trim(), value));
            rest = after;
        }
        Some((name, attributes))
    })
}

fn attribute<'a>(attributes: &[(&str, &'a str)], key: &str) -> Option<&'a str> {
    attributes.iter().find(|(name, _)| *name == key).map(|(_, value)| *value)
}

fn number(attributes: &[(&str, &str)], key: &str) -> Option<f64> {
    attribute(attributes, key).and_then(|value| value.trim().parse().ok()).filter(|value: &f64| value.is_finite())
}

/// The text of a file in the encoding its declaration names (EUC-KR, GB18030), else UTF-8.
fn text_of(data: &[u8]) -> String {
    let head = String::from_utf8_lossy(&data[..data.len().min(200)]).to_ascii_lowercase();
    let encoding = head
        .split_once("encoding=\"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .and_then(|(label, _)| encoding_rs::Encoding::for_label(label.as_bytes()))
        .unwrap_or(encoding_rs::UTF_8);
    encoding.decode(data).0.into_owned()
}

/// What an obstacle kind asks for.
enum Key {
    Tap(&'static [u8]),
    HoldStart(&'static [u8]),
    HoldBody,
    HoldEnd(&'static [u8]),
    None,
}

fn key_of(kind: i64) -> Key {
    match kind {
        18 => Key::Tap(&[LEFT]),
        17 => Key::Tap(&[DOWN]),
        16 => Key::Tap(&[UP]),
        19 => Key::Tap(&[RIGHT]),
        20 => Key::Tap(&[UP, LEFT]),
        21 => Key::Tap(&[UP, RIGHT]),
        22 => Key::Tap(&[DOWN, LEFT]),
        23 => Key::Tap(&[DOWN, RIGHT]),
        134 => Key::HoldStart(&[UP]),
        137 => Key::HoldStart(&[DOWN]),
        140 => Key::HoldStart(&[LEFT]),
        143 => Key::HoldStart(&[RIGHT]),
        128 => Key::HoldStart(&[UP, LEFT]),
        131 => Key::HoldStart(&[UP, RIGHT]),
        135 | 138 | 141 | 144 | 129 | 132 => Key::HoldBody,
        136 => Key::HoldEnd(&[UP]),
        139 => Key::HoldEnd(&[DOWN]),
        142 => Key::HoldEnd(&[LEFT]),
        145 => Key::HoldEnd(&[RIGHT]),
        130 => Key::HoldEnd(&[UP, LEFT]),
        133 => Key::HoldEnd(&[UP, RIGHT]),
        _ => Key::None,
    }
}

/// Coord to seconds through the tempo points `(seconds, BPM)`, the first at 0.
fn seconds_of(coord: f64, points: &[(f64, f64)]) -> f64 {
    let mut reached = 0.0;
    for (index, &(start, bpm)) in points.iter().enumerate() {
        let rate = bpm / 5.0;
        let span = points.get(index + 1).map_or(f64::INFINITY, |&(next, _)| (next - start) * rate);
        if coord - reached <= span {
            return start + (coord - reached) / rate;
        }
        reached += span;
    }
    unreachable!("the last segment never ends")
}

impl R2BeatDecoder {
    /// Decodes a chart and says what was dropped.
    ///
    /// # Errors
    /// Returns an error if the file is too large, not an R2Beat chart, or has no key obstacle.
    pub fn decode_with_report(data: &[u8]) -> RoxResult<(RoxChart, R2BeatReport)> {
        if data.len() > MAX_FILE_SIZE {
            return Err(invalid(format!("file too large: {} bytes", data.len())));
        }
        let text = text_of(data);
        let mut report = R2BeatReport::default();
        let (mut title, mut music, mut delay) = ("", "", 0.0);
        let mut bpms: Vec<(f64, f64)> = Vec::new();
        let mut areas: Vec<(f64, i64)> = Vec::new();
        let mut root = false;
        for (name, attributes) in tags(&text) {
            match name {
                "TITLE" => {
                    root = true;
                    title = attribute(&attributes, "Name").unwrap_or_default();
                }
                "BPM" => {
                    if let (Some(frame), Some(bpm)) = (number(&attributes, "Frame"), number(&attributes, "BPM").filter(|bpm| *bpm > 0.0)) {
                        bpms.push((frame, bpm));
                    }
                }
                "BGM" => music = attribute(&attributes, "Name").unwrap_or_default().rsplit(['/', '\\']).next().unwrap_or_default(),
                "DELAY" => delay = number(&attributes, "Value").unwrap_or(0.0),
                #[allow(clippy::cast_possible_truncation)] // kinds are small integers
                "AREA" => {
                    if let (Some(coord), Some(kind)) = (number(&attributes, "Coord"), number(&attributes, "Kind")) {
                        areas.push((coord, kind as i64));
                    }
                }
                _ => {}
            }
        }
        if !root || bpms.is_empty() {
            return Err(invalid("not an R2Beat chart (TITLE with BPM entries)"));
        }
        bpms.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut points: Vec<(f64, f64)> = bpms.iter().map(|&(frame, bpm)| ((frame + delay) / 60.0, bpm)).collect();
        if points[0].0 > 0.0 {
            points.insert(0, (0.0, points[0].1));
        }
        areas.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut chart = RoxChart::new(4);
        let mut holding: [Option<i64>; 4] = [None; 4];
        for (coord, kind) in areas {
            let seconds = seconds_of(coord.max(0.0), &points);
            if seconds > MAX_SECONDS {
                return Err(invalid("time out of range"));
            }
            #[allow(clippy::cast_possible_truncation)] // bounded above
            let time = (seconds * 1e6).round() as i64;
            match key_of(kind) {
                Key::Tap(columns) => chart.notes.extend(columns.iter().map(|&column| Note::tap(time, column))),
                Key::HoldStart(columns) => {
                    for &column in columns {
                        holding[usize::from(column)] = Some(time);
                    }
                }
                Key::HoldBody => {}
                Key::HoldEnd(columns) => {
                    for &column in columns {
                        match holding[usize::from(column)].take() {
                            Some(start) => chart.notes.push(Note::hold(start, (time - start).max(1), column)),
                            None => report.broken_holds += 1,
                        }
                    }
                }
                Key::None => report.no_key_obstacles += 1,
            }
        }
        report.broken_holds += holding.iter().flatten().count();
        if chart.notes.is_empty() {
            return Err(invalid("no key obstacle"));
        }
        chart.notes.sort_by_key(|note| (note.time_us, note.column));
        report.overlapping_notes_dropped = drop_stacked(&mut chart.notes, 4, |note| note).total();
        #[allow(clippy::cast_possible_truncation)] // bounded times; a BPM fits f32
        {
            chart.timing_points = points.iter().map(|&(seconds, bpm)| TimingPoint::bpm((seconds * 1e6).round() as i64, bpm as f32)).collect();
        }
        chart.metadata = Metadata { title: title.into(), audio_file: music.into(), ..Metadata::default() };
        chart.validate()?;
        Ok((chart, report))
    }
}

impl rox::codec::Decoder for R2BeatDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        R2BeatDecoder::decode_with_report(data).map(|(chart, _)| chart)
    }
}

crate::impl_format!(R2BeatDecoder, ["xml"]);

#[cfg(test)]
mod tests;
