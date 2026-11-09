#![warn(clippy::pedantic)]
//! `.ksh` decoder: the header, the measures of chart lines and the in-measure options to a 6-column ROX chart. See `mod.rs` for the decisions.

use std::path::Path;

use encoding_rs::{GBK, SHIFT_JIS, UTF_8};
use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, Note, RoxChart, TimingPoint};

use crate::formats::bms::bmson::safe_name;

/// Decoder of `.ksh` files (`K-Shoot MANIA`, the SOUND VOLTEX simulator): one chart per file.
pub struct KshDecoder;
crate::impl_format!(KshDecoder, ["ksh"]);

/// Columns of the chart: the four BT buttons, then the two FX buttons.
pub const COLUMNS: u8 = 6;
/// Files above this size are refused (a real one is a few hundred kilobytes).
pub(super) const MAX_FILE_SIZE: usize = 16 * 1024 * 1024;
/// Chart lines above this count are refused (a real chart has some tens of thousands).
const MAX_CHART_LINES: usize = 500_000;
/// Largest BPM read (a BPM is a `f32` in the model).
const MAX_BPM: f64 = 10_000_000.0;
/// Notes later than this, in microseconds (about 115 days), are refused: nothing can play them.
const MAX_TIME_US: f64 = 1e13;
/// Largest `o` read, in milliseconds.
const MAX_OFFSET_MS: i64 = 1_000_000_000;
/// The BPM when neither the header nor the body gives one (the format's default).
const DEFAULT_BPM: f64 = 120.0;
/// Microseconds in a minute: a beat at `bpm` lasts `MINUTE_US / bpm`.
const MINUTE_US: f64 = 60_000_000.0;
/// A laser segment this short or shorter, in whole notes, is a slam (an instant move).
const SLAM_WHOLE_NOTES: f64 = 1.0 / 32.0;
/// The 51 positions of a laser, from left to right.
const LASER_POSITIONS: &[u8; 51] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmno";
/// The background presets of the format: names, not files.
const BACKGROUND_PRESETS: [&str; 13] = [
    "desert", "grass", "night", "deepsea", "sky", "sunset", "ocean", "flame", "cyber", "space",
    "mars", "cloudy", "fantasy",
];
/// The jacket presets of the format: names, not files.
const JACKET_PRESETS: [&str; 3] = ["nowprinting1", "nowprinting2", "nowprinting3"];

/// What the decoder dropped, simplified or repaired in a file. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // flags and counters of one file
pub struct KshReport {
    /// The charset the text was read with: `UTF-8`, `Shift_JIS`, `GBK` or `Shift_JIS (lossy)`.
    pub charset: &'static str,
    /// Measures of the body (a bar line `--` ends each one).
    pub measures: usize,
    /// Chart lines of the body.
    pub chart_lines: usize,
    /// The last measure had no bar line after it: it is read all the same.
    pub unterminated_last_measure: bool,

    /// Tempo changes applied (the first tempo included).
    pub bpm_changes: usize,
    /// Time signature changes applied (the first one included when it is not 4/4).
    pub time_signature_changes: usize,
    /// The header `t` is a range string (`180-220`): the tempo is the first `t=` of the body, else the start of the range.
    pub header_bpm_range: bool,
    /// `t` values that are not a number in `(0, 1e7]` (a range in the body, a negative BPM, NaN): ignored.
    pub invalid_bpms: usize,
    /// `beat` values that are not `n/d` with both above 0: ignored.
    pub invalid_time_signatures: usize,
    /// Options with a value this reader can not use (`o`, `level`, `po`, `plength`): the default is used.
    pub invalid_option_values: usize,
    /// Measures with no chart line (they last their time signature at the BPM in force).
    pub empty_measures: usize,

    /// BT chips (`1`).
    pub bt_chips: usize,
    /// BT holds (a run of `2`).
    pub bt_holds: usize,
    /// FX chips (`2`).
    pub fx_chips: usize,
    /// FX holds (a run of any character other than `0` and `2`).
    pub fx_holds: usize,
    /// FX holds written with a legacy letter (`S`, `G`, `F`...): the audio effect it names is dropped.
    pub fx_legacy_effect_holds: usize,
    /// Holds still open at the end of the last measure: they end there.
    pub holds_open_at_end: usize,
    /// Holds whose length rounds to 0 µs (a hold of the model has a length): kept as taps.
    pub holds_as_taps: usize,
    /// BT characters other than `0`, `1`, `2`: read as empty.
    pub unknown_buttons: usize,

