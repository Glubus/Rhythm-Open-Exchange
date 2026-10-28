#![warn(clippy::pedantic)]
//! BMS / BME / BML / PMS decoder.
//!
//! See `mod.rs` for the decisions (layout, long notes, `#RANDOM`, audio). The steps are: text (`source.rs`),
//! lexing with control flow resolved (`parser.rs`), time (`timing.rs`), then this file: lanes, long notes,
//! columns and the ROX chart.

use std::cmp::Ordering;
use std::collections::HashMap;

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::audio_plan::{AudioPlan, SoundEvent};
use rox::model::{Hitsound, Metadata, Note, RoxChart, TimingPoint};

use super::parser::{self, BmsFile, Frac, id as obj_id};
use super::source;
use super::timing::{Beats, Change, Timeline};

/// Decoder of `.bms`, `.bme` and `.bml` files (same syntax, the extension is only a hint about the channels used).
pub struct BmsDecoder;
crate::impl_format!(BmsDecoder, ["bms", "bme", "bml"]);

/// Decoder of `.pms` files: a BMS-syntax file whose lanes are nine pop'n-music style buttons.
///
/// It is a separate decoder because the layout cannot be told from the bytes: only the extension says the chart is
/// nine-button.
pub struct PmsDecoder;
crate::impl_format!(PmsDecoder, ["pms"]);

impl Decoder for BmsDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        convert(data, Dialect::Bms).map(|(chart, _, _)| chart)
    }
}

impl Decoder for PmsDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        convert(data, Dialect::Pms).map(|(chart, _, _)| chart)
    }
}

impl BmsDecoder {
    /// Decodes `data` and reports what the decoder had to choose, skip or repair.
    ///
    /// # Errors
    /// Returns an error if the file has no playable object or does not give a valid chart.
    pub fn decode_with_report(data: &[u8]) -> RoxResult<(RoxChart, BmsReport)> {
        report_of(data, Dialect::Bms)
    }
}

impl BmsDecoder {
    /// [`Self::decode_with_report`] plus the [`AudioPlan`]: the background sounds (channel 01) and the keysound of every
    /// playable object (also the ones the chart drops, the free-zone lane, notes inside a hold), at their chart times.
    /// Invisible objects, mines and long-note tails make no sound.
    ///
    /// # Errors
    /// Returns an error if the file has no playable object or does not give a valid chart.
    pub fn decode_with_audio_plan(data: &[u8]) -> RoxResult<(RoxChart, BmsReport, AudioPlan)> {
        plan_of(data, Dialect::Bms)
    }
}

impl PmsDecoder {
    /// [`BmsDecoder::decode_with_audio_plan`] for a `.pms` file.
    ///
    /// # Errors
    /// Returns an error if the file has no playable object or does not give a valid chart.
    pub fn decode_with_audio_plan(data: &[u8]) -> RoxResult<(RoxChart, BmsReport, AudioPlan)> {
        plan_of(data, Dialect::Pms)
    }

    /// [`BmsDecoder::decode_with_report`] for a `.pms` file.
    ///
    /// # Errors
    /// Returns an error if the file has no playable object or does not give a valid chart.
    pub fn decode_with_report(data: &[u8]) -> RoxResult<(RoxChart, BmsReport)> {
        report_of(data, Dialect::Pms)
    }
}

fn report_of(data: &[u8], dialect: Dialect) -> RoxResult<(RoxChart, BmsReport)> {
    let (chart, report, _) = convert(data, dialect)?;
    chart.validate()?;
    Ok((chart, report))
}

fn plan_of(data: &[u8], dialect: Dialect) -> RoxResult<(RoxChart, BmsReport, AudioPlan)> {
    let (chart, report, plan) = convert(data, dialect)?;
    chart.validate()?;
    Ok((chart, report, plan))
}

