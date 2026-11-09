#![warn(clippy::pedantic)]
//! bmson 1.0.0 decoder (`.bmson`, the JSON dialect of BMS). Decode only.
//!
//! Time is in pulses (`info.resolution` per quarter note, default 240); there are no measures. Positions go through the
//! same `Timeline` as BMS (STOP in pulses becomes 1/48-beat units). The layout comes from `info.mode_hint`, with the
//! BMS column order: the scratch is its own column, left of the keys of the first side and right of the keys of the second.
//!
//! | `mode_hint` | `x` | columns |
//! |---|---|---|
//! | `beat-5k`, `beat-7k` | keys `1..=N`, scratch `8` | `[scratch] key1..keyN` (scratch column only when used) |
//! | `beat-10k`, `beat-14k` | P1 keys `1..=N`, scratch `8`; P2 keys `9..=8+N`, scratch `16` | `[scratch1] P1 keys \| P2 keys [scratch2]` (both scratches when either is used) |
//! | `popn-5k`, `popn-9k` | `1..=5`, `1..=9` | 5, 9 |
//! | `generic-nkeys` | `1..=n` | n |
//! | `keyboard-24k`, `keyboard-24k-double` | `1..=26`, `1..=52` (beatoraja's `Mode`, not the spec) | 26, 52 |
//!
//! An unknown hint reads as `beat-7k`, like beatoraja (counted in the report). `x` of `0` or null is a background sound.
//! A note with `l > 0` is a hold. Each playable note's channel file goes to the keysound table. Slicing of a channel's
//! audio is not done: when exactly one background note exists and its channel holds nothing else, that file is the
//! chart's audio and the time origin (as for BMS), otherwise the audio is empty. beatoraja extensions
//! (`mine_channels`, `key_channels`, `scroll_events`, `ln_type`, `up`) are ignored.

use std::collections::HashMap;

use rox::audio_plan::{AudioPlan, SoundEvent};
use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Hitsound, Metadata, Note, RoxChart, TimingPoint, drop_stacked};

use crate::formats::keysound;
use serde::Deserialize;
use serde_json::Value;

use super::timing::{Change, Timeline};

/// Decoder of `.bmson` files (version 1.x).
pub struct BmsonDecoder;
crate::impl_format!(BmsonDecoder, ["bmson"]);

#[derive(Deserialize)]
struct Bmson {
    version: Option<String>,
    info: Option<Info>,
    #[serde(default)]
    bpm_events: Vec<BpmEvent>,
    #[serde(default)]
    stop_events: Vec<StopEvent>,
    #[serde(default)]
    sound_channels: Vec<SoundChannel>,
}

#[derive(Deserialize)]
struct Info {
    #[serde(default)]
    title: String,
    #[serde(default)]
    subtitle: String,
    #[serde(default)]
    artist: String,
    #[serde(default)]
    subartists: Vec<String>,
    #[serde(default)]
    genre: String,
    mode_hint: Option<String>,
    #[serde(default)]
    chart_name: String,
    level: Option<f64>,
    init_bpm: Option<f64>,
    back_image: Option<String>,
    eyecatch_image: Option<String>,
    resolution: Option<i64>,
}

#[derive(Deserialize)]
struct BpmEvent {
    y: i64,
    bpm: f64,
}

#[derive(Deserialize)]
struct StopEvent {
    y: i64,
    duration: i64,
}

#[derive(Deserialize)]
struct SoundChannel {
    #[serde(default)]
    name: String,
    #[serde(default)]
    notes: Vec<BmsonNote>,
}

#[derive(Deserialize)]
struct BmsonNote {
    x: Option<Value>,
    y: i64,
    #[serde(default)]
    l: i64,
    #[serde(default)]
    c: bool,
}

/// What the decoder chose, skipped or repaired.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BmsonReport {
    /// Layout the lanes were put in, e.g. `beat-7k+scratch (8 columns)`.
    pub layout: String,
    /// The `mode_hint` was not known and `beat-7k` was used.
    pub unknown_mode_hint: bool,
    /// BPM events ignored (not positive or not finite) or at a negative pulse.
    pub ignored_bpm_events: usize,
    /// Notes whose `x` is not a lane of the layout, or whose `y` is negative: dropped.
    pub notes_outside_layout: usize,
    /// Notes that start inside a hold or at the same time and column as another note (layered notes keep the first): dropped.
    pub overlapping_notes_dropped: usize,
    /// Background notes that cannot be part of a ROX chart: not decoded.
    pub bgm_notes_skipped: usize,
    /// Whether the chart's audio is the one background sound of the file.
    pub single_audio: bool,
    /// File names refused because they are absolute or climb out of the folder.
    pub unsafe_names: usize,
}

