#![warn(clippy::pedantic)]
//! FNF Codename Engine charts: `songs/<song>/charts[/<variant>]/<difficulty>.json`, with the song's `meta.json` and `events.json`. Decode only.
//!
//! Format notes: `vsrg-format-docs/vsrg/friday-night-funkin/codename-json`, from the engine's source (FNF-CNE-Devs/CodenameEngine, Apache-2.0,
//! read for facts: `ChartData.hx`, `Chart.hx`, `Conductor.hx`, `Paths.hx`), checked on the corpus charts.
//!
//! - **Which file**: a `.json` that sets `codenameChart` to `true` (`Chart.detectChartFormat`; the engine writes it on every save), told by
//!   [`looks_like_codename`] over the whole file (the key comes after the notes).
//! - **Columns**: the player's strumlines (`type` 1) side by side, each `keyCount` wide (4 when absent): one player line is 4K, a 6-key one
//!   6K. The opponent's (`type` 0) and the extra lines (`type` 2) are not in the chart; their notes are counted.
//! - **Notes**: `time` ms, `id` the lane in its strumline, `sLen` > 0 a hold of that many ms. A custom note type (`type` n ≥ 1, the name
//!   `noteTypes[n - 1]`) is scripted by the mod; it is read as a normal note and counted by name.
//! - **Tempo**: `meta.bpm` (100 when absent, the engine's default) and `beatsPerMeasure` (4) from time 0, then the `BPM Change` events
//!   (`params[0]`) and the numerator of `Time Signature Change` (`params[0]`), from the chart's `events` and the song's `events.json`. A
//!   `Continuous BPM Change` (a ramp) is counted, not read. Note times are absolute: the tempo only draws the beat lines.
//! - **Files around it** ([`decode_file`]): the song folder is the parent of `charts`. Its meta is the first of `meta-<variant>-<difficulty>.json`,
//!   `meta-<variant>.json`, `meta-<difficulty>.json`, `meta.json` that exists (`Chart.loadChartMeta`), the chart's own `meta` fields over it;
//!   its events `events[-<variant>].json`; its music `song/Inst<instSuffix>-<difficulty>.ogg`, else `song/Inst<instSuffix>.ogg` (`Paths.inst`),
//!   named relative to the chart's folder. The vocals (`Voices*.ogg`) are not mixed in. A chart outside a `charts` folder is read alone.
//! - **Metadata**: the title is `displayName` (else the song folder's name), the difficulty the file stem.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Note, RoxChart, TimingPoint, drop_stacked};
use serde::Deserialize;
use serde_json::Value;

/// Decoder of a Codename Engine chart file alone (no meta, no global events).
pub struct CodenameDecoder;

/// Files above this size are refused (a chart is a few hundred kilobytes).
const MAX_FILE_SIZE: usize = 32 * 1024 * 1024;
/// The engine's BPM and beats per measure when the meta has none (`Flags.DEFAULT_BPM`, `DEFAULT_BEATS_PER_MEASURE`).
const DEFAULT_BPM: f64 = 100.0;
const DEFAULT_BEATS_PER_MEASURE: u8 = 4;
/// Most columns read (all player strumlines together).
const MAX_COLUMNS: usize = 64;
/// Times further than this many milliseconds from the start are not read.
const MAX_MS: f64 = 1e10;
const PLAYER: i64 = 1;

/// What the decoder dropped or changed. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodenameReport {
    /// Notes of the opponent's and the extra strumlines: not in the chart.
    pub other_strumline_notes: usize,
    /// Player notes with a lane outside their strumline, or a time or a length that is not a number in range: dropped.
    pub invalid_notes: usize,
    /// Player notes of a custom type, read as normal notes. Name -> count.
    pub note_types: BTreeMap<String, usize>,
    /// `Continuous BPM Change` events (tempo ramps): not read.
    pub tempo_ramps_ignored: usize,
    /// Notes on another note of their column (same time, or inside a hold): dropped.
    pub overlapping_notes_dropped: usize,
}

#[derive(Deserialize)]
struct ChartData {
    #[serde(rename = "strumLines")]
    strum_lines: Vec<StrumLine>,
    #[serde(default)]
    events: Vec<Event>,
    #[serde(default, rename = "noteTypes")]
    note_types: Vec<String>,
    #[serde(default)]
    meta: Option<Meta>,
}