/// What the decoder chose, skipped or repaired. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BmsReport {
    /// Charset the text was read with (`UTF-8`, `Shift_JIS`, `EUC-KR`, ...).
    pub charset: &'static str,
    /// Layout the lanes were put in, e.g. `1P 7K+scratch` (8 columns) or `PMS 9 buttons`.
    pub layout: String,
    /// `#IF`/`#ELSEIF`/`#ELSE` branches that were not taken (`#RANDOM` always draws 1).
    pub branches_skipped: usize,
    /// Data lines with an odd number of characters (the last one is ignored).
    pub odd_data_lines: usize,
    /// BPM changes ignored: not positive, not a number, or an undefined `#BPMxx`.
    pub ignored_bpm_changes: usize,
    /// `#LNOBJ` terminators with no note before them on their lane: dropped.
    pub orphan_long_note_ends: usize,
    /// Long-note heads (`#LNTYPE 1`, channels 5x/6x) that are never closed: decoded as taps.
    pub unclosed_long_notes: usize,
    /// Notes whose lane is not a column of the layout (foot pedal, a lane the layout does not have): dropped.
    pub notes_outside_layout: usize,
    /// Notes that start inside a long note or at the same place as another note of their column: dropped.
    pub overlapping_notes_dropped: usize,
    /// File names (`#WAVxx`, `#STAGEFILE`, `#BACKBMP`) refused because they are absolute or climb out of the folder: read as undefined.
    pub unsafe_names: usize,
    /// Channel-01 sounds (background music) that cannot be part of a ROX chart: not decoded.
    pub bgm_objects_skipped: usize,
    /// Whether the chart's audio is the one background sound of the file (see `mod.rs`).
    pub single_audio: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dialect {
    Bms,
    Pms,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Visible,
    Invisible,
    LongChannel,
    Mine,
}

/// A lane as the channel names it: `side` 0 for 1P (`1x`, `3x`, `5x`, `Dx`), 1 for 2P; `slot` 0..=4 keys 1..5
/// (channels x1..x5), 5 and 6 keys 6 and 7 (x8, x9), 7 the scratch (x6), 8 the free zone (x7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Lane {
    side: u8,
    slot: u8,
}

const SCRATCH: u8 = 7;
const FREE_ZONE: u8 = 8;

fn classify(channel: [u8; 2]) -> Option<(Kind, Lane)> {
    let (kind, side) = match channel[0] {
        b'1' => (Kind::Visible, 0),
        b'2' => (Kind::Visible, 1),
        b'3' => (Kind::Invisible, 0),
        b'4' => (Kind::Invisible, 1),
        b'5' => (Kind::LongChannel, 0),
        b'6' => (Kind::LongChannel, 1),
        b'D' => (Kind::Mine, 0),
        b'E' => (Kind::Mine, 1),
        _ => return None,
    };
    let slot = match channel[1] {
        b'1'..=b'5' => channel[1] - b'1',
        b'8' => 5,
        b'9' => 6,
        b'6' => SCRATCH,
        b'7' => FREE_ZONE,
        _ => return None,
    };
    Some((kind, Lane { side, slot }))
}

#[derive(Debug, Clone, Copy)]
struct Pos {
    measure: u16,
    frac: Frac,
}

fn pos_cmp(a: Pos, b: Pos) -> Ordering {
    a.measure.cmp(&b.measure).then_with(|| a.frac.cmp(b.frac))
}

#[derive(Debug, Clone, Copy)]
struct Item {
    pos: Pos,
    unit: u32,
    pair: [u8; 2],
}

impl Item {
    fn id(&self) -> u16 {
        obj_id(self.pair).unwrap_or(0)
    }

    /// Index of the object in the slots of the line it came from.
    fn slot(&self) -> u32 {
        u32::try_from(
            u64::from(self.pos.frac.num) * u64::from(self.unit) / u64::from(self.pos.frac.den),
        )
        .unwrap_or(0)
    }
}

/// Sorts by position and keeps, at one position, the object of the last line of the file: the lines of one channel of
/// one measure are merged and a later object overrides an earlier one.
fn merge(mut items: Vec<Item>) -> Vec<Item> {
    items.sort_by(|a, b| pos_cmp(a.pos, b.pos));
    let mut out: Vec<Item> = Vec::with_capacity(items.len());
    for item in items {
        match out.last_mut() {
            Some(last) if pos_cmp(last.pos, item.pos) == Ordering::Equal => *last = item,
            _ => out.push(item),
        }
    }
    out
}

/// A playable object before columns and times.
#[derive(Debug, Clone, Copy)]
struct Draft {
    pos: Pos,
    end: Option<Pos>,
    lane: Lane,
    mine: bool,
    id: u16,
}

/// The position of the slot after the one of `item` (the first empty slot of a run).
fn slot_after(item: &Item) -> Pos {
    let next = item.slot() + 1;
    if next >= item.unit {
        Pos {
            measure: item.pos.measure + 1,
            frac: Frac::new(0, 1),
        }
    } else {
        Pos {
            measure: item.pos.measure,
            frac: Frac::new(next, item.unit),
        }
    }
}

