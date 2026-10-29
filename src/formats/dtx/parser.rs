#![warn(clippy::pedantic)]
//! Lexing of a DTX file: headers, resource tables and chips, with the control flow resolved.
//!
//! Nothing here knows about lanes, columns or time; it turns the text into a [`DtxFile`]. The rules are those of
//! `CDTX.cs` of `DTXmaniaNX` (read, not copied; `tRead_FromString`, `t入力_行解析`, `tInput_LineAnalysis_ChipLocation`):
//!
//! - **Line**: blanks, `#`, the command up to `:`, a blank or `;`, then the parameter up to `;` (a comment) or the end of
//!   the line, trimmed. A line that does not start with `#` is a comment. `\r`, `\n` and `\r\n` end a line, a tab is a blank.
//!   Command names are case-insensitive and match by PREFIX in a fixed order: a command that starts with a known name and
//!   goes on (`#TITLEfoo`) gives the rest as the parameter, in place of the parameter that follows.
//! - **Data line**: the command is five characters, `mmmcc`: the measure is a base-36 digit times 100 plus two decimal
//!   digits (`000`..`Z99`), the channel two hexadecimal digits. The objects are pairs of base-36 digits (`_` ignored; a
//!   character that is not base 36 refuses the line; an odd last character is dropped; `00` is empty), except channel `03`
//!   (hexadecimal). The object `i` of `n` sits at tick `384 * i / n` (integer division) of the 384 ticks of the measure. The
//!   lines of one channel of one measure are NOT merged: every object is a chip.
//! - **Tables** (`#WAVzz`, `#VOLUMEzz`/`#WAVVOLzz`, `#BPMzz`): a resource may be used before it is defined and may be
//!   defined again. A chip uses the definition most recently read BEFORE its line; a chip with none before it uses the first
//!   one read after ([`Table::at`]). `#VOLUMEzz` works the same way for the `#WAVzz` it precedes.
//! - **Control flow**: `#RANDOM n` always draws 1 (the game draws 1..n at random; the decoder is reproducible), the value is 0
//!   before any `#RANDOM`, `#IF m` keeps its block when `m` equals it (an unreadable `m` is 1), blocks nest up to 255 and a
//!   skipped block hides everything, data lines included. There is no `#ELSE`.
//! - **Numbers**: a real number takes `.` or `,` as the decimal point (the last one of the leading run of digits and
//!   separators; `.` `,` `'` and blanks inside are dropped), see [`real`].

use std::collections::{BTreeMap, HashMap};

use super::channels::{Dropped, Role, classify};
use crate::formats::bms::bmson::safe_name;

/// Ticks of a measure: the resolution of a DTX chart.
pub(super) const TICKS: u64 = 384;

/// What the lexer counted while it worked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ParseReport {
    pub branches_skipped: usize,
    pub invalid_data_lines: usize,
    pub odd_data_lines: usize,
    pub ignored_bar_lengths: usize,
    pub ignored_bpm_changes: usize,
    pub hidden_chips: usize,
    pub blank_hit_sounds: usize,
    pub wailing_chips: usize,
    pub fill_in_chips: usize,
    pub visual_chips: usize,
    pub inert_chips: usize,
    pub unknown_chips: usize,
    pub unsafe_names: usize,
    pub dtxv_play_speed_ignored: bool,
    pub path_wav_ignored: bool,
}

/// One object of a data line that the decoder reads.
#[derive(Debug, Clone, Copy)]
pub(super) struct Chip {
    pub measure: u16,
    /// Tick in the measure, `0..384`.
    pub tick: u16,
    pub channel: u8,
    /// The object: a base-36 id (a `#WAVzz` or `#BPMzz` index), or the BPM itself for channel `03` (hexadecimal).
    pub id: u16,
    /// Order of the statement in the file: tables are looked up by it.
    pub seq: u32,
}

/// The definitions of one kind of table, by id, in file order.
#[derive(Debug)]
pub(super) struct Table<T> {
    entries: HashMap<u16, Vec<(u32, T)>>,
}

impl<T> Default for Table<T> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

impl<T> Table<T> {
    fn push(&mut self, id: u16, seq: u32, value: T) {
        self.entries.entry(id).or_default().push((seq, value));
    }

    /// The first definition of `id` in the file.
    pub fn first(&self, id: u16) -> Option<&T> {
        self.entries.get(&id)?.first().map(|(_, value)| value)
    }

    /// The definition a user at statement `seq` sees: the last one before it, else the first one of the file.
    pub fn at(&self, id: u16, seq: u32) -> Option<&T> {
        let list = self.entries.get(&id)?;
        let before = list.partition_point(|(s, _)| *s < seq);
        let at = before.saturating_sub(1);
        list.get(at).map(|(_, value)| value)
    }