#[derive(Deserialize)]
struct StrumLine {
    #[serde(default, rename = "type")]
    kind: i64,
    #[serde(default)]
    notes: Vec<ChartNote>,
    #[serde(default, rename = "keyCount")]
    key_count: Option<i64>,
}

#[derive(Deserialize)]
struct ChartNote {
    time: f64,
    id: i64,
    #[serde(default, rename = "type")]
    kind: i64,
    #[serde(default, rename = "sLen")]
    length: f64,
}

#[derive(Deserialize)]
struct Event {
    #[serde(default)]
    name: Option<String>,
    /// The old numbered events (`Chart.hx` converts 2 to `BPM Change`).
    #[serde(default, rename = "type")]
    kind: Option<i64>,
    #[serde(default)]
    time: f64,
    #[serde(default)]
    params: Vec<Value>,
}

#[derive(Deserialize, Default)]
struct Meta {
    #[serde(default, rename = "displayName")]
    display_name: Option<String>,
    #[serde(default)]
    bpm: Option<f64>,
    #[serde(default, rename = "beatsPerMeasure")]
    beats_per_measure: Option<f64>,
    #[serde(default, rename = "instSuffix")]
    inst_suffix: Option<String>,
}

#[derive(Deserialize)]
struct EventsFile {
    #[serde(default)]
    events: Vec<Event>,
}

impl Meta {
    /// `self` with the fields `over` sets laid over it (the chart's own `meta` wins over the meta file's).
    fn merged(self, over: Option<Meta>) -> Meta {
        let Some(over) = over else { return self };
        Meta {
            display_name: over.display_name.or(self.display_name),
            bpm: over.bpm.or(self.bpm),
            beats_per_measure: over.beats_per_measure.or(self.beats_per_measure),
            inst_suffix: over.inst_suffix.or(self.inst_suffix),
        }
    }
}

impl Decoder for CodenameDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode(data, None, None).map(|(chart, _, _)| chart)
    }
}

impl CodenameDecoder {
    /// Decodes a chart with the song's meta and events files, when there are, and says what was dropped.
    ///
    /// # Errors
    /// Returns an error if a file is too large or is not JSON of its kind, or the chart has no playable note.
    pub fn decode_with_report(chart: &[u8], meta: Option<&[u8]>, events: Option<&[u8]>) -> RoxResult<(RoxChart, CodenameReport)> {
        decode(chart, meta, events).map(|(chart, report, _)| (chart, report))
    }
}

/// Whether a file's bytes (all of them) are a Codename chart: a JSON object that sets `"codenameChart"` to `true` (or `"true"`).
#[must_use]
pub fn looks_like_codename(data: &[u8]) -> bool {
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    if data.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
        return false;
    }
    memchr::memmem::find_iter(data, b"\"codenameChart\"").any(|at| {
        let rest = data[at + 15..].trim_ascii_start();
        rest.strip_prefix(b":").is_some_and(|value| {
            let value = value.trim_ascii_start();
            value.starts_with(b"true") || value.starts_with(b"\"true\"")
        })
    })
}

/// Whether the file at `path` is a Codename chart (a `.json` that [`looks_like_codename`]); the file is read.
#[must_use]
pub fn is_codename_chart_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        && std::fs::metadata(path).is_ok_and(|meta| meta.len() <= MAX_FILE_SIZE as u64)
        && std::fs::read(path).is_ok_and(|data| looks_like_codename(&data))
}

fn invalid(reason: impl std::fmt::Display) -> RoxError {
    RoxError::InvalidFormat(format!("codename: {reason}"))
}

fn parse<'a, T: Deserialize<'a>>(data: &'a [u8], what: &str) -> RoxResult<T> {
    if data.len() > MAX_FILE_SIZE {
        return Err(invalid(format!("{what} too large: {} bytes", data.len())));
    }
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    serde_json::from_slice(data).map_err(|e| invalid(format!("{what}: {e}")))
}