impl BmsonDecoder {
    /// Decodes `data` and reports what the decoder had to choose, skip or repair.
    ///
    /// # Errors
    /// Returns an error if the JSON is invalid, the version is not 1.x, `init_bpm` is missing or no note is playable.
    pub fn decode_with_report(data: &[u8]) -> RoxResult<(RoxChart, BmsonReport)> {
        let (chart, report, _) = convert(data)?;
        chart.validate()?;
        Ok((chart, report))
    }

    /// [`Self::decode_with_report`] plus the [`AudioPlan`]: every sound channel sliced as beatoraja does (see
    /// [`slice_plan`](self) in the module notes), background notes included.
    ///
    /// # Errors
    /// Same as [`Self::decode_with_report`].
    pub fn decode_with_audio_plan(data: &[u8]) -> RoxResult<(RoxChart, BmsonReport, AudioPlan)> {
        let (chart, report, plan) = convert(data)?;
        chart.validate()?;
        Ok((chart, report, plan))
    }
}

impl Decoder for BmsonDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        convert(data).map(|(chart, _, _)| chart)
    }
}

/// Column layout of a `mode_hint`.
#[derive(Debug, Clone, Copy)]
enum Mode {
    Beat { keys: u8, double: bool },
    Flat(u8),
}

impl Mode {
    fn parse(hint: &str) -> Option<Self> {
        let hint = hint.trim().to_ascii_lowercase();
        match hint.as_str() {
            "beat-5k" => Some(Self::Beat {
                keys: 5,
                double: false,
            }),
            "beat-7k" => Some(Self::Beat {
                keys: 7,
                double: false,
            }),
            "beat-10k" => Some(Self::Beat {
                keys: 5,
                double: true,
            }),
            "beat-14k" => Some(Self::Beat {
                keys: 7,
                double: true,
            }),
            "popn-5k" => Some(Self::Flat(5)),
            "popn-9k" => Some(Self::Flat(9)),
            "keyboard-24k" => Some(Self::Flat(26)),
            "keyboard-24k-double" => Some(Self::Flat(52)),
            other => {
                let n: u8 = other
                    .strip_prefix("generic-")?
                    .strip_suffix("keys")?
                    .parse()
                    .ok()?;
                (n > 0).then_some(Self::Flat(n))
            }
        }
    }

    /// Whether `x` is a scratch lane.
    fn is_scratch(self, x: i64) -> bool {
        matches!(self, Self::Beat { double: false, .. }) && x == 8
            || matches!(self, Self::Beat { double: true, .. }) && (x == 8 || x == 16)
    }

    /// Column of lane `x`, `scratch` telling whether the layout has scratch columns.
    fn column(self, x: i64, scratch: bool) -> Option<u8> {
        let column = match self {
            Self::Flat(n) => (1..=i64::from(n)).contains(&x).then(|| x - 1)?,
            Self::Beat { keys, double } => {
                let keys = i64::from(keys);
                let lead = i64::from(scratch);
                match x {
                    8 if scratch => 0,
                    16 if scratch && double => lead + 2 * keys,
                    1.. if x <= keys => lead + x - 1,
                    9.. if double && x < 9 + keys => lead + keys + (x - 9),
                    _ => return None,
                }
            }
        };
        u8::try_from(column).ok()
    }

    fn count(self, scratch: bool) -> u8 {
        match self {
            Self::Flat(n) => n,
            Self::Beat { keys, double } => {
                let sides = if double { 2 } else { 1 };
                keys * sides + if scratch { sides } else { 0 }
            }
        }
    }
}

/// `name` with `\` as `/`, unless it is empty, absolute, holds a NUL or climbs out of the folder.
pub(crate) fn safe_name(name: &str, refused: &mut usize) -> Option<String> {
    let name = name.trim().replace('\\', "/");
    if name.is_empty() {
        return None;
    }
    let bytes = name.as_bytes();
    let drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if name.starts_with('/')
        || drive
        || name.contains('\0')
        || name.split('/').any(|part| part == "..")
    {
        *refused += 1;
        return None;
    }
    Some(name)
}

