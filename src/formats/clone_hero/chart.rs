#![warn(clippy::pedantic)]
//! The text of a `.chart` file: sections, `[Song]`, `[SyncTrack]` and the tick to time conversion.
//!
//! Nothing here knows ROX. Sections are cut on lines (`[Name]`, `{`, body lines, `}`), the bodies are kept as slices
//! of the text. The decisions are in `mod.rs`.

use std::borrow::Cow;

/// Largest `.chart` (or `song.ini`) text read, in bytes.
pub const MAX_TEXT_BYTES: usize = 64 * 1024 * 1024;
/// Events later than this many seconds into the chart are not read (a tempo close to 0 would send them out of range).
pub const MAX_SECONDS: f64 = 1.0e7;
/// Distinct sections read; real files have a few dozen.
pub const MAX_SECTIONS: usize = 4096;
/// `MAX_SECONDS` in µs.
pub const MAX_US: f64 = MAX_SECONDS * 1.0e6;
/// Resolution of a chart that does not give one.
pub const DEFAULT_RESOLUTION: u32 = 192;
/// Tempo before the first `B` event, in thousandths of a BPM.
const DEFAULT_TEMPO_MILLI: u64 = 120_000;

/// Text of a file: UTF-8 when it is valid, else Latin-1 (every byte is the code point of the same value); a leading
/// byte-order mark is removed.
#[must_use]
pub fn decode_text(bytes: &[u8]) -> Cow<'_, str> {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => Cow::Borrowed(text),
        Err(_) => Cow::Owned(bytes.iter().map(|&b| char::from(b)).collect()),
    };
    match text {
        Cow::Borrowed(t) => Cow::Borrowed(t.strip_prefix('\u{feff}').unwrap_or(t)),
        Cow::Owned(t) => {
            if let Some(rest) = t.strip_prefix('\u{feff}') {
                Cow::Owned(rest.to_owned())
            } else {
                Cow::Owned(t)
            }
        }
    }
}

/// A `[Section]` with the lines of its body (trimmed, never empty). Sections of the same name are merged.
#[derive(Debug, Default)]
pub struct Section<'a> {
    pub name: &'a str,
    pub lines: Vec<&'a str>,
}

/// The sections of a `.chart` text, in order of first appearance. A line `[Name]` outside a body opens a section, `{`
/// starts its body, `}` ends it; a bracketed line met inside a body opens a new section (an unclosed body).
#[must_use]
pub fn sections(text: &str) -> Vec<Section<'_>> {
    let mut out: Vec<Section<'_>> = Vec::new();
    let mut index: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut current: Option<usize> = None;
    let mut in_body = false;
    for raw in text.split(['\r', '\n']) {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let name = line[1..line.len() - 1].trim();
            current = if let Some(&at) = index.get(name) {
                Some(at)
            } else if out.len() < MAX_SECTIONS {
                out.push(Section {
                    name,
                    lines: Vec::new(),
                });
                index.insert(name, out.len() - 1);
                Some(out.len() - 1)
            } else {
                None // more distinct sections than any real file: the rest is ignored
            };
            in_body = false;
        } else if line == "{" {
            in_body = current.is_some();
        } else if line == "}" {
            in_body = false;
        } else if in_body && let Some(at) = current {
            out[at].lines.push(line);
        }
    }
    out
}

/// `key = value` split at the first `=`, both trimmed.
#[must_use]
pub fn split_pair(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    Some((key.trim(), value.trim()))
}