    fn map<U>(self, mut f: impl FnMut(u16, u32, T) -> U) -> Table<U> {
        Table {
            entries: self
                .entries
                .into_iter()
                .map(|(id, list)| {
                    let list = list
                        .into_iter()
                        .map(|(seq, value)| (seq, f(id, seq, value)))
                        .collect();
                    (id, list)
                })
                .collect(),
        }
    }
}

/// A `#WAVzz` definition: the sanitised file name (empty when absent or refused) and its `#VOLUMEzz` in percent.
#[derive(Debug, Clone)]
pub(super) struct Sound {
    pub name: String,
    pub volume: u8,
}

/// `#DLEVEL`, `#GLEVEL`, `#BLEVEL` of the game: the level (tenths) and its decimal part (hundredths).
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Level {
    pub value: Option<i32>,
    pub decimal: i32,
}

#[derive(Debug, Default)]
pub(super) struct DtxFile {
    /// Text headers by canonical name (`TITLE`, `ARTIST`, `GENRE`, `BACKGROUND`, `BACKGROUND_GR`, `STAGEFILE`, `PREIMAGE`), last one wins.
    pub headers: HashMap<&'static str, String>,
    /// Drums, guitar, bass.
    pub levels: [Level; 3],
    pub base_bpm: Option<f64>,
    pub bpm: Table<f64>,
    pub wav: Table<Sound>,
    /// `#mmm02`: the length of a measure, the last line of a measure wins.
    pub measure_len: BTreeMap<u16, f64>,
    pub chips: Vec<Chip>,
    pub report: ParseReport,
}

/// The value of a base-36 digit.
fn b36(c: u8) -> Option<u16> {
    match c {
        b'0'..=b'9' => Some(u16::from(c - b'0')),
        b'A'..=b'Z' => Some(u16::from(c - b'A') + 10),
        b'a'..=b'z' => Some(u16::from(c - b'a') + 10),
        _ => None,
    }
}

fn hex(c: u8) -> Option<u16> {
    match c {
        b'0'..=b'9' | b'A'..=b'F' | b'a'..=b'f' => b36(c),
        _ => None,
    }
}

/// A two-character base-36 index.
fn id36(pair: &[u8]) -> Option<u16> {
    Some(b36(*pair.first()?)? * 36 + b36(*pair.get(1)?)?)
}

/// A real number as the game reads it (`TryParse` of `CDTX.cs`): digits, with `.` or `,` as the decimal point; the last one of
/// the leading run of digits and separators is the point, the other separators and `'` and blanks are dropped, and the first
/// other character ends the number. Only finite numbers.
pub(super) fn real(text: &str) -> Option<f64> {
    let bytes = text.as_bytes();
    let mut point = None;
    let mut end = bytes.len();
    for (i, c) in bytes.iter().enumerate() {
        match c {
            b'0'..=b'9' | b'\'' | b' ' => {}
            b'.' | b',' => point = Some(i),
            _ => {
                end = i;
                break;
            }
        }
    }
    let mut number = String::with_capacity(end.min(32));
    for (i, c) in bytes[..end].iter().enumerate() {
        match c {
            b'0'..=b'9' => number.push(char::from(*c)),
            _ if Some(i) == point => number.push('.'),
            _ => {}
        }
    }
    number.parse::<f64>().ok().filter(|n| n.is_finite())
}

fn int(text: &str) -> Option<i32> {
    text.trim().parse().ok()
}

/// Whether `command` starts with `name`, ignoring the case.
fn starts_with(command: &str, name: &str) -> bool {
    command
        .as_bytes()
        .get(..name.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(name.as_bytes()))
}

/// The command and its parameter of one line, `None` for a comment.
fn statement(line: &str) -> Option<(&str, &str)> {
    let body = line
        .trim_start_matches([' ', '\t', '\u{feff}'])
        .strip_prefix('#')?;
    let end = body.find([':', ' ', '\t', ';']).unwrap_or(body.len());
    let (command, rest) = body.split_at(end);
    let rest = rest.strip_prefix(':').unwrap_or(rest);
    let rest = rest.trim_start_matches([' ', '\t']);
    Some((command, rest.split(';').next().unwrap_or("").trim()))
}

/// A data line command: `mmmcc`, the measure and the channel.
fn data_command(command: &str) -> Option<(u16, u8)> {
    let c = command.as_bytes();
    if c.len() != 5 || !c[1].is_ascii_digit() || !c[2].is_ascii_digit() {
        return None;
    }
    let measure = b36(c[0])? * 100 + u16::from(c[1] - b'0') * 10 + u16::from(c[2] - b'0');
    let channel = u8::try_from(hex(c[3])? * 16 + hex(c[4])?).ok()?;
    Some((measure, channel))
}