/// A playable note before columns and times.
struct Draft {
    y: i64,
    end: i64,
    x: i64,
    channel: usize,
}

fn convert(data: &[u8]) -> RoxResult<(RoxChart, BmsonReport, AudioPlan)> {
    let file: Bmson = serde_json::from_slice(data).map_err(|e| RoxError::InvalidFormat(format!("bmson JSON: {e}")))?;
    check_version(file.version.as_deref())?;
    let info = file.info.as_ref().ok_or_else(|| RoxError::InvalidFormat("bmson without `info`".into()))?;
    let init_bpm = info
        .init_bpm
        .filter(|bpm| bpm.is_finite() && *bpm > 0.0)
        .ok_or_else(|| RoxError::InvalidFormat("bmson `info.init_bpm` is missing or not positive".into()))?;
    let mut report = BmsonReport::default();
    let hint = info.mode_hint.as_deref().unwrap_or("beat-7k");
    let parsed_mode = Mode::parse(hint);
    report.unknown_mode_hint = parsed_mode.is_none();
    let mode = parsed_mode.unwrap_or(Mode::Beat { keys: 7, double: false });
    let mut clock = Clock::new(&file, info, init_bpm, &mut report);

    let lanes = Lanes::read(&file.sound_channels, &mut report);
    let scratch = lanes.drafts.iter().any(|d| mode.is_scratch(d.x));
    let key_count = mode.count(scratch);
    report.layout = format!("{hint} ({key_count} columns)");
    let single = lanes.single_audio(&file.sound_channels, &mut report);
    clock.shift_us = single.as_ref().map_or(0, |&(_, y)| -clock.unshifted_us(y));

    let mut rox = RoxChart::new(key_count);
    rox.metadata = metadata(info, single.map(|(name, _)| name), &mut report.unsafe_names);
    rox.timing_points = clock.bpm_points();
    add_notes(&mut rox, &lanes.drafts, &file.sound_channels, &clock, |x| mode.column(x, scratch), &mut report);
    if rox.notes.is_empty() {
        return Err(RoxError::InvalidFormat("bmson file with no playable note".into()));
    }
    let plan = slice_plan(&file.sound_channels, &clock, &mut 0); // the names were counted above
    Ok((rox, report, plan))
}

fn check_version(version: Option<&str>) -> RoxResult<()> {
    match version {
        Some(v) if v.trim().starts_with("1.") => Ok(()),
        Some(v) => Err(RoxError::InvalidFormat(format!("bmson version {v:?} is not supported (1.x only)"))),
        None => Err(RoxError::InvalidFormat("bmson without `version` (legacy 0.2x) is not supported".into())),
    }
}

fn to_us(seconds: f64) -> i64 {
    #[allow(clippy::cast_possible_truncation)] // seconds to µs
    let us = (seconds * 1_000_000.0).round() as i64;
    us
}

/// Pulses to chart time: the timeline with stops, the tempo alone (slice lengths), and the shift that puts the single audio at 0.
struct Clock {
    resolution: f64,
    timeline: Timeline,
    tempo_only: Timeline,
    shift_us: i64,
}

impl Clock {
    fn new(file: &Bmson, info: &Info, init_bpm: f64, report: &mut BmsonReport) -> Self {
        // Pulses: 0 and null are 240, negative is absolute.
        let resolution = match info.resolution.map(i64::unsigned_abs) {
            None | Some(0) => 240.0,
            #[allow(clippy::cast_precision_loss)] // a resolution is small
            Some(r) => r as f64,
        };
        let changes = tempo_changes(file, resolution, report);
        let no_stops: Vec<(f64, Change)> = changes.iter().copied().filter(|(_, c)| matches!(c, Change::Bpm(_))).collect();
        Self {
            resolution,
            timeline: Timeline::new(init_bpm, &changes),
            tempo_only: Timeline::new(init_bpm, &no_stops),
            shift_us: 0,
        }
    }

    fn beat(&self, y: i64) -> f64 {
        beat_of(y, self.resolution)
    }

    fn unshifted_us(&self, y: i64) -> i64 {
        to_us(self.timeline.seconds(self.beat(y)))
    }

    fn us(&self, y: i64) -> i64 {
        self.unshifted_us(y) + self.shift_us
    }