#[allow(clippy::too_many_lines)] // one pass: text, tempo, objects, layout, audio, notes
fn convert(data: &[u8], dialect: Dialect) -> RoxResult<(RoxChart, BmsReport, AudioPlan)> {
    let text = source::decode(data);
    let mut file = parser::parse(&text.text);
    let unsafe_names = sanitize_names(&mut file);
    let mut report = BmsReport {
        charset: text.charset,
        branches_skipped: file.report.branches_skipped,
        odd_data_lines: file.report.odd_data_lines,
        unsafe_names,
        ..BmsReport::default()
    };

    // Channels
    let mut lanes: HashMap<(Kind, Lane), Vec<Item>> = HashMap::new();
    let mut bgm = Vec::new();
    let (mut ch03, mut ch08, mut ch09) = (Vec::new(), Vec::new(), Vec::new());
    for line in &file.lines {
        let items = line.objs.iter().map(|obj| Item {
            pos: Pos {
                measure: line.measure,
                frac: obj.frac,
            },
            unit: obj.unit,
            pair: obj.pair,
        });
        match &line.channel {
            b"01" => bgm.extend(items),
            b"03" => ch03.extend(items),
            b"08" => ch08.extend(items),
            b"09" => ch09.extend(items),
            channel => {
                if let Some(key) = classify(*channel) {
                    lanes.entry(key).or_default().extend(items);
                }
            }
        }
    }

    // Time
    let beats = Beats::new(&file.measure_len);
    let initial_bpm = file
        .headers
        .get("BPM")
        .and_then(|v| v.split_ascii_whitespace().next())
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|bpm| bpm.is_finite() && *bpm > 0.0)
        .unwrap_or(130.0);
    let timeline = Timeline::new(
        initial_bpm,
        &tempo_changes(&file, &beats, ch03, ch08, ch09, &mut report),
    );

    // Objects
    let drafts = drafts(&file, lanes, &mut report);
    if drafts.iter().all(|d| d.mine) {
        return Err(RoxError::InvalidFormat(
            "BMS file with no playable object (channels 11-19, 21-29, 51-69)".into(),
        ));
    }
    let layout = Layout::detect(dialect, &drafts);
    report.layout.clone_from(&layout.description);

    // Audio: the one background sound of the file, when there is exactly one.
    let bgm_all: Vec<Item> = bgm.clone();
    let bgm: Vec<Item> = bgm
        .into_iter()
        .filter(|item| {
            file.wav
                .get(&item.id())
                .is_some_and(|name| !name.is_empty())
        })
        .collect();
    let single_audio = (bgm.len() == 1).then(|| bgm[0]);
    report.single_audio = single_audio.is_some();
    report.bgm_objects_skipped = if single_audio.is_some() { 0 } else { bgm.len() };
    let to_us = |seconds: f64| -> i64 {
        #[allow(clippy::cast_possible_truncation)] // seconds to µs, any real chart fits
        let us = (seconds * 1_000_000.0).round() as i64;
        us
    };
    let shift_us = single_audio.map_or(0, |item| {
        -to_us(timeline.seconds(beats.beat(item.pos.measure, item.pos.frac)))
    });

    let mut rox = RoxChart::new(layout.key_count);
    rox.metadata = metadata(&file, single_audio.as_ref());
    for point in timeline.bpm_points() {
        #[allow(clippy::cast_possible_truncation)] // a BPM fits f32
        rox.timing_points.push(TimingPoint::bpm(
            to_us(point.seconds) + shift_us,
            point.bpm as f32,
        ));
    }

    // Notes, sorted, with the overlaps of a column removed.
    let mut built: Vec<(Note, Option<u16>)> = Vec::with_capacity(drafts.len());
    for draft in &drafts {
        let Some(column) = layout.column(draft.lane) else {
            report.notes_outside_layout += 1;
            continue;
        };
        let time = |pos: Pos| to_us(timeline.seconds(beats.beat(pos.measure, pos.frac))) + shift_us;
        let start = time(draft.pos);
        let note = if draft.mine {
            Note::mine(start, column)
        } else {
            match draft.end.map(time) {
                Some(end) if end > start => Note::hold(start, end - start, column),
                _ => Note::tap(start, column),
            }
        };
        built.push((note, (!draft.mine).then_some(draft.id)));
    }
    built.sort_by_key(|(note, _)| (note.time_us, kind_rank(note), note.column));
    let mut last_end = vec![i64::MIN; usize::from(layout.key_count)];
    let mut last_start = vec![i64::MIN; usize::from(layout.key_count)];
    let mut sounds: HashMap<u16, u16> = HashMap::new();
    for (mut note, id) in built {
        let column = usize::from(note.column);
        if note.time_us < last_end[column] || note.time_us == last_start[column] {
            report.overlapping_notes_dropped += 1;
            continue;
        }
        last_end[column] = note.end_time_us();
        last_start[column] = note.time_us;
        if let Some(id) = id
            && let Some(name) = file.wav.get(&id).filter(|name| !name.is_empty())
        {
            let index = *sounds.entry(id).or_insert_with(|| {
                rox.hitsounds.push(Hitsound::new(name.as_str()));
                u16::try_from(rox.hitsounds.len() - 1).unwrap_or(u16::MAX)
            });
            note.hitsound_index = Some(index);
        }
        rox.notes.push(note);
    }
    let plan = audio_plan(&file, &bgm_all, &drafts, &timeline, &beats, shift_us, &to_us);
    Ok((rox, report, plan))
}