/// The statements that set a header, in the order the game tests them (a name that is the start of another comes after it).
const NAMES: [&str; 22] = [
    "PATH_WAV",
    "PATH",
    "TITLE",
    "ARTIST",
    "COMMENT",
    "DLEVEL",
    "PLAYLEVEL",
    "GLEVEL",
    "BLEVEL",
    "DLVDEC",
    "GLVDEC",
    "BLVDEC",
    "GENRE",
    "STAGEFILE",
    "PREVIEW",
    "PREIMAGE",
    "PREMOVIE",
    "BACKGROUND_GR",
    "BACKGROUND",
    "WALL",
    "DTXVPLAYSPEED",
    "BASEBPM",
];

pub(super) fn parse(text: &str) -> DtxFile {
    let mut file = DtxFile::default();
    let mut wav: Table<String> = Table::default();
    let mut volume: Table<u8> = Table::default();
    let mut path = String::new();
    // `#IF` nesting: true while the block is skipped.
    let mut skipped = vec![false];
    let mut random = 0i32;
    let mut seq = 0u32;
    for raw in text.split(['\n', '\r']) {
        let Some((command, param)) = statement(raw) else {
            continue;
        };
        seq = seq.saturating_add(1);
        if starts_with(command, "IF") {
            let param = glued(command, "IF", param);
            if skipped.len() >= 256 {
                continue;
            }
            let parent = skipped.last().copied().unwrap_or(false);
            let skip = parent || int(param).unwrap_or(1) != random;
            if skip && !parent {
                file.report.branches_skipped += 1;
            }
            skipped.push(skip);
        } else if starts_with(command, "ENDIF") {
            if skipped.len() > 1 {
                skipped.pop();
            }
        } else if skipped.last().copied().unwrap_or(false) {
        } else if let Some((measure, channel)) = data_command(command) {
            data_line(&mut file, seq, measure, channel, param);
        } else if starts_with(command, "RANDOM") {
            // The game draws 1..=n at random; the decoder always draws 1 and does not read n.
            random = 1;
        } else {
            header(
                &mut file,
                &mut wav,
                &mut volume,
                &mut path,
                seq,
                command,
                param,
            );
        }
    }
    let mut refused = 0;
    let resolve = |id: u16, seq: u32, name: String, refused: &mut usize| -> Sound {
        // The name must be safe by itself, and so must its folder and it together.
        let full =
            safe_name(&name, refused).and_then(|name| safe_name(&format!("{path}{name}"), refused));
        Sound {
            name: full.unwrap_or_default(),
            volume: volume.at(id, seq).copied().unwrap_or(100),
        }
    };
    file.wav = wav.map(|id, seq, name| resolve(id, seq, name, &mut refused));
    for key in ["STAGEFILE", "PREIMAGE", "BACKGROUND", "BACKGROUND_GR"] {
        if let Some(name) = file.headers.get_mut(key) {
            *name = safe_name(name, &mut refused).unwrap_or_default();
        }
    }
    file.report.unsafe_names = refused;
    file
}

/// `param`, or the part of `command` after `name` when it goes on (the game's `t入力_パラメータ食い込みチェック`).
fn glued<'a>(command: &'a str, name: &str, param: &'a str) -> &'a str {
    command
        .get(name.len()..)
        .filter(|rest| !rest.is_empty())
        .map_or(param, str::trim)
}

