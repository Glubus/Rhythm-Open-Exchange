#![warn(clippy::pedantic)]
//! `.sus` reader: text to a [`Score`] (beats, columns), then to a ROX chart through `score::build`. See `mod.rs` for the decisions.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, RoxChart};

use super::score::{
    BpmChange, MAX_FILE_SIZE, MAX_OBJECTS, Score, ScoreNote, SekaiReport, build, column_of,
    valid_bpm,
};

/// Decoder of `.sus` files (the Project SEKAI dialect of Sliding Universal Score): one chart per file.
pub struct SusDecoder;
crate::impl_format!(SusDecoder, ["sus"]);

/// Ticks per beat when the file gives none (`sus-js`; the notes: "by default, one beat is 480 ticks").
const DEFAULT_TICKS_PER_BEAT: u32 = 480;
/// Beats of a measure before the first `#mmm02` (`sus-js`: 4/4 when there is none).
const DEFAULT_MEASURE_BEATS: f64 = 4.0;
/// Longest measure read, in beats.
const MAX_MEASURE_BEATS: f64 = 1024.0;
/// Leftmost SUS lane of the stage: lanes `2..=13` are the game's 12 lanes (`susToUSC` drops `x <= 1` and `x >= 14`).
const FIRST_LANE: u32 = 2;
/// Lane `f`: the fever markers.
const FEVER_LANE: u32 = 15;
/// Largest `ticks_per_beat` read.
const MAX_TICKS_PER_BEAT: u32 = 1_000_000;
/// Largest `#MEASUREBS` read: with the three digits of a line the measure stays far inside what a `f64` holds.
const MAX_MEASURE_BASE: i64 = 1_000_000;

impl Decoder for SusDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode(data, "").map(|(chart, _)| chart)
    }
}

impl SusDecoder {
    /// Decodes a `.sus`; the title is `#TITLE` or, when it is absent or empty, `fallback_title`.
    ///
    /// # Errors
    /// Returns an error if the file is too large, has no valid BPM or no playable note.
    pub fn decode_with_title(data: &[u8], fallback_title: &str) -> RoxResult<RoxChart> {
        decode(data, fallback_title).map(|(chart, _)| chart)
    }

    /// Decodes a `.sus` and says what was dropped or repaired.
    ///
    /// # Errors
    /// Returns an error if the file is too large, has no valid BPM or no playable note.
    pub fn decode_with_report(data: &[u8]) -> RoxResult<(RoxChart, SekaiReport)> {
        decode(data, "")
    }

    /// Decodes the `.sus` at `path`; the file stem is the title when the file gives none.
    ///
    /// # Errors
    /// Returns an error if the file can not be read, is too large, has no valid BPM or no playable note.
    pub fn decode_file(path: &Path) -> RoxResult<RoxChart> {
        Self::decode_with_title(
            &std::fs::read(path)?,
            &crate::formats::ssq::decoder::title_of(path),
        )
    }
}

fn decode(data: &[u8], fallback_title: &str) -> RoxResult<(RoxChart, SekaiReport)> {
    let mut report = SekaiReport::default();
    let score = parse(data, fallback_title, &mut report)?;
    let chart = build(score, &mut report, "sus")?;
    chart.validate()?;
    Ok((chart, report))
}

/// One `#header: data` line with the measure base in force where it stands.
struct DataLine<'a> {
    header: &'a str,
    data: &'a str,
    base: i64,
}

/// The metadata lines that matter, as written (quotes removed), the last of each kept.
#[derive(Default)]
struct Header<'a> {
    title: &'a str,
    artist: &'a str,
    genre: &'a str,
    designer: &'a str,
    difficulty: &'a str,
    playlevel: Option<&'a str>,
    wave: &'a str,
    background: &'a str,
}

/// The measures: where each one starts, in beats.
struct Bars {
    /// `(first measure, its start in beats, its length in beats)`, sorted; a length holds from its measure until the next entry.
    entries: Vec<(i64, f64, f64)>,
}