    /// Laser notes (a run of laser characters in one of the two lanes): dropped.
    pub laser_notes: usize,
    /// Laser slams (two positions of a laser 1/32 of a whole note or less apart, an instant move): dropped.
    pub laser_slams: usize,
    /// Laser characters that are neither `-`, `:` nor one of the 51 positions (or a `:` with no laser before it): end the laser, counted.
    pub invalid_laser_chars: usize,
    /// Lane spins `@(` and `@)`: dropped.
    pub lane_spins: usize,
    /// Half spins `@<` and `@>`: dropped.
    pub half_spins: usize,
    /// Swings `S<` and `S>`: dropped.
    pub swings: usize,
    /// Text after the laser characters that is none of the six lane-spin forms: dropped.
    pub unknown_spin_suffixes: usize,
    /// `laserrange_l`, `laserrange_r`, `laser_l_curve`, `laser_r_curve` lines: dropped.
    pub laser_options: usize,

    /// `fx-l`, `fx-r`, `fx-l_param1`, `fx-r_param1`, `fx-l_se`, `fx-r_se` lines (audio effects and key sounds of FX buttons), and `fx:<effect>:<parameter>` lines
    /// (a parameter of a user-defined FX effect, which `libkson` reads and the format's own list does not give): dropped.
    pub fx_options: usize,
    /// `stop` lines: a stop freezes the SCROLL, not the song, so no time moves (see `mod.rs`).
    pub stops: usize,
    /// `scroll_speed`, `rotation_deg`, `tilt`, `zoom_*`, `center_split` lines and their `*_curve` options (camera and scroll): dropped.
    pub scroll_options: usize,
    /// `chokkakuvol`, `chokkakuse`, `pfiltergain`, `filtertype`, the deprecated gauge-rate lines and `filter:<effect>:<parameter>` lines (a parameter of a user-defined laser
    /// effect, as `fx:` above) in the body: dropped.
    pub audio_options: usize,
    /// Option lines whose name is not one of the format's: dropped.
    pub unknown_options: usize,
    /// `#define_fx` and `#define_filter` lines: dropped.
    pub definitions: usize,
    /// Comment lines (`//`, and the unofficial `;`).
    pub comments: usize,
    /// Lines that are none of the line types (and have no `|`): dropped.
    pub unknown_lines: usize,

    /// Files of `m` beyond the first (the effect variants of the song): not read.
    pub extra_audio_files: usize,
    /// File names that are absolute or climb out of the folder: refused.
    pub unsafe_names: usize,
    /// `difficulty` is not one of the four names of the format: read as `infinite`, as the format says.
    pub unknown_difficulty: bool,
}

/// One option line that moves time, kept with the chart line it comes before.
#[derive(Clone, Copy)]
enum Timing {
    Bpm(f64),
    Beat(u32, u32),
}

/// A chart line: its characters and the timing options written before it.
struct ChartLine {
    bt: [u8; 4],
    fx: [u8; 2],
    laser: [u8; 2],
    timing: Vec<Timing>,
}

/// A button held since `start_us`.
#[derive(Clone, Copy)]
struct Held {
    start_us: f64,
    legacy: bool,
}

/// A note before its times are rounded.
struct Draft {
    start_us: f64,
    end_us: Option<f64>,
    column: u8,
}

/// The state of one laser lane, only to count what is dropped.
#[derive(Clone, Copy, Default)]
struct Laser {
    /// A laser note is running.
    running: bool,
    /// Whole notes since the last position of the run.
    span: f64,
}

/// The tempo and the time signature from a time on.
struct Event {
    time_us: f64,
    bpm: f64,
    quarter_beats: u8,
}