/// Background sounds and the keysound of every playable object, at chart time.
fn audio_plan(
    file: &BmsFile,
    bgm: &[Item],
    drafts: &[Draft],
    timeline: &Timeline,
    beats: &Beats,
    shift_us: i64,
    to_us: &dyn Fn(f64) -> i64,
) -> AudioPlan {
    let at = |pos: Pos| to_us(timeline.seconds(beats.beat(pos.measure, pos.frac))) + shift_us;
    let sound = |id: u16| file.wav.get(&id).filter(|name| !name.is_empty());
    let mut events = Vec::new();
    for item in bgm {
        if let Some(name) = sound(item.id()) {
            events.push(SoundEvent::whole(at(item.pos), name.as_str()));
        }
    }
    for draft in drafts.iter().filter(|d| !d.mine) {
        if let Some(name) = sound(draft.id) {
            events.push(SoundEvent::whole(at(draft.pos), name.as_str()));
        }
    }
    AudioPlan::new(events)
}

/// Empties every file name that is absolute, has a drive letter or climbs out of the folder; returns how many.
fn sanitize_names(file: &mut BmsFile) -> usize {
    let mut refused = 0;
    for name in file.wav.values_mut() {
        *name = super::bmson::safe_name(name, &mut refused).unwrap_or_default();
    }
    for key in ["STAGEFILE", "BACKBMP"] {
        if let Some(name) = file.headers.get_mut(key) {
            *name = super::bmson::safe_name(name, &mut refused).unwrap_or_default();
        }
    }
    refused
}

fn kind_rank(note: &Note) -> u8 {
    if note.is_hold() {
        0
    } else if note.is_mine() {
        2
    } else {
        1
    }
}

/// BPM and STOP changes with their beat, sorted: at one beat the BPM changes come before the STOP, channel 03 before 08.
fn tempo_changes(
    file: &BmsFile,
    beats: &Beats,
    ch03: Vec<Item>,
    ch08: Vec<Item>,
    ch09: Vec<Item>,
    report: &mut BmsReport,
) -> Vec<(f64, Change)> {
    let mut changes: Vec<(f64, u8, Change)> = Vec::new();
    for item in merge(ch03) {
        let value = std::str::from_utf8(&item.pair)
            .ok()
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .filter(|v| *v > 0);
        match value {
            Some(bpm) => changes.push((beat_of(beats, item), 0, Change::Bpm(f64::from(bpm)))),
            None => report.ignored_bpm_changes += 1,
        }
    }
    for item in merge(ch08) {
        match file.bpm_table.get(&item.id()).filter(|bpm| **bpm > 0.0) {
            Some(bpm) => changes.push((beat_of(beats, item), 0, Change::Bpm(*bpm))),
            None => report.ignored_bpm_changes += 1,
        }
    }
    for item in merge(ch09) {
        if let Some(stop) = file.stop_table.get(&item.id()).filter(|stop| **stop > 0.0) {
            changes.push((beat_of(beats, item), 1, Change::Stop(*stop)));
        }
    }
    changes.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    changes
        .into_iter()
        .map(|(beat, _, change)| (beat, change))
        .collect()
}

fn beat_of(beats: &Beats, item: Item) -> f64 {
    beats.beat(item.pos.measure, item.pos.frac)
}