impl Bars {
    fn new(lengths: &BTreeMap<i64, f64>) -> Self {
        let mut entries: Vec<(i64, f64, f64)> = Vec::with_capacity(lengths.len());
        for (&measure, &length) in lengths {
            #[allow(clippy::cast_precision_loss)] // measures are below 1 001 000
            let start = match entries.last() {
                None => measure as f64 * DEFAULT_MEASURE_BEATS,
                Some(&(previous, start, previous_length)) => {
                    start + (measure - previous) as f64 * previous_length
                }
            };
            entries.push((measure, start, length));
        }
        Self { entries }
    }

    /// Start (beats) and length (beats) of `measure`.
    #[allow(clippy::cast_precision_loss)] // measures are below 1 001 000
    fn locate(&self, measure: i64) -> (f64, f64) {
        let after = self.entries.partition_point(|entry| entry.0 <= measure);
        match after.checked_sub(1).map(|at| self.entries[at]) {
            None => (
                measure as f64 * DEFAULT_MEASURE_BEATS,
                DEFAULT_MEASURE_BEATS,
            ),
            Some((first, start, length)) => (start + (measure - first) as f64 * length, length),
        }
    }
}

/// A non-empty slot of a data line.
struct Slot {
    beat: f64,
    /// The position in thousandths of a tick: what two objects have in common when they are on the same tick.
    key: i64,
    pair: [u8; 2],
}

/// A note or overlay of a tap, directional, slide or guide line.
struct Raw {
    beat: f64,
    key: i64,
    /// Leftmost SUS lane.
    x: u32,
    width: u32,
    kind: u32,
}

/// What a pass over the lines needs to read a slot.
struct Grid<'a> {
    bars: &'a Bars,
    ticks_per_beat: f64,
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(value)
}

fn digit(byte: u8) -> Option<u32> {
    char::from(byte).to_digit(36)
}

/// The measure number of a header of a note line (`mmm` and at least two more characters).
fn measure_number(header: &str) -> Option<i64> {
    let bytes = header.as_bytes();
    if bytes.len() < 5 || !bytes[..3].iter().all(u8::is_ascii_digit) {
        return None;
    }
    header[..3].parse().ok()
}

/// What the lines of a file say apart from the notes.
struct Directives<'a> {
    header: Header<'a>,
    lines: Vec<DataLine<'a>>,
    /// Seconds added to every time (`-#WAVEOFFSET`).
    shift: f64,
    ticks: u32,
}

/// The objects of a file, by kind, still on the SUS lanes.
#[derive(Default)]
struct Objects {
    bpm: Vec<BpmChange>,
    taps: Vec<Raw>,
    directionals: Vec<Raw>,
    /// Slide points by channel (a lower-case character).
    slides: BTreeMap<u8, Vec<Raw>>,
    guides: BTreeMap<u8, Vec<Raw>>,
}

fn parse(data: &[u8], fallback_title: &str, report: &mut SekaiReport) -> RoxResult<Score> {
    if data.len() > MAX_FILE_SIZE {
        return Err(RoxError::InvalidFormat(format!(
            "sus: file too large: {} bytes (max {MAX_FILE_SIZE})",
            data.len()
        )));
    }
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let text = String::from_utf8_lossy(data);
    report.invalid_text = matches!(text, std::borrow::Cow::Owned(_));

    let Directives {
        header,
        lines,
        shift,
        ticks,
    } = read_directives(&text, report);
    report.ticks_per_beat = ticks;
    let (lengths, definitions) = read_definitions(&lines, report);
    let bars = Bars::new(&lengths);
    let grid = Grid {
        bars: &bars,
        ticks_per_beat: f64::from(ticks),
    };
    let objects = read_objects(&lines, &grid, &definitions, report)?;
    let notes = resolve(
        &objects.taps,
        &objects.directionals,
        objects.slides,
        objects.guides,
        report,
    );
    Ok(Score {
        metadata: metadata(&header, fallback_title, report),
        shift_seconds: shift,
        bpm: objects.bpm,
        notes,
    })
}