/// Where the notes are in time: a walk over the measures.
struct Walk {
    /// Microseconds of the start of the next measure (or of the next line).
    now_us: f64,
    bpm: f64,
    /// Time signature of the measure being read.
    signature: (u32, u32),
    /// The signature the report has counted up to now (4/4 at the start).
    counted_signature: (u32, u32),
    /// Time signature from the next measure on, set by a `beat` after the first line of a measure.
    next_signature: Option<(u32, u32)>,
    events: Vec<Event>,
    held: [Option<Held>; COLUMNS as usize],
    lasers: [Laser; 2],
    drafts: Vec<Draft>,
}

fn invalid(message: impl std::fmt::Display) -> RoxError {
    RoxError::InvalidFormat(format!("ksh: {message}"))
}

/// The text of the file: a UTF-8 byte order mark, else valid UTF-8, else `Shift_JIS` (CP932), else GBK (CP936), else `Shift_JIS` with
/// U+FFFD for what it can not read.
fn text_of(data: &[u8]) -> (std::borrow::Cow<'_, str>, &'static str) {
    if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return (UTF_8.decode_with_bom_removal(data).0, "UTF-8");
    }
    if let Ok(text) = std::str::from_utf8(data) {
        return (text.into(), "UTF-8");
    }
    if let Some(text) = SHIFT_JIS.decode_without_bom_handling_and_without_replacement(data) {
        return (text, "Shift_JIS");
    }
    if let Some(text) = GBK.decode_without_bom_handling_and_without_replacement(data) {
        return (text, "GBK");
    }
    (
        SHIFT_JIS.decode_without_bom_handling(data).0,
        "Shift_JIS (lossy)",
    )
}

/// The lines of `text`, numbered from 1: a line ends at `\n`, `\r` or `\r\n`.
fn lines_of(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let bytes = text.as_bytes();
    let mut at = 0;
    let mut number = 0;
    std::iter::from_fn(move || {
        if at > bytes.len() {
            return None;
        }
        let start = at;
        let end = bytes[start..]
            .iter()
            .position(|&byte| byte == b'\n' || byte == b'\r')
            .map_or(bytes.len(), |offset| start + offset);
        at = if end == bytes.len() {
            end + 1
        } else if bytes[end] == b'\r' && bytes.get(end + 1) == Some(&b'\n') {
            end + 2
        } else {
            end + 1
        };
        number += 1;
        // `\r` and `\n` are ASCII: both ends are character boundaries.
        Some((number, &text[start..end]))
    })
}

/// A BPM: a finite number in `(0, 1e7]`.
fn bpm_of(value: &str) -> Option<f64> {
    value
        .trim_ascii()
        .parse::<f64>()
        .ok()
        .filter(|bpm| bpm.is_finite() && *bpm > 0.0 && *bpm <= MAX_BPM)
}

/// A time signature `n/d`, both whole numbers above 0.
fn signature_of(value: &str) -> Option<(u32, u32)> {
    let (numerator, denominator) = value.split_once('/')?;
    let numerator = numerator.trim_ascii().parse::<u32>().ok()?;
    let denominator = denominator.trim_ascii().parse::<u32>().ok()?;
    (numerator > 0 && denominator > 0).then_some((numerator, denominator))
}

/// Beats per measure in quarter notes, as the model counts them: `4 n / d` rounded, at least 1, at most 255.
fn quarter_beats(signature: (u32, u32)) -> u8 {
    let beats = (u64::from(signature.0) * 4 + u64::from(signature.1) / 2) / u64::from(signature.1);
    u8::try_from(beats.clamp(1, 255)).unwrap_or(u8::MAX)
}

/// What a body option line is, by its name.
enum OptionKind {
    Timing,
    Stop,
    Scroll,
    Laser,
    Fx,
    Audio,
    Unknown,
}

