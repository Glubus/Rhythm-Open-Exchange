#![warn(clippy::pedantic)]
//! Lexing of a BMS file: control flow resolved, headers and channel lines collected.
//!
//! Nothing here knows about game layouts or time; it only turns text into
//! [`BmsFile`]. Rules (Be-Music Source, hitkey "BMS command memo"):
//!
//! - a line is a statement when it starts with `#` (after blanks); every other line is a comment;
//! - command names and object ids are case-insensitive (ids are read as upper case);
//! - a header given twice keeps the LAST one; the lines of one channel of one measure are merged later by the decoder;
//! - `#RANDOM n` always draws 1 (reproducible), `#SETRANDOM n` forces `n`; see [`RandomState`].

use std::collections::{BTreeMap, HashMap};

/// A position inside a measure, as the reduced fraction `num / den` (`num < den`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Frac {
    pub num: u32,
    pub den: u32,
}

impl Frac {
    /// Slot `index` of a line of `slots` slots.
    pub fn new(index: u32, slots: u32) -> Self {
        let g = gcd(index, slots).max(1);
        Self {
            num: index / g,
            den: slots / g,
        }
    }

    pub fn cmp(self, other: Self) -> std::cmp::Ordering {
        (u64::from(self.num) * u64::from(other.den))
            .cmp(&(u64::from(other.num) * u64::from(self.den)))
    }

    pub fn as_f64(self) -> f64 {
        f64::from(self.num) / f64::from(self.den)
    }
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// One non-empty object of a channel line.
#[derive(Debug, Clone, Copy)]
pub(super) struct Obj {
    pub frac: Frac,
    /// Number of slots of the line this object came from (the grid of the line).
    pub unit: u32,
    /// The two characters of the object, upper case.
    pub pair: [u8; 2],
}

/// One `#mmmCC:data` line.
#[derive(Debug)]
pub(super) struct Line {
    pub measure: u16,
    /// Channel, upper case.
    pub channel: [u8; 2],
    pub objs: Vec<Obj>,
}

/// What the lexer counted while it worked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParseReport {
    /// `#IF`/`#ELSEIF`/`#ELSE` branches that were not taken.
    pub branches_skipped: usize,
    /// Lines with an odd number of data characters (the last character is ignored).
    pub odd_data_lines: usize,
}

#[derive(Debug, Default)]
pub(super) struct BmsFile {
    /// Plain headers (`TITLE`, `ARTIST`, `BPM`, `PLAYLEVEL`, ...), names upper case, last one wins.
    pub headers: HashMap<String, String>,
    /// `#WAVxx`: file name by id.
    pub wav: HashMap<u16, String>,
    /// `#BPMxx` / `#EXBPMxx`.
    pub bpm_table: HashMap<u16, f64>,
    /// `#STOPxx`.
    pub stop_table: HashMap<u16, f64>,
    pub lnobj: Option<u16>,
    /// `#LNTYPE`: 1 (default) or 2.
    pub lntype: u8,
    /// Channel `02`, last line of a measure wins.
    pub measure_len: BTreeMap<u16, f64>,
    pub lines: Vec<Line>,
    pub report: ParseReport,
}

/// Base-36 digit.
pub(super) fn b36(c: u8) -> Option<u16> {
    match c {
        b'0'..=b'9' => Some(u16::from(c - b'0')),
        b'A'..=b'Z' => Some(u16::from(c - b'A') + 10),
        b'a'..=b'z' => Some(u16::from(c - b'a') + 10),
        _ => None,
    }
}

/// A two-character base-36 id (`01`..`ZZ`); `None` when a character is not alphanumeric.
pub(super) fn id(pair: [u8; 2]) -> Option<u16> {
    Some(b36(pair[0])? * 36 + b36(pair[1])?)
}

/// `#IF` nesting state. The random value in effect is the top of `values`, 1 before any `#RANDOM`
/// (players such as `IIDXv` and HDX also default to 1).
struct RandomState {
    values: Vec<u32>,
    frames: Vec<Frame>,
}

struct Frame {
    parent_active: bool,
    active: bool,
    matched: bool,
}

impl RandomState {
    fn new() -> Self {
        Self {
            values: vec![1],
            frames: Vec::new(),
        }
    }

    fn active(&self) -> bool {
        self.frames.last().is_none_or(|frame| frame.active)
    }

    fn value(&self) -> u32 {
        *self.values.last().unwrap_or(&1)
    }

    fn open(&mut self, wanted: Option<u32>) {
        let parent_active = self.active();
        let active = parent_active && wanted == Some(self.value());
        self.frames.push(Frame {
            parent_active,
            active,
            matched: active,
        });
    }

    fn branch(&mut self, wanted: Option<u32>, is_else: bool) {
        let value = self.value();
        if let Some(frame) = self.frames.last_mut() {
            let hit = if is_else {
                !frame.matched
            } else {
                !frame.matched && wanted == Some(value)
            };
            frame.active = frame.parent_active && hit;
            frame.matched |= hit;
        }
    }
}