    /// The length of the music from `y` to `next`, stops not counted.
    fn tempo_len_us(&self, y: i64, next: i64) -> i64 {
        to_us(self.tempo_only.seconds(self.beat(next))) - to_us(self.tempo_only.seconds(self.beat(y)))
    }

    fn bpm_points(&self) -> Vec<TimingPoint> {
        #[allow(clippy::cast_possible_truncation)] // a BPM fits f32
        let point = |seconds, bpm: f64| TimingPoint::bpm(to_us(seconds) + self.shift_us, bpm as f32);
        self.timeline.bpm_points().into_iter().map(|p| point(p.seconds, p.bpm)).collect()
    }
}

#[allow(clippy::cast_precision_loss)] // pulses of any real chart fit f64 exactly
fn beat_of(y: i64, resolution: f64) -> f64 {
    y as f64 / resolution
}

/// The BPM and stop events by beat; at one beat a BPM change comes before a stop. STOP pulses become 1/48-beat units.
fn tempo_changes(file: &Bmson, resolution: f64, report: &mut BmsonReport) -> Vec<(f64, Change)> {
    let mut changes: Vec<(f64, u8, Change)> = Vec::new();
    for event in &file.bpm_events {
        if event.y >= 0 && event.bpm.is_finite() && event.bpm > 0.0 {
            changes.push((beat_of(event.y, resolution), 0, Change::Bpm(event.bpm)));
        } else {
            report.ignored_bpm_events += 1;
        }
    }
    for event in file.stop_events.iter().filter(|event| event.y >= 0 && event.duration > 0) {
        #[allow(clippy::cast_precision_loss)]
        changes.push((beat_of(event.y, resolution), 1, Change::Stop(event.duration as f64 / resolution * 48.0)));
    }
    changes.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    changes.into_iter().map(|(b, _, c)| (b, c)).collect()
}

/// The notes of every channel: playable drafts, background notes `(channel, pulse)`, and which channels hold a playable note.
struct Lanes {
    drafts: Vec<Draft>,
    bgm: Vec<(usize, i64)>,
    playable_channels: Vec<bool>,
}

impl Lanes {
    fn read(channels: &[SoundChannel], report: &mut BmsonReport) -> Self {
        let mut lanes = Self { drafts: Vec::new(), bgm: Vec::new(), playable_channels: vec![false; channels.len()] };
        for (index, channel) in channels.iter().enumerate() {
            for note in &channel.notes {
                let x = lane_of(note);
                if note.y < 0 {
                    report.notes_outside_layout += 1;
                } else if x <= 0 {
                    lanes.bgm.push((index, note.y));
                } else {
                    lanes.playable_channels[index] = true;
                    lanes.drafts.push(Draft { y: note.y, end: note.y.saturating_add(note.l.max(0)), x, channel: index });
                }
            }
        }
        // A background note at the pulse of a playable note of its channel is dropped (the spec: the slice is shared).
        let covered: std::collections::HashSet<(usize, i64)> = lanes.drafts.iter().map(|d| (d.channel, d.y)).collect();
        lanes.bgm.retain(|key| !covered.contains(key));
        lanes
    }

    /// The chart's audio: exactly one background note, alone in its channel, with a safe name. Returns the name and its pulse.
    fn single_audio(&self, channels: &[SoundChannel], report: &mut BmsonReport) -> Option<(String, i64)> {
        let single = match self.bgm[..] {
            [(channel, y)] if !self.playable_channels[channel] && channels[channel].notes.len() == 1 => {
                safe_name(&channels[channel].name, &mut report.unsafe_names).map(|name| (name, y))
            }
            _ => None,
        };
        report.single_audio = single.is_some();
        report.bgm_notes_skipped = if single.is_some() { 0 } else { self.bgm.len() };
        single
    }
}

/// The lane `x` of a note; 0 (background) when null, a float lane is rounded down.
fn lane_of(note: &BmsonNote) -> i64 {
    #[allow(clippy::cast_possible_truncation)] // a lane is small
    note.x.as_ref().and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f as i64))).unwrap_or(0)
}