fn option_kind(key: &str) -> OptionKind {
    match key {
        "t" | "beat" => OptionKind::Timing,
        "stop" => OptionKind::Stop,
        "scroll_speed" | "rotation_deg" | "tilt" | "zoom_top" | "zoom_bottom" | "zoom_side"
        | "center_split" => OptionKind::Scroll,
        "laserrange_l" | "laserrange_r" | "laser_l_curve" | "laser_r_curve" => OptionKind::Laser,
        "fx-l" | "fx-r" | "fx-l_param1" | "fx-r_param1" | "fx-l_se" | "fx-r_se" => OptionKind::Fx,
        _ if key.starts_with("fx:") => OptionKind::Fx,
        _ if key.starts_with("filter:") => OptionKind::Audio,
        "chokkakuvol" | "chokkakuse" | "pfiltergain" | "filtertype" | "chiprate" | "longrate"
        | "laserrate" => OptionKind::Audio,
        _ if key.ends_with("_curve") => OptionKind::Scroll,
        _ => OptionKind::Unknown,
    }
}

/// The header options the format has (read or not), to tell a typo from a real option.
const HEADER_KEYS: [&str; 32] = [
    "title",
    "title_translit",
    "title_img",
    "artist",
    "artist_translit",
    "artist_img",
    "effect",
    "jacket",
    "illustrator",
    "difficulty",
    "level",
    "t",
    "to",
    "beat",
    "m",
    "mvol",
    "o",
    "bg",
    "layer",
    "po",
    "plength",
    "total",
    "chokkakuvol",
    "chokkakuautovol",
    "filtertype",
    "pfiltergain",
    "pfilterdelay",
    "v",
    "vo",
    "ver",
    "ver_compat",
    "information",
];

/// The header: the options read, last of each name winning.
#[derive(Default)]
struct Header {
    options: Vec<(String, String)>,
}

impl Header {
    fn get(&self, key: &str) -> Option<&str> {
        self.options
            .iter()
            .rev()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }
}

/// The ASCII shape of a chart line: `bbbb|ff|ll`, then anything.
fn is_chart_shaped(line: &[u8]) -> bool {
    line.len() >= 10 && line[4] == b'|' && line[7] == b'|'
}

fn decode(data: &[u8], fallback_title: &str) -> RoxResult<(RoxChart, KshReport)> {
    if data.len() > MAX_FILE_SIZE {
        return Err(invalid("file too large"));
    }
    let (text, charset) = text_of(data);
    let mut report = KshReport { charset, ..KshReport::default() };
    let mut lines = lines_of(&text);
    let header = read_header_lines(&mut lines, &mut report)?;
    let (metadata, offset_ms, mut walk) = read_header(&header, fallback_title, &mut report);
    #[allow(clippy::cast_precision_loss)] // |o| <= 1e9 ms
    {
        walk.now_us = offset_ms as f64 * 1000.0;
    }
    read_body(lines, &mut walk, &mut report)?;
    walk.finish(&mut report);

    let mut chart = RoxChart::new(COLUMNS);
    chart.metadata = metadata;
    chart.notes = notes_of(walk.drafts, &mut report);
    if chart.notes.is_empty() {
        return Err(invalid("no playable note"));
    }
    chart.timing_points = timing_points(&walk.events);
    chart.validate()?;
    Ok((chart, report))
}

/// Whether a line is a comment (counted).
fn is_comment(line: &str, report: &mut KshReport) -> bool {
    let comment = line.starts_with("//") || line.starts_with(';');
    report.comments += usize::from(comment);
    comment
}

/// The header: option lines up to the first bar line (a file without one is not a chart).
fn read_header_lines<'a>(lines: &mut impl Iterator<Item = (usize, &'a str)>, report: &mut KshReport) -> RoxResult<Header> {
    let mut header = Header::default();
    for (_, raw) in lines {
        let line = raw.trim_ascii();
        if line == "--" {
            return Ok(header);
        }
        if line.is_empty() || is_comment(line, report) {
            continue;
        }
        if let Some((key, value)) = line.split_once('=').filter(|(key, _)| !key.is_empty()) {
            report.unknown_options += usize::from(!HEADER_KEYS.contains(&key));
            header.options.push((key.to_owned(), value.to_owned()));
        } else if line.starts_with("#define_") {
            report.definitions += 1;
        } else {
            report.unknown_lines += 1;
        }
    }
    Err(invalid("no bar line `--`: not a K-Shoot MANIA chart"))
}