/// Every playable object of the file with its lane, long notes resolved.
fn drafts(
    file: &BmsFile,
    lanes: HashMap<(Kind, Lane), Vec<Item>>,
    report: &mut BmsReport,
) -> Vec<Draft> {
    let mut keys: Vec<(Kind, Lane)> = lanes.keys().copied().collect();
    // Lane order makes the result independent of the hash map order.
    keys.sort_by_key(|(kind, lane)| (*kind as u8, lane.side, lane.slot));
    let mut lanes = lanes;
    let mut out = Vec::new();
    for key in keys {
        let (kind, lane) = key;
        let items = merge(lanes.remove(&key).unwrap_or_default());
        match kind {
            Kind::Invisible => {}
            Kind::Mine => out.extend(items.iter().map(|item| Draft {
                pos: item.pos,
                end: None,
                lane,
                mine: true,
                id: item.id(),
            })),
            Kind::Visible => visible(file, lane, &items, &mut out, report),
            Kind::LongChannel => {
                if file.lntype == 2 {
                    long_runs(lane, &items, &mut out);
                } else {
                    long_pairs(lane, &items, &mut out, report);
                }
            }
        }
    }
    out
}

/// Notes of a visible lane; with `#LNOBJ`, the terminator closes the note before it.
fn visible(
    file: &BmsFile,
    lane: Lane,
    items: &[Item],
    out: &mut Vec<Draft>,
    report: &mut BmsReport,
) {
    let first = out.len();
    for item in items {
        if file.lnobj == Some(item.id()) {
            match out[first..].last_mut() {
                Some(last) if last.end.is_none() => last.end = Some(item.pos),
                _ => report.orphan_long_note_ends += 1,
            }
            continue;
        }
        out.push(Draft {
            pos: item.pos,
            end: None,
            lane,
            mine: false,
            id: item.id(),
        });
    }
}

/// `#LNTYPE 1`: the objects of a long-note channel alternate start, end, start, end...
fn long_pairs(lane: Lane, items: &[Item], out: &mut Vec<Draft>, report: &mut BmsReport) {
    for pair in items.chunks(2) {
        if pair.len() == 2 {
            out.push(Draft {
                pos: pair[0].pos,
                end: Some(pair[1].pos),
                lane,
                mine: false,
                id: pair[0].id(),
            });
        } else {
            report.unclosed_long_notes += 1;
            out.push(Draft {
                pos: pair[0].pos,
                end: None,
                lane,
                mine: false,
                id: pair[0].id(),
            });
        }
    }
}

/// `#LNTYPE 2` (MGQ): a long note is a run of consecutive slots with an object; it ends at the first empty slot. A run
/// continues over a bar line when the last slot of a measure and the first of the next both hold an object.
fn long_runs(lane: Lane, items: &[Item], out: &mut Vec<Draft>) {
    let mut i = 0;
    while i < items.len() {
        let head = items[i];
        let mut last = head;
        while i + 1 < items.len() && continues(&last, &items[i + 1]) {
            i += 1;
            last = items[i];
        }
        out.push(Draft {
            pos: head.pos,
            end: Some(slot_after(&last)),
            lane,
            mine: false,
            id: head.id(),
        });
        i += 1;
    }
}

fn continues(last: &Item, next: &Item) -> bool {
    if next.pos.measure == last.pos.measure {
        next.unit == last.unit && next.slot() == last.slot() + 1
    } else {
        next.pos.measure == last.pos.measure + 1 && last.slot() + 1 == last.unit && next.slot() == 0
    }
}

/// Columns of the ROX chart.
struct Layout {
    columns: [[Option<u8>; 9]; 2],
    key_count: u8,
    description: String,
}

impl Layout {
    fn column(&self, lane: Lane) -> Option<u8> {
        self.columns[usize::from(lane.side)][usize::from(lane.slot)]
    }