fn header(
    file: &mut DtxFile,
    wav: &mut Table<String>,
    volume: &mut Table<u8>,
    path: &mut String,
    seq: u32,
    command: &str,
    param: &str,
) {
    if let Some(name) = NAMES.iter().find(|name| starts_with(command, name)) {
        let param = glued(command, name, param);
        match *name {
            "PATH_WAV" => file.report.path_wav_ignored |= !param.is_empty(),
            "PATH" => {
                *path = if param == "PATH_WAV" {
                    String::new()
                } else {
                    param.to_owned()
                }
            }
            "TITLE" => text(file, "TITLE", param),
            "ARTIST" => text(file, "ARTIST", param),
            "GENRE" => text(file, "GENRE", param),
            "STAGEFILE" => text(file, "STAGEFILE", param),
            "PREIMAGE" => text(file, "PREIMAGE", param),
            "BACKGROUND_GR" => text(file, "BACKGROUND_GR", param),
            "BACKGROUND" | "WALL" => text(file, "BACKGROUND", param),
            "DLEVEL" | "PLAYLEVEL" => level(&mut file.levels[0], param),
            "GLEVEL" => level(&mut file.levels[1], param),
            "BLEVEL" => level(&mut file.levels[2], param),
            "DLVDEC" => file.levels[0].decimal = decimal(param, file.levels[0].decimal),
            "GLVDEC" => file.levels[1].decimal = decimal(param, file.levels[1].decimal),
            "BLVDEC" => file.levels[2].decimal = decimal(param, file.levels[2].decimal),
            "DTXVPLAYSPEED" => {
                file.report.dtxv_play_speed_ignored |= real(param).is_some_and(|s| s > 0.0);
            }
            "BASEBPM" => {
                if let Some(base) = real(param).filter(|b| *b > 0.0) {
                    file.base_bpm = Some(base);
                }
            }
            // Read by the game, not by this decoder.
            _ => {}
        }
    } else if starts_with(command, "BPM") {
        let rest = &command[3..];
        let id = if rest.len() < 2 {
            Some(0)
        } else {
            id36(rest.as_bytes())
        };
        if let (Some(id), Some(bpm)) = (id, real(param).filter(|b| *b > 0.0)) {
            file.bpm.push(id, seq, bpm);
        }
    } else if let Some(rest) =
        strip_prefix(command, "WAVVOL").or_else(|| strip_prefix(command, "VOLUME"))
    {
        if let (Some(id), Some(percent)) = (id36(rest.as_bytes()), int(param)) {
            volume.push(id, seq, u8::try_from(percent.clamp(0, 100)).unwrap_or(100));
        }
    } else if starts_with(command, "PANEL")
        || starts_with(command, "WAVPAN")
        || starts_with(command, "PAN")
    {
        // The pan of a sample is not used: an audio plan has no pan.
    } else if let Some(rest) = strip_prefix(command, "WAV")
        && let Some(id) = id36(rest.as_bytes())
    {
        wav.push(id, seq, param.to_owned());
    }
}

fn text(file: &mut DtxFile, key: &'static str, param: &str) {
    file.headers.insert(key, param.to_owned());
}

/// `command` without its start `name`, ignoring the case.
fn strip_prefix<'a>(command: &'a str, name: &str) -> Option<&'a str> {
    starts_with(command, name).then(|| &command[name.len()..])
}

/// `#DLEVEL` and the like: 0..=1000, clamped; from 100 on, the level is the tenths and the rest the decimal part.
fn level(level: &mut Level, param: &str) {
    let Some(value) = int(param) else {
        return;
    };
    let value = value.clamp(0, 1000);
    if value >= 100 {
        level.value = Some(value / 10);
        level.decimal = value - value / 10 * 10;
    } else {
        level.value = Some(value);
    }
}

/// `#DLVDEC` and the like: 0..=10, clamped; the value in place when the parameter is not a number.
fn decimal(param: &str, current: i32) -> i32 {
    int(param).map_or(current, |v| v.clamp(0, 10))
}

/// One `#mmmcc: objects` line.
fn data_line(file: &mut DtxFile, seq: u32, measure: u16, channel: u8, param: &str) {
    if channel == 0x02 {
        match real(param).filter(|length| *length > 0.0) {
            Some(length) => {
                file.measure_len.insert(measure, length);
            }
            None => file.report.ignored_bar_lengths += 1,
        }
        return;
    }
    let digits: Vec<u8> = param.bytes().filter(|b| *b != b'_').collect();
    if digits.is_empty() {
        return;
    }
    if digits.iter().any(|b| !b.is_ascii_alphanumeric()) {
        file.report.invalid_data_lines += 1;
        return;
    }
    let mut count = digits.len();
    if count % 2 == 1 {
        file.report.odd_data_lines += 1;
        count -= 1;
    }
    let slots = count / 2;
    let role = classify(channel);
    #[allow(clippy::chunks_exact_to_as_chunks)]
    // `as_chunks` is newer than the toolchain this crate supports
    for (i, pair) in digits[..count].chunks_exact(2).enumerate() {
        let id = if channel == 0x03 {
            let (Some(high), Some(low)) = (hex(pair[0]), hex(pair[1])) else {
                file.report.ignored_bpm_changes += 1;
                continue;
            };
            high * 16 + low
        } else {
            let Some(id) = id36(pair) else {
                continue;
            };
            id
        };
        if id == 0 {
            continue;
        }
        if let Role::Dropped(why) = role {
            let counter = match why {
                Dropped::Hidden => &mut file.report.hidden_chips,
                Dropped::BlankHit => &mut file.report.blank_hit_sounds,
                Dropped::Wailing => &mut file.report.wailing_chips,
                Dropped::FillIn => &mut file.report.fill_in_chips,
                Dropped::Visual => &mut file.report.visual_chips,
                Dropped::Inert => &mut file.report.inert_chips,
                Dropped::Unknown => &mut file.report.unknown_chips,
            };
            *counter += 1;
            continue;
        }
        let tick = u64::try_from(i).unwrap_or(0) * TICKS / u64::try_from(slots).unwrap_or(1);
        file.chips.push(Chip {
            measure,
            tick: u16::try_from(tick).unwrap_or(0),
            channel,
            id,
            seq,
        });
    }
}