/// The body: measures of chart lines, each ended by a bar line, walked as they end (a last measure without one too).
fn read_body<'a>(lines: impl Iterator<Item = (usize, &'a str)>, walk: &mut Walk, report: &mut KshReport) -> RoxResult<()> {
    let mut measure: Vec<ChartLine> = Vec::new();
    let mut pending: Vec<Timing> = Vec::new();
    for (number, raw) in lines {
        let line = raw.trim_ascii();
        if line.is_empty() || is_comment(line, report) {
            continue;
        }
        if line == "--" {
            report.measures += 1;
            walk.measure(&measure, report)?;
            measure.clear();
        } else if line.starts_with('#') {
            if line.starts_with("#define_") {
                report.definitions += 1;
            } else {
                report.unknown_lines += 1;
            }
        } else if let Some((key, value)) = line.split_once('=') {
            read_option(key, value, &mut pending, report);
        } else if is_chart_shaped(line.as_bytes()) {
            report.chart_lines += 1;
            if report.chart_lines > MAX_CHART_LINES {
                return Err(invalid("too many chart lines"));
            }
            measure.push(chart_line(line.as_bytes(), std::mem::take(&mut pending), report));
        } else if line.contains('|') {
            return Err(invalid(format!("line {number} is not a chart line (`bbbb|ff|ll`)")));
        } else {
            report.unknown_lines += 1;
        }
    }
    if !measure.is_empty() {
        report.unterminated_last_measure = true;
        report.measures += 1;
        walk.measure(&measure, report)?;
    }
    Ok(())
}

/// The tempo the header gives: `t` (120 when absent), or the start of the range string a chart with tempo changes has.
fn header_bpm(header: &Header, report: &mut KshReport) -> f64 {
    let Some(value) = header.get("t") else {
        return DEFAULT_BPM;
    };
    if let Some(bpm) = bpm_of(value) {
        return bpm;
    }
    let value = value.trim_ascii();
    // `180-220`: the tempos of the chart; the first `t=` of the body is the real one, the start of the range is the fallback.
    let range_start = value.char_indices().skip(1).find(|(_, c)| *c == '-').map(|(at, _)| &value[..at]);
    if let Some(start) = range_start {
        report.header_bpm_range = true;
        if let Some(bpm) = bpm_of(start) {
            return bpm;
        }
    }
    report.invalid_bpms += 1;
    DEFAULT_BPM
}

impl Header {
    fn text(&self, key: &str) -> &str {
        self.get(key).map_or("", str::trim_ascii)
    }

    /// An integer option in `low..=high`; `default` when absent, and when invalid (counted).
    fn number(&self, key: &str, default: i64, (low, high): (i64, i64), report: &mut KshReport) -> i64 {
        self.get(key).map_or(default, |value| {
            value.trim_ascii().parse::<i64>().ok().filter(|parsed| (low..=high).contains(parsed)).unwrap_or_else(|| {
                report.invalid_option_values += 1;
                default
            })
        })
    }

    /// The first file of an image option, unless it names one of the game's presets.
    fn image(&self, key: &str, presets: &[&str], report: &mut KshReport) -> Option<String> {
        let first = self.text(key).split(';').next().unwrap_or("").trim_ascii();
        if presets.contains(&first) {
            return None;
        }
        safe_name(first, &mut report.unsafe_names)
    }
}

/// The difficulty name; one the format does not know reads as `Infinite` (counted).
fn difficulty_of(header: &Header, report: &mut KshReport) -> &'static str {
    match header.text("difficulty").to_ascii_lowercase().as_str() {
        "" | "light" => "Light",
        "challenge" => "Challenge",
        "extended" => "Extended",
        "infinite" => "Infinite",
        _ => {
            report.unknown_difficulty = true;
            "Infinite"
        }
    }
}