pub(super) fn parse(text: &str) -> BmsFile {
    let mut file = BmsFile {
        lntype: 1,
        ..BmsFile::default()
    };
    let mut random = RandomState::new();
    // `str::lines` splits on `\n` and `\r\n`; a lone `\r` also ends a line in BMS files.
    for raw in text.split(['\n', '\r']) {
        let line = raw.trim_start_matches('\u{feff}').trim();
        let Some(body) = line.strip_prefix('#') else {
            continue;
        };
        let body = strip_ext(body);
        if is_channel_line(body) {
            if random.active() {
                channel_line(&mut file, body);
            }
            continue;
        }
        let (name, value) = match body.split_once(|c: char| c.is_ascii_whitespace()) {
            Some((name, value)) => (name, value.trim()),
            None => (body, ""),
        };
        let name = name.to_ascii_uppercase();
        match name.as_str() {
            "RANDOM" => random.values.push(1),
            "SETRANDOM" => random.values.push(value.parse().unwrap_or(1)),
            "ENDRANDOM" => {
                if random.values.len() > 1 {
                    random.values.pop();
                }
            }
            "IF" => {
                random.open(value.parse().ok());
                if !random.active() && random.frames.last().is_some_and(|f| f.parent_active) {
                    file.report.branches_skipped += 1;
                }
            }
            "ELSEIF" | "ELSE" => {
                random.branch(value.parse().ok(), name == "ELSE");
                if !random.active() && random.frames.last().is_some_and(|f| f.parent_active) {
                    file.report.branches_skipped += 1;
                }
            }
            "ENDIF" => {
                random.frames.pop();
            }
            _ if random.active() => header(&mut file, &name, value),
            _ => {}
        }
    }
    file
}

/// Removes the `EXT ` prefix some Bemuse files put before a statement (`#EXT #00111:...`).
fn strip_ext(body: &str) -> &str {
    match body.get(..3) {
        Some(prefix) if prefix.eq_ignore_ascii_case("EXT") => {
            body[3..].trim_start().strip_prefix('#').unwrap_or(body)
        }
        _ => body,
    }
}

/// `mmmCC:` with three decimal digits and two alphanumeric characters.
fn is_channel_line(body: &str) -> bool {
    let bytes = body.as_bytes();
    bytes.len() >= 6
        && bytes[..3].iter().all(u8::is_ascii_digit)
        && bytes[3..5].iter().all(u8::is_ascii_alphanumeric)
        && bytes[5] == b':'
}

fn channel_line(file: &mut BmsFile, body: &str) {
    let bytes = body.as_bytes();
    let measure = u16::from(bytes[0] - b'0') * 100
        + u16::from(bytes[1] - b'0') * 10
        + u16::from(bytes[2] - b'0');
    let channel = [bytes[3].to_ascii_uppercase(), bytes[4].to_ascii_uppercase()];
    let data = body[6..].trim();
    if channel == *b"02" {
        // The measure length is a real number, not object pairs.
        if let Ok(length) = data.parse::<f64>()
            && length.is_finite()
            && length >= 0.0
        {
            file.measure_len.insert(measure, length);
        }
        return;
    }
    let data = data.as_bytes();
    if data.len() % 2 == 1 {
        file.report.odd_data_lines += 1;
    }
    let slots = u32::try_from(data.len() / 2).unwrap_or(u32::MAX);
    let mut objs = Vec::new();
    #[allow(clippy::chunks_exact_to_as_chunks)]
    // `as_chunks` is newer than the toolchain this crate supports
    for (index, pair) in data.chunks_exact(2).enumerate() {
        let pair = [pair[0].to_ascii_uppercase(), pair[1].to_ascii_uppercase()];
        if pair == *b"00" || id(pair).is_none() {
            continue;
        }
        objs.push(Obj {
            frac: Frac::new(u32::try_from(index).unwrap_or(u32::MAX), slots),
            unit: slots,
            pair,
        });
    }
    file.lines.push(Line {
        measure,
        channel,
        objs,
    });
}

/// The two-character base-36 suffix of `name` after `prefix` (`WAV01`, `BPMZZ`), as an id.
fn suffix_id(name: &str, prefix: &str) -> Option<u16> {
    let rest = name.strip_prefix(prefix)?.as_bytes();
    (rest.len() == 2).then(|| id([rest[0], rest[1]]))?
}

fn number(value: &str) -> Option<f64> {
    let token = value.split_ascii_whitespace().next()?;
    token.parse::<f64>().ok().filter(|n| n.is_finite())
}

fn header(file: &mut BmsFile, name: &str, value: &str) {
    if let Some(id) = suffix_id(name, "WAV") {
        file.wav.insert(id, value.replace('\\', "/"));
    } else if let Some(id) = suffix_id(name, "EXBPM").or_else(|| suffix_id(name, "BPM")) {
        if let Some(bpm) = number(value) {
            file.bpm_table.insert(id, bpm);
        }
    } else if let Some(id) = suffix_id(name, "STOP") {
        if let Some(stop) = number(value) {
            file.stop_table.insert(id, stop);
        }
    } else if name == "LNOBJ" {
        let bytes = value.trim().as_bytes();
        file.lnobj = (bytes.len() == 2)
            .then(|| id([bytes[0].to_ascii_uppercase(), bytes[1].to_ascii_uppercase()]))
            .flatten();
    } else if name == "LNTYPE" {
        file.lntype = if value.trim() == "2" { 2 } else { 1 };
    } else if !name.is_empty() {
        file.headers.insert(name.to_string(), value.to_string());
    }
}