    /// Layout from the lanes the chart really uses, so it never depends on `#PLAYER` (players ignore it too).
    ///
    /// Beat/BME (`.bms`, `.bme`, `.bml`), columns in this order:
    /// - one side (1P, or 2P alone, treated as a single chart): `[scratch] key1..keyN`;
    /// - both sides (double play): `[scratch1] key1..keyN | key1..keyN [scratch2]`.
    ///
    /// `N` is 7 when any lane x8/x9 (keys 6 and 7) is used, else 5. A scratch column exists only when the scratch is used
    /// (for double play, on both sides as soon as either is used, so the two halves mirror).
    ///
    /// PMS: 9 columns, buttons 1..9 left to right: `11 12 13 14 15 22 23 24 25` (standard), or
    /// `11 12 13 14 15 18 19 16 17` (BME-type) when the chart has no 2P lane and uses channels 16..19.
    fn detect(dialect: Dialect, drafts: &[Draft]) -> Self {
        let mut used = [[false; 9]; 2];
        for draft in drafts.iter().filter(|d| !d.mine) {
            if dialect == Dialect::Bms && draft.lane.slot == FREE_ZONE {
                continue;
            }
            used[usize::from(draft.lane.side)][usize::from(draft.lane.slot)] = true;
        }
        let mut columns = [[None; 9]; 2];
        if dialect == Dialect::Pms {
            let bme_type = !used[1].iter().any(|u| *u) && used[0][5..9].iter().any(|u| *u);
            let buttons: [Lane; 9] = if bme_type {
                [0, 1, 2, 3, 4, 5, 6, 7, 8].map(|slot| Lane { side: 0, slot })
            } else {
                [
                    (0, 0),
                    (0, 1),
                    (0, 2),
                    (0, 3),
                    (0, 4),
                    (1, 1),
                    (1, 2),
                    (1, 3),
                    (1, 4),
                ]
                .map(|(side, slot)| Lane { side, slot })
            };
            for (button, lane) in buttons.iter().enumerate() {
                columns[usize::from(lane.side)][usize::from(lane.slot)] =
                    Some(u8::try_from(button).unwrap_or(0));
            }
            return Self {
                columns,
                key_count: 9,
                description: format!(
                    "PMS 9 buttons ({})",
                    if bme_type { "BME-type" } else { "standard" }
                ),
            };
        }
        let sides: Vec<usize> = (0..2).filter(|s| used[*s].iter().any(|u| *u)).collect();
        let keys7 = sides.iter().any(|s| used[*s][5] || used[*s][6]);
        let n: u8 = if keys7 { 7 } else { 5 };
        let double = sides.len() == 2;
        let scratch = |side: usize| used[side][usize::from(SCRATCH)];
        let mut next = 0u8;
        let mut assign = |side: usize, slot: usize, columns: &mut [[Option<u8>; 9]; 2]| {
            columns[side][slot] = Some(next);
            next += 1;
        };
        if double {
            let both = scratch(0) || scratch(1);
            if both {
                assign(0, usize::from(SCRATCH), &mut columns);
            }
            for key in 0..n {
                assign(0, usize::from(key), &mut columns);
            }
            for key in 0..n {
                assign(1, usize::from(key), &mut columns);
            }
            if both {
                assign(1, usize::from(SCRATCH), &mut columns);
            }
            let key_count = next;
            return Self {
                columns,
                key_count,
                description: format!(
                    "2P {n}K{} (double, {key_count} columns)",
                    if both { "+scratch" } else { "" }
                ),
            };
        }
        // `sides` is never empty here: the caller refuses a file with no playable object.
        let side = sides.first().copied().unwrap_or(0);
        let has_scratch = scratch(side);
        if has_scratch {
            assign(side, usize::from(SCRATCH), &mut columns);
        }
        for key in 0..n {
            assign(side, usize::from(key), &mut columns);
        }
        Self {
            columns,
            key_count: next,
            description: format!(
                "{}P {n}K{} ({next} columns)",
                side + 1,
                if has_scratch { "+scratch" } else { "" }
            ),
        }
    }
}

fn metadata(file: &BmsFile, audio: Option<&Item>) -> Metadata {
    let header = |name: &str| {
        file.headers
            .get(name)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
    };
    let difficulty = header("DIFFICULTY").and_then(|d| match d.parse::<u8>().ok()? {
        1 => Some("Beginner"),
        2 => Some("Normal"),
        3 => Some("Hyper"),
        4 => Some("Another"),
        5 => Some("Insane"),
        _ => None,
    });
    let mut metadata = Metadata {
        title: header("TITLE").unwrap_or_default().into(),
        artist: header("ARTIST").unwrap_or_default().into(),
        creator: header("MAKER").unwrap_or_default().into(),
        difficulty_name: difficulty
            .or_else(|| header("SUBTITLE"))
            .unwrap_or("Normal")
            .into(),
        difficulty_value: header("PLAYLEVEL")
            .and_then(|level| level.parse::<f32>().ok())
            .filter(|level| level.is_finite()),
        genre: header("GENRE").map(Into::into),
        background_file: header("STAGEFILE")
            .or_else(|| header("BACKBMP"))
            .map(|name| name.replace('\\', "/").into()),
        ..Metadata::default()
    };
    if let Some(item) = audio
        && let Some(name) = file.wav.get(&item.id())
    {
        metadata.audio_file = name.as_str().into();
    }
    metadata
}