/// The metadata and the starting state of the walk from the header options.
fn read_header(header: &Header, fallback_title: &str, report: &mut KshReport) -> (Metadata, i64, Walk) {
    let offset_ms = header.number("o", 0, (-MAX_OFFSET_MS, MAX_OFFSET_MS), report);
    let level = header.number("level", 1, (1, 20), report);
    let preview_ms = header.number("po", 0, (0, MAX_OFFSET_MS), report);
    let preview_length_ms = header.number("plength", 0, (0, MAX_OFFSET_MS), report);
    let difficulty = difficulty_of(header, report);

    let mut files = header.text("m").split(';').map(str::trim_ascii);
    let audio_file = files.next().and_then(|name| safe_name(name, &mut report.unsafe_names)).unwrap_or_default();
    report.extra_audio_files = files.filter(|name| !name.is_empty()).count();
    let background_file =
        header.image("bg", &BACKGROUND_PRESETS, report).or_else(|| header.image("jacket", &JACKET_PRESETS, report));

    let signature = header.get("beat").map_or((4, 4), |value| {
        signature_of(value).unwrap_or_else(|| {
            report.invalid_time_signatures += 1;
            (4, 4)
        })
    });
    let bpm = header_bpm(header, report);

    let title = header.text("title");
    #[allow(clippy::cast_precision_loss)] // 1..=20
    let metadata = Metadata {
        title: if title.is_empty() { fallback_title } else { title }.into(),
        artist: header.text("artist").into(),
        creator: header.text("effect").into(),
        difficulty_name: difficulty.into(),
        difficulty_value: Some(level as f32),
        audio_file: audio_file.into(),
        background_file: background_file.map(Into::into),
        // The offset is applied to the times themselves, see `mod.rs`.
        audio_offset_us: 0,
        preview_time_us: preview_ms * 1000,
        preview_duration_us: preview_length_ms * 1000,
        ..Metadata::default()
    };
    let walk = Walk {
        now_us: 0.0,
        bpm,
        signature,
        counted_signature: (4, 4),
        next_signature: None,
        events: Vec::new(),
        held: [None; COLUMNS as usize],
        lasers: [Laser::default(); 2],
        drafts: Vec::new(),
    };
    (metadata, offset_ms, walk)
}

/// A body option line: the ones that move time are kept for the next chart line, the others are counted.
fn read_option(key: &str, value: &str, pending: &mut Vec<Timing>, report: &mut KshReport) {
    match option_kind(key) {
        OptionKind::Timing if key == "t" => match bpm_of(value) {
            Some(bpm) => pending.push(Timing::Bpm(bpm)),
            None => report.invalid_bpms += 1,
        },
        OptionKind::Timing => match signature_of(value) {
            Some((numerator, denominator)) => pending.push(Timing::Beat(numerator, denominator)),
            None => report.invalid_time_signatures += 1,
        },
        OptionKind::Stop => report.stops += 1,
        OptionKind::Scroll => report.scroll_options += 1,
        OptionKind::Laser => report.laser_options += 1,
        OptionKind::Fx => report.fx_options += 1,
        OptionKind::Audio => report.audio_options += 1,
        OptionKind::Unknown => report.unknown_options += 1,
    }
}

/// A chart line (`bbbb|ff|ll` and maybe a lane spin; the shape was checked) with its timing options; counts the lane spin.
fn chart_line(line: &[u8], timing: Vec<Timing>, report: &mut KshReport) -> ChartLine {
    match line.get(10..) {
        Some([]) | None => {}
        Some(suffix) => match &suffix[..2.min(suffix.len())] {
            b"@(" | b"@)" => report.lane_spins += 1,
            b"@<" | b"@>" => report.half_spins += 1,
            b"S<" | b"S>" => report.swings += 1,
            _ => report.unknown_spin_suffixes += 1,
        },
    }
    ChartLine {
        bt: [line[0], line[1], line[2], line[3]],
        fx: [line[5], line[6]],
        laser: [line[8], line[9]],
        timing,
    }
}

/// The legacy letters of a long FX note (each named an audio effect before v1.60).
fn is_legacy_fx_letter(byte: u8) -> bool {
    b"SVTWUGHKIJLFPBQXAD".contains(&byte)
}