/// `value` without ONE outermost pair of quotes (there is no escape syntax; quotes inside stay).
#[must_use]
pub fn unquote(value: &str) -> &str {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

/// `<tick> = <rest>` of a track event: the tick (digits only) and what follows the `=`.
#[must_use]
pub fn split_event(line: &str) -> Option<(u64, &str)> {
    let (tick, rest) = line.split_once('=')?;
    let tick = tick.trim();
    if tick.is_empty() || !tick.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((tick.parse().ok()?, rest.trim()))
}

/// The `[Song]` keys ROX reads.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SongSection {
    pub name: Option<String>,
    pub artist: Option<String>,
    pub charter: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    /// `Resolution` as written; `None` when the key is absent.
    pub resolution: Option<u32>,
    /// `Offset`, seconds.
    pub offset_s: Option<f64>,
    /// `PreviewStart` / `PreviewEnd`, seconds.
    pub preview_start_s: Option<f64>,
    pub preview_end_s: Option<f64>,
    /// The stream tags: `(tag, file)`, a file of `None` (or empty) is left out.
    pub streams: Vec<(&'static str, String)>,
}

/// The stream tags of `[Song]`, in the order of the format notes.
pub const STREAM_TAGS: [&str; 11] = [
    "MusicStream",
    "GuitarStream",
    "RhythmStream",
    "BassStream",
    "KeysStream",
    "DrumStream",
    "Drum2Stream",
    "Drum3Stream",
    "Drum4Stream",
    "VocalStream",
    "CrowdStream",
];

fn text_value(value: &str) -> Option<String> {
    let value = unquote(value);
    (!value.is_empty()).then(|| value.to_owned())
}

fn decimal(value: &str) -> Option<f64> {
    unquote(value).parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Reads the body of `[Song]`. Keys are matched as written (the parsers of the format do the same).
///
/// # Errors
/// A `Resolution` that is present but not a positive whole number.
pub fn parse_song(lines: &[&str]) -> Result<SongSection, String> {
    let mut song = SongSection::default();
    for line in lines {
        let Some((key, value)) = split_pair(line) else {
            continue;
        };
        match key {
            "Name" => song.name = text_value(value),
            "Artist" => song.artist = text_value(value),
            "Charter" => song.charter = text_value(value),
            "Album" => song.album = text_value(value),
            "Genre" => song.genre = text_value(value),
            "Resolution" => {
                let res = unquote(value).parse::<u32>().ok().filter(|&r| r > 0);
                song.resolution = Some(
                    res.ok_or_else(|| format!("Resolution `{value}` is not a positive number"))?,
                );
            }
            "Offset" => song.offset_s = decimal(value),
            "PreviewStart" => song.preview_start_s = decimal(value),
            "PreviewEnd" => song.preview_end_s = decimal(value),
            _ => {
                if let Some(tag) = STREAM_TAGS.iter().find(|&&t| t == key)
                    && let Some(file) = text_value(value)
                    && !file.eq_ignore_ascii_case("none")
                {
                    song.streams.push((tag, file));
                }
            }
        }
    }
    Ok(song)
}

/// What the `[SyncTrack]` lines held.
#[derive(Debug, Default)]
pub struct SyncEvents {
    /// `(tick, tempo in thousandths of a BPM)`, last of a tick wins, sorted.
    pub tempos: Vec<(u64, u64)>,
    /// `(tick, beats per measure in quarter notes)`, last of a tick wins, sorted.
    pub signatures: Vec<(u64, u8)>,
    /// Lines that are not `<tick> = <code> ...`.
    pub malformed: usize,
    /// `B` events whose tempo is 0, not a number or beyond `u32`, `TS` events with a numerator of 0 or an exponent
    /// above 31: ignored.
    pub ignored: usize,
    /// Tempos and signatures overwritten by a later event on the same tick.
    pub replaced: usize,
}

/// Reads the body of `[SyncTrack]`. `A` (anchors) and unknown codes are skipped.
#[must_use]
pub fn parse_sync(lines: &[&str]) -> SyncEvents {
    let mut out = SyncEvents::default();
    let mut tempos: Vec<(u64, u64)> = Vec::new();
    let mut signatures: Vec<(u64, u8)> = Vec::new();
    for line in lines {
        let Some((tick, rest)) = split_event(line) else {
            out.malformed += 1;
            continue;
        };
        let mut words = rest.split_whitespace();
        match words.next() {
            Some("B") => match words.next().and_then(|w| w.parse::<u64>().ok()) {
                Some(milli) if milli > 0 && u32::try_from(milli).is_ok() => {
                    tempos.push((tick, milli));
                }
                _ => out.ignored += 1,
            },
            Some("TS") => {
                let numerator = words.next().and_then(|w| w.parse::<u32>().ok());
                let exponent = match words.next() {
                    None => Some(2),
                    Some(w) => w.parse::<u32>().ok(),
                };
                match (numerator, exponent) {
                    (Some(n), Some(e)) if n > 0 && e <= 31 => {
                        signatures.push((tick, quarter_beats(n, e)));
                    }
                    _ => out.ignored += 1,
                }
            }
            _ => {}
        }
    }
    out.replaced += last_of_tick(&mut tempos) + last_of_tick(&mut signatures);
    out.tempos = tempos;
    out.signatures = signatures;
    out
}

/// Sorts by tick (stable) and keeps the last event of each tick; returns how many were dropped.
pub(super) fn last_of_tick<T: Copy>(events: &mut Vec<(u64, T)>) -> usize {
    events.sort_by_key(|e| e.0);
    let before = events.len();
    let mut kept: Vec<(u64, T)> = Vec::with_capacity(before);
    for &event in events.iter() {
        match kept.last_mut() {
            Some(last) if last.0 == event.0 => *last = event,
            _ => kept.push(event),
        }
    }
    *events = kept;
    before - events.len()
}

/// Beats per measure counted in quarter notes: `numerator x 4 / 2^exponent`, rounded, at least 1, at most 255.
pub(super) fn quarter_beats(numerator: u32, exponent: u32) -> u8 {
    let beats = (u64::from(numerator) * 4 + (1u64 << exponent) / 2) >> exponent;
    u8::try_from(beats.clamp(1, 255)).unwrap_or(u8::MAX)
}

/// One tempo of the map.
#[derive(Debug, Clone, Copy)]
pub struct Segment {
    pub tick: u64,
    /// Thousandths of a BPM.
    pub milli: u64,
    /// µs of the chart at `tick` (unrounded, so that tempo changes do not accumulate rounding).
    pub start_us: f64,
}

/// Ticks to microseconds: `60 / bpm x ticks / resolution` seconds accumulated tempo by tempo from tick 0 (time
/// signatures do not enter it).
#[derive(Debug)]
pub struct TempoMap {
    pub segments: Vec<Segment>,
    resolution: f64,
}

impl TempoMap {
    /// The map of `tempos` (sorted, one per tick). A chart with no `B` at tick 0 starts at 120 BPM.
    #[must_use]
    pub fn new(resolution: u32, tempos: &[(u64, u64)]) -> Self {
        let resolution_f = f64::from(resolution);
        let mut pairs: Vec<(u64, u64)> = Vec::with_capacity(tempos.len() + 1);
        if tempos.first().is_none_or(|first| first.0 > 0) {
            pairs.push((0, DEFAULT_TEMPO_MILLI));
        }
        pairs.extend_from_slice(tempos);
        let mut segments: Vec<Segment> = Vec::with_capacity(pairs.len());
        for (tick, milli) in pairs {
            let start_us = match segments.last() {
                None => 0.0,
                Some(prev) => prev.start_us + span_us(tick - prev.tick, prev.milli, resolution_f),
            };
            segments.push(Segment {
                tick,
                milli,
                start_us,
            });
        }
        Self {
            segments,
            resolution: resolution_f,
        }
    }

    /// Time of `tick` in µs, unrounded.
    #[allow(clippy::cast_precision_loss)] // ticks of a text file stay far below 2^53
    #[must_use]
    pub fn us(&self, tick: u64) -> f64 {
        let at = self.segments.partition_point(|s| s.tick <= tick) - 1;
        let seg = &self.segments[at];
        seg.start_us + span_us(tick - seg.tick, seg.milli, self.resolution)
    }
}

/// µs spanned by `ticks` at `milli` thousandths of a BPM.
#[allow(clippy::cast_precision_loss)] // see `TempoMap::us`
fn span_us(ticks: u64, milli: u64, resolution: f64) -> f64 {
    ticks as f64 * 60.0e9 / (milli as f64 * resolution)
}