/// The chart, what was dropped, and the meta it was read with.
fn decode(chart: &[u8], meta: Option<&[u8]>, events: Option<&[u8]>) -> RoxResult<(RoxChart, CodenameReport, Meta)> {
    let data: ChartData = parse(chart, "chart")?;
    let meta = match meta {
        Some(meta) => parse::<Meta>(meta, "meta")?,
        None => Meta::default(),
    }
    .merged(data.meta);
    let mut all_events = data.events;
    if let Some(events) = events {
        all_events.extend(parse::<EventsFile>(events, "events")?.events);
    }
    let mut report = CodenameReport::default();
    let mut notes = Vec::new();
    let mut first_column = 0usize;
    for line in &data.strum_lines {
        if line.kind != PLAYER {
            report.other_strumline_notes += line.notes.len();
            continue;
        }
        let width = usize::try_from(line.key_count.unwrap_or(4)).ok().filter(|&width| width > 0 && first_column + width <= MAX_COLUMNS);
        let Some(width) = width else {
            report.invalid_notes += line.notes.len();
            continue;
        };
        for note in &line.notes {
            match note_of(note, first_column, width) {
                Some(decoded) => {
                    if note.kind > 0 {
                        let name = usize::try_from(note.kind - 1).ok().and_then(|index| data.note_types.get(index));
                        *report.note_types.entry(name.cloned().unwrap_or_else(|| format!("#{}", note.kind))).or_default() += 1;
                    }
                    notes.push(decoded);
                }
                None => report.invalid_notes += 1,
            }
        }
        first_column += width;
    }
    if notes.is_empty() {
        return Err(invalid("no playable note in the player's strumlines"));
    }
    let key_count = u8::try_from(first_column).expect("at most 64 columns");
    let mut rox = RoxChart::new(key_count);
    notes.sort_by_key(|note| (note.time_us, note.column));
    rox.notes = notes;
    report.overlapping_notes_dropped = drop_stacked(&mut rox.notes, key_count, |note| note).total();
    rox.timing_points = tempo(&meta, &mut all_events, &mut report, rox.notes[0].time_us);
    if let Some(title) = &meta.display_name {
        rox.metadata.title = title.as_str().into();
    }
    rox.validate()?;
    Ok((rox, report, meta))
}

fn note_of(note: &ChartNote, first_column: usize, width: usize) -> Option<Note> {
    let lane = usize::try_from(note.id).ok().filter(|&lane| lane < width)?;
    let column = u8::try_from(first_column + lane).ok()?;
    let in_range = |ms: f64| ms.is_finite() && ms.abs() <= MAX_MS;
    if !in_range(note.time) || !in_range(note.length) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)] // bounded above
    let (start, length) = ((note.time * 1000.0).round() as i64, (note.length * 1000.0).round() as i64);
    Some(if length > 0 { Note::hold(start, length, column) } else { Note::tap(start, column) })
}

/// The meta's BPM and measure at 0 (or at the first note, if earlier), then the tempo events in time order.
fn tempo(meta: &Meta, events: &mut [Event], report: &mut CodenameReport, first_note: i64) -> Vec<TimingPoint> {
    let mut bpm = meta.bpm.filter(|bpm| bpm.is_finite() && *bpm > 0.0).unwrap_or(DEFAULT_BPM);
    let mut signature = meta.beats_per_measure.and_then(numerator).unwrap_or(DEFAULT_BEATS_PER_MEASURE);
    #[allow(clippy::cast_possible_truncation)] // a BPM fits f32
    let point = |time_us: i64, bpm: f64, signature: u8| TimingPoint::Bpm { time_us, bpm: bpm as f32, signature };
    let mut points = vec![point(first_note.min(0), bpm, signature)];
    events.sort_by(|a, b| a.time.total_cmp(&b.time));
    for event in events.iter() {
        let name = event.name.as_deref().or(match event.kind {
            Some(2) => Some("BPM Change"),
            _ => None,
        });
        let value = event.params.first().and_then(Value::as_f64);
        match name {
            Some("BPM Change") => match value.filter(|bpm| bpm.is_finite() && *bpm > 0.0) {
                Some(next) => bpm = next,
                None => continue,
            },
            Some("Time Signature Change") => match value.and_then(numerator) {
                Some(next) => signature = next,
                None => continue,
            },
            Some("Continuous BPM Change") => {
                report.tempo_ramps_ignored += 1;
                continue;
            }
            _ => continue,
        }
        if !(event.time.is_finite() && event.time.abs() <= MAX_MS) {
            continue;
        }
        #[allow(clippy::cast_possible_truncation)] // bounded above
        let time_us = ((event.time * 1000.0).round() as i64).max(0);
        // A change at the time of the previous point replaces it.
        if points.last().is_some_and(|last| last.time_us() == time_us) {
            points.pop();
        }
        points.push(point(time_us, bpm, signature));
    }
    points
}