impl Walk {
    /// Walks one measure: the time of each of its lines, the notes they start and end.
    fn measure(&mut self, lines: &[ChartLine], report: &mut KshReport) -> RoxResult<()> {
        if let Some(next) = self.next_signature.take() {
            self.signature = next;
        }
        // A `beat` before the first line is "right after the bar line": it is the signature of this measure.
        for timing in lines.first().map_or(&[][..], |line| &line.timing[..]) {
            if let Timing::Beat(numerator, denominator) = *timing {
                self.signature = (numerator, denominator);
            }
        }
        if self.signature != self.counted_signature {
            report.time_signature_changes += 1;
            self.counted_signature = self.signature;
        }
        let whole_notes = f64::from(self.signature.0) / f64::from(self.signature.1);
        if lines.is_empty() {
            report.empty_measures += 1;
            self.mark();
            self.now_us += whole_notes * 4.0 * MINUTE_US / self.bpm;
            return self.check_time();
        }
        // The lines share the measure evenly: each lasts `1 / n` of it, at the tempo in force from it.
        #[allow(clippy::cast_precision_loss)] // at most 500 000 lines
        let count = lines.len() as f64;
        for (index, line) in lines.iter().enumerate() {
            for timing in &line.timing {
                match *timing {
                    Timing::Bpm(bpm) => {
                        self.bpm = bpm;
                        report.bpm_changes += 1;
                    }
                    // Index 0 was read above; later ones apply from the next measure.
                    Timing::Beat(numerator, denominator) if index > 0 => {
                        self.next_signature = Some((numerator, denominator));
                    }
                    Timing::Beat(..) => {}
                }
            }
            self.mark();
            self.notes(line, whole_notes / count, report);
            self.now_us += whole_notes / count * 4.0 * MINUTE_US / self.bpm;
            self.check_time()?;
        }
        Ok(())
    }

    fn check_time(&self) -> RoxResult<()> {
        if self.now_us.abs() <= MAX_TIME_US {
            Ok(())
        } else {
            Err(invalid("the chart lasts more than 1e7 seconds"))
        }
    }

    /// Records the tempo and the signature in force now, when they are not the ones of the last record.
    fn mark(&mut self) {
        let quarter_beats = quarter_beats(self.signature);
        if self.events.last().is_none_or(|last| {
            last.bpm.to_bits() != self.bpm.to_bits() || last.quarter_beats != quarter_beats
        }) {
            self.events.push(Event {
                time_us: self.now_us,
                bpm: self.bpm,
                quarter_beats,
            });
        }
    }

    /// The notes of one chart line, at `now_us`; `spacing` is the line's length in whole notes.
    fn notes(&mut self, line: &ChartLine, spacing: f64, report: &mut KshReport) {
        let at = self.now_us;
        for (lane, &byte) in line.bt.iter().enumerate() {
            if byte == b'2' {
                self.hold(lane, at, false);
                continue;
            }
            if byte != b'0' && byte != b'1' {
                report.unknown_buttons += 1;
            }
            self.release(lane, at, report);
            if byte == b'1' {
                self.chip(lane, at);
                report.bt_chips += 1;
            }
        }
        for (side, &byte) in line.fx.iter().enumerate() {
            let column = 4 + side;
            match byte {
                b'0' => self.release(column, at, report),
                b'2' => {
                    self.release(column, at, report);
                    self.chip(column, at);
                    report.fx_chips += 1;
                }
                _ => self.hold(column, at, is_legacy_fx_letter(byte)),
            }
        }
        for (laser, &byte) in self.lasers.iter_mut().zip(&line.laser) {
            match byte {
                b'-' => laser.running = false,
                b':' if laser.running => {}
                byte if LASER_POSITIONS.contains(&byte) => {
                    if !laser.running {
                        laser.running = true;
                        report.laser_notes += 1;
                    } else if laser.span <= SLAM_WHOLE_NOTES + 1e-9 {
                        report.laser_slams += 1;
                    }
                    laser.span = 0.0;
                }
                _ => {
                    laser.running = false;
                    report.invalid_laser_chars += 1;
                }
            }
            if laser.running {
                laser.span += spacing;
            }
        }
    }

    /// `lane` is held from `at`: starts the hold, or goes on with the one that runs.
    fn hold(&mut self, lane: usize, at: f64, legacy: bool) {
        match &mut self.held[lane] {
            Some(held) => held.legacy |= legacy,
            slot => {
                *slot = Some(Held {
                    start_us: at,
                    legacy,
                });
            }
        }
    }