/// Pass 1: the lines in file order, the metadata, the offset, the ticks and the measure base each line stands under.
fn read_directives<'a>(text: &'a str, report: &mut SekaiReport) -> Directives<'a> {
    let mut found = Directives {
        header: Header::default(),
        lines: Vec::new(),
        shift: 0.0,
        ticks: DEFAULT_TICKS_PER_BEAT,
    };
    let mut base = 0_i64;
    for raw in text.split(['\n', '\r']) {
        let Some(rest) = raw.trim().strip_prefix('#') else {
            continue;
        };
        let Some(end) = rest.find(|c: char| c == ':' || c.is_whitespace()) else {
            if !rest.is_empty() {
                report.ignored_directives += 1; // `#NOSPEED`, `#NOATTRIBUTE`
            }
            continue;
        };
        let (key, after) = (&rest[..end], rest[end..].trim_start());
        if let Some(data) = after.strip_prefix(':') {
            found.lines.push(DataLine {
                header: key,
                data: data.trim(),
                base,
            });
            continue;
        }
        let value = unquote(after.trim());
        let header = &mut found.header;
        match key.to_ascii_uppercase().as_str() {
            "TITLE" => header.title = value,
            "ARTIST" => header.artist = value,
            "GENRE" => header.genre = value,
            "DESIGNER" => header.designer = value,
            "DIFFICULTY" => header.difficulty = value,
            "PLAYLEVEL" => header.playlevel = Some(value),
            "WAVE" => header.wave = value,
            "BACKGROUND" => header.background = value,
            "WAVEOFFSET" => match value.parse::<f64>() {
                Ok(seconds) if seconds.is_finite() => found.shift = -seconds,
                _ => report.invalid_directives += 1,
            },
            "MEASUREBS" => match value.parse::<i64>() {
                Ok(n) if (0..=MAX_MEASURE_BASE).contains(&n) => base = n,
                _ => report.invalid_directives += 1,
            },
            "REQUEST" => {
                let mut words = value.split_whitespace();
                if words.next() == Some("ticks_per_beat") {
                    match words.next().and_then(|n| n.parse::<u32>().ok()) {
                        Some(n) if (1..=MAX_TICKS_PER_BEAT).contains(&n) => found.ticks = n,
                        _ => report.invalid_directives += 1,
                    }
                } else {
                    report.ignored_directives += 1; // `enable_priority` and the like
                }
            }
            _ => report.ignored_directives += 1,
        }
    }
    found
}

/// Pass 2: the measure lengths (by measure) and the BPM definitions (by id).
fn read_definitions<'a>(
    lines: &[DataLine<'a>],
    report: &mut SekaiReport,
) -> (BTreeMap<i64, f64>, HashMap<&'a str, f64>) {
    let mut lengths: BTreeMap<i64, f64> = BTreeMap::new();
    let mut definitions: HashMap<&str, f64> = HashMap::new();
    for line in lines {
        let name = line.header;
        if name.len() == 5 && name.is_ascii() && name.starts_with("BPM") {
            match unquote(line.data).parse::<f64>() {
                Ok(bpm) if valid_bpm(bpm) => {
                    definitions.insert(&name[3..], bpm);
                }
                _ => report.invalid_bpm += 1,
            }
        } else if name.len() == 5 && name.ends_with("02") && measure_number(name).is_some() {
            let measure = measure_number(name).unwrap_or_default() + line.base;
            match line.data.parse::<f64>() {
                Ok(beats) if beats.is_finite() && beats > 0.0 && beats <= MAX_MEASURE_BEATS => {
                    lengths.insert(measure, beats);
                }
                _ => report.invalid_measure_lengths += 1,
            }
        }
    }
    (lengths, definitions)
}