fn numerator(value: f64) -> Option<u8> {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // range checked
    (value.is_finite() && (1.0..=255.0).contains(&value)).then(|| value.round() as u8)
}

/// Reads a chart file with the files of its song folder (see the module notes): `data` is the chart's bytes, `path` its path.
///
/// # Errors
/// Returns an error if a file is not JSON of its kind or the chart has no playable note.
pub fn decode_file(path: &Path, data: &[u8]) -> RoxResult<RoxChart> {
    let difficulty = path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
    let song = song_folder(path);
    let (meta, events) = match &song {
        Some(song) => (read_first(&meta_names(song, &difficulty)), read_first(&[song.folder.join(format!("events{}.json", song.suffix()))])),
        None => (None, None),
    };
    let (mut chart, _, meta) = decode(data, meta.as_deref(), events.as_deref())?;
    chart.metadata.difficulty_name = difficulty.as_str().into();
    if meta.display_name.is_none() {
        // The song folder's name, or the chart's folder for a chart read alone.
        let folder = song.as_ref().map(|song| song.folder.as_path()).or_else(|| path.parent());
        chart.metadata.title = folder.and_then(Path::file_name).map(|name| name.to_string_lossy().into_owned()).unwrap_or_default().into();
    }
    if let Some(song) = &song {
        chart.metadata.audio_file = instrumental(song, &difficulty, meta.inst_suffix.as_deref().unwrap_or("")).into();
    }
    Ok(chart)
}

/// A song folder and the variant a chart belongs to.
struct Song {
    folder: PathBuf,
    variant: Option<String>,
}

impl Song {
    fn suffix(&self) -> String {
        self.variant.as_ref().map_or_else(String::new, |variant| format!("-{variant}"))
    }
}

/// The parent of the `charts` folder that holds `path` directly or through one variant folder.
fn song_folder(path: &Path) -> Option<Song> {
    let is_charts = |folder: &Path| folder.file_name().is_some_and(|name| name.eq_ignore_ascii_case("charts"));
    let parent = path.parent()?;
    if is_charts(parent) {
        return Some(Song { folder: parent.parent()?.to_path_buf(), variant: None });
    }
    let charts = parent.parent()?;
    if !is_charts(charts) {
        return None;
    }
    Some(Song { folder: charts.parent()?.to_path_buf(), variant: Some(parent.file_name()?.to_string_lossy().into_owned()) })
}

fn meta_names(song: &Song, difficulty: &str) -> Vec<PathBuf> {
    let mut names = Vec::new();
    if let Some(variant) = &song.variant {
        names.push(format!("meta-{variant}-{difficulty}.json"));
        names.push(format!("meta-{variant}.json"));
    }
    names.push(format!("meta-{difficulty}.json"));
    names.push("meta.json".to_owned());
    names.into_iter().map(|name| song.folder.join(name)).collect()
}

fn read_first(paths: &[PathBuf]) -> Option<Vec<u8>> {
    paths.iter().find_map(|path| std::fs::read(path).ok())
}

/// The instrumental's name relative to the chart's folder (`../song/Inst.ogg`), or empty when the song has none.
fn instrumental(song: &Song, difficulty: &str, suffix: &str) -> String {
    let up = if song.variant.is_some() { "../../" } else { "../" };
    [format!("Inst{suffix}-{difficulty}.ogg"), format!("Inst{suffix}.ogg")]
        .into_iter()
        .find(|name| song.folder.join("song").join(name).is_file())
        .map_or_else(String::new, |name| format!("{up}song/{name}"))
}

#[cfg(test)]
mod tests;