    /// Ends the hold of `lane` at `at`, if there is one.
    fn release(&mut self, lane: usize, at: f64, report: &mut KshReport) {
        let Some(held) = self.held[lane].take() else {
            return;
        };
        if lane < 4 {
            report.bt_holds += 1;
        } else {
            report.fx_holds += 1;
            report.fx_legacy_effect_holds += usize::from(held.legacy);
        }
        #[allow(clippy::cast_possible_truncation)] // a lane is below 6
        self.drafts.push(Draft {
            start_us: held.start_us,
            end_us: Some(at),
            column: lane as u8,
        });
    }

    fn chip(&mut self, lane: usize, at: f64) {
        #[allow(clippy::cast_possible_truncation)] // a lane is below 6
        self.drafts.push(Draft {
            start_us: at,
            end_us: None,
            column: lane as u8,
        });
    }

    /// The end of the body: the holds still open end with the last measure.
    fn finish(&mut self, report: &mut KshReport) {
        for lane in 0..self.held.len() {
            if self.held[lane].is_some() {
                report.holds_open_at_end += 1;
                self.release(lane, self.now_us, report);
            }
        }
    }
}

/// The notes of the chart, times rounded to microseconds, sorted by time then column.
#[allow(clippy::cast_possible_truncation)] // times are within +-1e13 µs (checked while walking)
fn notes_of(drafts: Vec<Draft>, report: &mut KshReport) -> Vec<Note> {
    let mut notes: Vec<Note> = drafts
        .into_iter()
        .map(|draft| {
            let start = draft.start_us.round() as i64;
            match draft.end_us.map(|end| end.round() as i64 - start) {
                Some(length) if length > 0 => Note::hold(start, length, draft.column),
                Some(_) => {
                    report.holds_as_taps += 1;
                    Note::tap(start, draft.column)
                }
                None => Note::tap(start, draft.column),
            }
        })
        .collect();
    notes.sort_by_key(|note| (note.time_us, note.column));
    notes
}

/// The tempo and signature records as timing points: one per change, the last of two at the same microsecond.
#[allow(clippy::cast_possible_truncation)] // times are within +-1e13 µs; a BPM is at most 1e7
fn timing_points(events: &[Event]) -> Vec<TimingPoint> {
    let mut points: Vec<(i64, f32, u8)> = Vec::with_capacity(events.len());
    for event in events {
        let point = (
            event.time_us.round() as i64,
            event.bpm as f32,
            event.quarter_beats,
        );
        if points.last().is_some_and(|last| last.0 == point.0) {
            points.pop();
        }
        if points
            .last()
            .is_none_or(|last| last.1.to_bits() != point.1.to_bits() || last.2 != point.2)
        {
            points.push(point);
        }
    }
    points
        .into_iter()
        .map(|(time_us, bpm, signature)| TimingPoint::Bpm {
            time_us,
            bpm,
            signature,
        })
        .collect()
}

impl Decoder for KshDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode(data, "").map(|(chart, _)| chart)
    }
}

impl KshDecoder {
    /// Decodes a `.ksh`; the title is `title` or, when it is absent or empty, `fallback_title`.
    ///
    /// # Errors
    /// Returns an error if the file is too large, has no bar line `--`, has a line that is neither an option, a comment nor a chart line, or has no playable note.
    pub fn decode_with_title(data: &[u8], fallback_title: &str) -> RoxResult<RoxChart> {
        decode(data, fallback_title).map(|(chart, _)| chart)
    }

    /// Decodes a `.ksh` and says what was dropped or repaired.
    ///
    /// # Errors
    /// As [`KshDecoder::decode_with_title`].
    pub fn decode_with_report(data: &[u8]) -> RoxResult<(RoxChart, KshReport)> {
        decode(data, "")
    }

    /// Decodes the `.ksh` at `path`; the file stem is the title when the file gives none.
    ///
    /// # Errors
    /// Returns an error if the file can not be read, is too large, or does not decode.
    pub fn decode_file(path: &Path) -> RoxResult<RoxChart> {
        if std::fs::metadata(path)?.len() > MAX_FILE_SIZE as u64 {
            return Err(invalid("file too large"));
        }
        Self::decode_with_title(
            &std::fs::read(path)?,
            &crate::formats::ssq::decoder::title_of(path),
        )
    }
}