/// Places the drafts on their columns, drops overlapping notes (counted) and gives each kept note its channel's sound.
fn add_notes(
    rox: &mut RoxChart,
    drafts: &[Draft],
    channels: &[SoundChannel],
    clock: &Clock,
    column_of: impl Fn(i64) -> Option<u8>,
    report: &mut BmsonReport,
) {
    let mut built: Vec<(Note, usize)> = Vec::with_capacity(drafts.len());
    for draft in drafts {
        let Some(column) = column_of(draft.x) else {
            report.notes_outside_layout += 1;
            continue;
        };
        let (start, end) = (clock.us(draft.y), clock.us(draft.end));
        let note = if end > start { Note::hold(start, end - start, column) } else { Note::tap(start, column) };
        built.push((note, draft.channel));
    }
    report.overlapping_notes_dropped += drop_stacked(&mut built, rox.key_count, |(note, _)| note).total();
    let mut sounds = HashMap::new();
    for (mut note, channel) in built {
        if let Some(name) = safe_name(&channels[channel].name, &mut report.unsafe_names) {
            note.hitsound_index = Some(keysound(rox, &mut sounds, name.clone(), || Hitsound::new(name)));
        }
        rox.notes.push(note);
    }
}

/// Slices every sound channel into [`SoundEvent`]s, as beatoraja's decoder does.
///
/// Per channel, the distinct pulses of its notes in order (a background note and a playable note of one pulse make ONE
/// event: the slice is shared). A pulse restarts the audio when any of its notes has `c` false: its slice then begins
/// at 0 of the file, else it begins where the previous slice ended. A slice lasts until the next pulse when that pulse
/// does not restart, and to the end of the file otherwise (and for the last pulse). Lengths are measured on the tempo
/// only, stops not counted: the music pauses at a stop. The spec's example table gives the restart pulses a file offset
/// that keeps growing; beatoraja and the spec's own text ("restart the audio") start them at 0, and so does this.
fn slice_plan(channels: &[SoundChannel], clock: &Clock, refused: &mut usize) -> AudioPlan {
    let mut events = Vec::new();
    for channel in channels {
        let Some(name) = safe_name(&channel.name, refused) else { continue };
        let pulses = pulses_of(channel);
        let mut start = 0_i64;
        for (index, &(y, restarts)) in pulses.iter().enumerate() {
            if restarts {
                start = 0;
            }
            let len = pulses.get(index + 1).filter(|(_, next_restarts)| !next_restarts).map(|&(next, _)| clock.tempo_len_us(y, next));
            events.push(SoundEvent {
                time_us: clock.us(y),
                file: name.as_str().into(),
                from_us: start,
                len_us: len,
                volume: None,
                index: None,
                optional: false,
            });
            start += len.unwrap_or(0);
        }
    }
    AudioPlan::new(events)
}

/// The distinct pulses of a channel's notes in order, and whether each restarts the audio.
fn pulses_of(channel: &SoundChannel) -> Vec<(i64, bool)> {
    let mut notes: Vec<&BmsonNote> = channel.notes.iter().filter(|n| n.y >= 0).collect();
    notes.sort_by_key(|n| n.y);
    let mut pulses: Vec<(i64, bool)> = Vec::new();
    for note in notes {
        match pulses.last_mut() {
            Some((y, restarts)) if *y == note.y => *restarts |= !note.c,
            _ => pulses.push((note.y, !note.c)),
        }
    }
    pulses
}

fn metadata(info: &Info, audio: Option<String>, refused: &mut usize) -> Metadata {
    let text = |s: &str| s.trim().to_string();
    let difficulty = [info.chart_name.trim(), info.subtitle.trim()]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or("Normal");
    let creator = info
        .subartists
        .iter()
        .find_map(|entry| {
            entry
                .split_once(':')
                .filter(|(key, _)| key.trim() == "chart")
                .map(|(_, v)| v.trim().to_string())
        })
        .unwrap_or_default();
    Metadata {
        title: text(&info.title).into(),
        artist: text(&info.artist).into(),
        creator: creator.into(),
        difficulty_name: difficulty.into(),
        #[allow(clippy::cast_possible_truncation)] // a level is small
        difficulty_value: info.level.filter(|l| l.is_finite()).map(|l| l as f32),
        genre: Some(text(&info.genre)).filter(|g| !g.is_empty()).map(Into::into),
        background_file: info
            .back_image
            .as_deref()
            .and_then(|n| safe_name(n, refused))
            .or_else(|| info.eyecatch_image.as_deref().and_then(|n| safe_name(n, refused)))
            .map(Into::into),
        audio_file: audio.unwrap_or_default().into(),
        ..Metadata::default()
    }
}