/// Pass 3: BPM changes, taps, directionals, slides and guides, by what the header of each line says.
fn read_objects(
    lines: &[DataLine<'_>],
    grid: &Grid<'_>,
    definitions: &HashMap<&str, f64>,
    report: &mut SekaiReport,
) -> RoxResult<Objects> {
    let mut budget = MAX_OBJECTS;
    let mut found = Objects::default();
    for line in lines {
        let name = line.header;
        if !name.is_ascii() {
            report.unsupported_lines += 1;
            continue;
        }
        if is_directive_line(line, report) {
            continue;
        }
        let Some(measure) = measure_number(name).map(|n| n + line.base) else {
            report.unsupported_lines += 1;
            continue;
        };
        let bytes = name.as_bytes();
        let channel = || bytes[5].to_ascii_lowercase();
        let list = match (bytes.len(), bytes[3]) {
            (5, b'0') if bytes[4] == b'2' => continue, // measure length, read in pass 2
            (5, b'0') if bytes[4] == b'8' => {
                read_bpm_changes(line, measure, grid, definitions, &mut found.bpm, &mut budget, report)?;
                continue;
            }
            (5, b'1') => &mut found.taps,
            (5, b'5') => &mut found.directionals,
            (6, b'3') => found.slides.entry(channel()).or_default(),
            (6, b'9') => found.guides.entry(channel()).or_default(),
            (6, b'2' | b'4') => {
                report.unsupported_notes += slots(line, measure, grid, &mut budget, report)?.len();
                continue;
            }
            _ => {
                report.unsupported_lines += 1;
                continue;
            }
        };
        read_notes(line, measure, grid, list, &mut budget, report)?;
    }
    Ok(found)
}

/// The 5-character lines that are no objects: `#BPMzz` (defined in pass 2), `#TILzz` time scales and `#ATRzz` attributes (counted).
fn is_directive_line(line: &DataLine<'_>, report: &mut SekaiReport) -> bool {
    let name = line.header;
    if name.len() != 5 {
        return false;
    }
    match &name[..3] {
        "BPM" => true,
        "TIL" => {
            report.time_scale_changes += line.data.split(',').filter(|s| !s.trim().is_empty()).count();
            true
        }
        "ATR" => {
            report.ignored_directives += 1;
            true
        }
        _ => false,
    }
}

/// The non-empty slots of a line. `budget` counts the objects still allowed in the file.
fn slots(
    line: &DataLine<'_>,
    measure: i64,
    grid: &Grid<'_>,
    budget: &mut usize,
    report: &mut SekaiReport,
) -> RoxResult<Vec<Slot>> {
    let bytes = line.data.as_bytes();
    if bytes.len() % 2 == 1 {
        report.odd_data_lines += 1;
    }
    let count = bytes.len() / 2;
    let (start, length) = grid.bars.locate(measure);
    let mut found = Vec::new();
    for (index, pair) in bytes.as_chunks::<2>().0.iter().enumerate() {
        if pair == b"00" {
            continue;
        }
        *budget = budget.checked_sub(1).ok_or_else(|| {
            RoxError::InvalidFormat(format!("sus: more than {MAX_OBJECTS} objects"))
        })?;
        #[allow(clippy::cast_precision_loss)] // a slot index is below 32 million
        let beat = start + length * index as f64 / count as f64;
        #[allow(clippy::cast_possible_truncation)]
        // at most 1e6 measures x 1024 beats x 1e6 ticks x 1e3
        let key = (beat * grid.ticks_per_beat * 1000.0).round() as i64;
        found.push(Slot {
            beat,
            key,
            pair: *pair,
        });
    }
    Ok(found)
}

/// `#mmm08`: each slot names a BPM definition.
fn read_bpm_changes(
    line: &DataLine<'_>,
    measure: i64,
    grid: &Grid<'_>,
    definitions: &HashMap<&str, f64>,
    bpm: &mut Vec<BpmChange>,
    budget: &mut usize,
    report: &mut SekaiReport,
) -> RoxResult<()> {
    let (_, length) = grid.bars.locate(measure);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped to 1..=255
    let signature = length.round().clamp(1.0, 255.0) as u8;
    for slot in slots(line, measure, grid, budget, report)? {
        let value = std::str::from_utf8(&slot.pair)
            .ok()
            .and_then(|id| definitions.get(id));
        match value {
            Some(&value) => bpm.push(BpmChange {
                beat: slot.beat,
                bpm: value,
                signature,
            }),
            None => report.unknown_bpm_ids += 1,
        }
    }
    Ok(())
}

/// The slots of a note line (`#mmm1x`, `#mmm5x`, `#mmm3xy`, `#mmm9xy`): the kind is the first character of a pair, the width the
/// second, the lane the `x` of the header.
fn read_notes(
    line: &DataLine<'_>,
    measure: i64,
    grid: &Grid<'_>,
    out: &mut Vec<Raw>,
    budget: &mut usize,
    report: &mut SekaiReport,
) -> RoxResult<()> {
    let Some(x) = digit(line.header.as_bytes()[4]) else {
        report.unsupported_lines += 1;
        return Ok(());
    };
    for slot in slots(line, measure, grid, budget, report)? {
        let (Some(kind), Some(width)) = (digit(slot.pair[0]), digit(slot.pair[1])) else {
            report.invalid_pairs += 1;
            continue;
        };
        if kind == 0 {
            continue; // "0 is always unassigned"
        }
        if width == 0 {
            report.zero_width_notes += 1;
            continue;
        }
        out.push(Raw {
            beat: slot.beat,
            key: slot.key,
            x,
            width,
            kind,
        });
    }
    Ok(())
}

/// Settles what is on the same tick and lane (the notes: overlaps are decided by the leftmost lane alone) and makes the notes.
fn resolve(
    taps: &[Raw],
    directionals: &[Raw],
    slides: BTreeMap<u8, Vec<Raw>>,
    guides: BTreeMap<u8, Vec<Raw>>,
    report: &mut SekaiReport,
) -> Vec<ScoreNote> {
    let places = Places {
        // `susToUSC`: only the slide kinds 1, 2, 3 and 5 keep a tap from being a note.
        slides: slides.values().flatten().filter(|raw| matches!(raw.kind, 1 | 2 | 3 | 5)).map(place).collect(),
        guides: guides.values().flatten().map(place).collect(),
        directionals: directionals.iter().map(place).collect(),
    };
    let mut note_places: HashSet<(i64, u32)> = HashSet::new();
    let mut notes = Vec::new();
    for tap in taps {
        match tap.kind {
            1 | 2 | 5 | 6 => {
                note_places.insert(place(tap));
                notes.extend(places.tap_note(tap, report));
            }
            3 | 7 | 8 => places.overlay(place(tap), report),
            4 => report.skill_markers += 1,
            _ => report.unsupported_notes += 1,
        }
    }
    for directional in directionals {
        let at = place(directional);
        if !note_places.contains(&at) || places.slides.contains(&at) {
            places.overlay(at, report);
        }
    }
    for points in slides.into_values() {
        for slide in channel_slides(points) {
            read_slide(&slide, &mut notes, report);
        }
    }
    for points in guides.into_values() {
        report.guides += channel_slides(points).len();
    }
    notes
}

/// Where an object is: its tick and leftmost lane.
fn place(raw: &Raw) -> (i64, u32) {
    (raw.key, raw.x)
}

/// The places of the slide joints, guide joints and directionals.
struct Places {
    slides: HashSet<(i64, u32)>,
    guides: HashSet<(i64, u32)>,
    directionals: HashSet<(i64, u32)>,
}

impl Places {
    /// A tap of kind 1, 2, 5 or 6: a note when it is on the stage and no slide modifier, trace or fever marker (counted).
    fn tap_note(&self, tap: &Raw, report: &mut SekaiReport) -> Option<ScoreNote> {
        let at = place(tap);
        if self.slides.contains(&at) {
            report.slide_modifiers += 1;
        } else if tap.kind >= 5 {
            report.traces += 1;
        } else if tap.x == FEVER_LANE && tap.width == 1 {
            report.fever_markers += 1;
        } else if let Some(column) = stage_column(tap) {
            report.critical_flags += usize::from(tap.kind == 2);
            report.flicks += usize::from(self.directionals.contains(&at));
            return Some(ScoreNote { beat: tap.beat, end_beat: None, column });
        } else {
            report.notes_outside_lanes += 1;
        }
        None
    }

    /// An overlaid object that is not a note: a modifier of a slide or guide joint, or stray.
    fn overlay(&self, at: (i64, u32), report: &mut SekaiReport) {
        if self.slides.contains(&at) {
            report.slide_modifiers += 1;
        } else if self.guides.contains(&at) {
            report.guide_modifiers += 1;
        } else {
            report.stray_overlays += 1;
        }
    }
}

/// The column of a note entirely on the stage (SUS lanes `2..=13`).
fn stage_column(raw: &Raw) -> Option<u8> {
    let left = i64::from(raw.x) - i64::from(FIRST_LANE);
    column_of(left, i64::from(raw.width))
}

/// The slides of one channel: its points by time (file order within a tick), cut after each end (kind 2), as `susToUSC` does.
fn channel_slides(mut points: Vec<Raw>) -> Vec<Vec<Raw>> {
    points.sort_by(|a, b| a.beat.total_cmp(&b.beat));
    let mut slides: Vec<Vec<Raw>> = Vec::new();
    let mut open = false;
    for point in points {
        if !open {
            slides.push(Vec::new());
            open = true;
        }
        let ends = point.kind == 2;
        if let Some(slide) = slides.last_mut() {
            slide.push(point);
        }
        if ends {
            open = false;
        }
    }
    slides
}

/// A slide: a hold from its first joint to its last on the first joint's column.
fn read_slide(points: &[Raw], notes: &mut Vec<ScoreNote>, report: &mut SekaiReport) {
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return;
    };
    if first.kind != 1 {
        report.slide_points_without_start += points.len();
        return;
    }
    if last.kind != 2 {
        report.unterminated_slides += 1;
        return;
    }
    let Some(column) = stage_column(first) else {
        report.notes_outside_lanes += 1;
        return;
    };
    report.slide_joints_dropped += points.len() - 2;
    notes.push(ScoreNote {
        beat: first.beat,
        end_beat: Some(last.beat),
        column,
    });
}

fn metadata(header: &Header<'_>, fallback_title: &str, report: &mut SekaiReport) -> Metadata {
    let non_empty = |value: &str| (!value.is_empty()).then(|| value.into());
    Metadata {
        title: if header.title.is_empty() {
            fallback_title.into()
        } else {
            header.title.into()
        },
        artist: header.artist.into(),
        creator: header.designer.into(),
        difficulty_name: header.difficulty.into(),
        difficulty_value: header.playlevel.and_then(|level| play_level(level, report)),
        audio_file: header.wave.into(),
        background_file: non_empty(header.background),
        genre: non_empty(header.genre),
        ..Metadata::default()
    }
}

/// `#PLAYLEVEL`: a number, with a `+` after it adding half a level.
fn play_level(value: &str, report: &mut SekaiReport) -> Option<f32> {
    let value = value.trim();
    let (number, plus) = value
        .strip_suffix('+')
        .map_or((value, 0.0), |number| (number.trim_end(), 0.5));
    match number.parse::<f32>() {
        Ok(level) if level.is_finite() && level >= 0.0 => Some(level + plus),
        _ => {
            if !value.is_empty() {
                report.invalid_directives += 1;
            }
            None
        }
    }
}
