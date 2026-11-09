#![warn(clippy::pedantic)]
//! `.dtx` decoder: chips to a ROX chart per instrument, plus the audio plan.
//!
//! See `mod.rs` for the decisions. The steps are: text (`bms::source`), lexing (`parser.rs`), the tempo map (the shared
//! `bms::timing`), then one pass per instrument: lanes to columns, holds, notes and sounds.

use std::collections::{BTreeMap, HashMap};

use rox::audio_plan::{AudioPlan, SoundEvent};
use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Hitsound, Metadata, Note, RoxChart, TimingPoint};

pub use super::channels::Part;
use super::channels::{BLUE, GREEN, OPEN, PINK, Pad, RED, Role, YELLOW, classify};
use super::parser::{self, Chip, DtxFile, Sound};
use crate::formats::bms::Frac;
use crate::formats::bms::source;
use crate::formats::bms::timing::{Beats, Change, Timeline};

/// Decoder of `.dtx` files: the first instrument of the file that has chips (see [`DtxDecoder::variants`] for the others).
pub struct DtxDecoder;
crate::impl_format!(DtxDecoder, ["dtx"]);

/// BPM of a chart that gives none (`DTXMania` starts at 120).
const DEFAULT_BPM: f64 = 120.0;

/// The drum columns, left to right: left cymbal, hi-hat (closed and open), snare, high tom, bass drum (and the left pedal),
/// low tom, floor tom, cymbal (right crash), ride.
const DRUM_LANES: [&str; 9] = ["LC", "HH", "SD", "HT", "BD", "LT", "FT", "CY", "RD"];

/// A playable instrument of the file. An instrument without a single chip is not a variant, so `index` counts the
/// instruments that have chips from 0 (Drums, Guitar, Bass in that order).
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    /// Number to give to [`DtxDecoder::decode_variant`]: `0..variants.len()`.
    pub index: usize,
    pub part: Part,
    /// The level the file gives the instrument (`#DLEVEL`, `#GLEVEL`, `#BLEVEL`), as the game shows it (`7.60`); `None` when absent or 0.
    pub level: Option<f32>,
    /// Notes of the decoded chart (a hold is one).
    pub notes: usize,
    /// Holds among them.
    pub holds: usize,
    pub key_count: u8,
    pub duration_us: i64,
}

/// What the decoder chose, skipped or repaired for one instrument. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DtxReport {
    /// Charset the text was read with (`UTF-8`, `Shift_JIS`, `EUC-KR`, ...).
    pub charset: &'static str,
    /// The instrument this report is about.
    pub part: Part,
    /// The columns, e.g. `Drums 9 columns: LC HH SD HT BD LT FT CY RD` or `Guitar 6 columns: Open R G B Y P`.
    pub layout: String,
    /// `#IF` blocks that were not taken (`#RANDOM` always draws 1).
    pub branches_skipped: usize,
    /// Data lines refused: a character that is not base 36.
    pub invalid_data_lines: usize,
    /// Data lines with an odd number of characters (the last one is ignored).
    pub odd_data_lines: usize,
    /// Channel-`02` lines whose length is not a positive number: ignored.
    pub ignored_bar_lengths: usize,
    /// BPM changes ignored: a channel-`03` pair that is not hexadecimal, or a channel-`08` object with no `#BPMzz`.
    pub ignored_bpm_changes: usize,
    /// Drum chips of the hidden channels (`0x31..=0x3C`): dropped, no sound.
    pub hidden_chips: usize,
    /// Blank-hit sound settings (`0xB1..=0xBE`): dropped.
    pub blank_hit_sounds: usize,
    /// Wailing chips of the guitar and bass (`0x28`, `0xA8`) and the wailing sound (`0x2F`): dropped.
    pub wailing_chips: usize,
    /// Drum fill-in markers (`0x1F`, `0x53`): dropped.
    pub fill_in_chips: usize,
    /// Picture, video, bar-line and beat-line chips: dropped.
    pub visual_chips: usize,
    /// Chips of channels that exist but change nothing here (bonus effects, MIDI chorus, click, mixer, flow speed): dropped.
    pub inert_chips: usize,
    /// Chips of channels this decoder does not know: dropped.
    pub unknown_chips: usize,
    /// File names (`#WAVzz`, `#STAGEFILE`, `#PREIMAGE`, `#BACKGROUND`) refused because they are absolute or climb out of the folder.
    pub unsafe_names: usize,
    /// `#DTXVPLAYSPEED` was given: it is a speed of the `DTXViewer` playback, not read.
    pub dtxv_play_speed_ignored: bool,
    /// `#PATH_WAV` was given: a folder of the player's machine, not read (the names stay relative to the chart's folder).
    pub path_wav_ignored: bool,
    /// Drums: notes of the open hi-hat channel, played on the hi-hat column (the game gives both one lane).
    pub hi_hat_open_notes: usize,
    /// Drums: notes of the left pedal and left bass drum channels, folded onto the bass drum column.
    pub left_pedal_notes_folded: usize,
    /// Chips of the other two instruments: not part of this chart (and not in its audio plan).
    pub other_instrument_chips: usize,
    /// Guitar and bass: hold markers that start no hold (no note at their position, a note inside the hold, or none left to close).
    pub hold_markers_dropped: usize,
    /// Notes that start inside a hold or at the same time as another note of their column: dropped (their sound stays in the plan).
    pub overlapping_notes_dropped: usize,
    /// Background sounds of the file (channel `01` and the effect channels `0x61..=0x92`).
    pub background_sounds: usize,
    /// Whether the chart's audio is the one background sound of the file (see `mod.rs`).
    pub single_audio: bool,
}

/// An instrument, converted.
struct Built {
    chart: RoxChart,
    report: DtxReport,
    plan: AudioPlan,
    part: Part,
    level: Option<f32>,
    holds: usize,
}

impl Decoder for DtxDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode_nth(data, 0).map(|built| built.chart)
    }
}

impl DtxDecoder {
    /// The instruments of the file that have chips, in the order Drums, Guitar, Bass.
    ///
    /// # Errors
    /// Returns an error if the file has no chip of any instrument.
    pub fn variants(data: &[u8]) -> RoxResult<Vec<Variant>> {
        Ok(build_all(data, usize::MAX)?
            .iter()
            .enumerate()
            .map(|(index, built)| variant_of(index, built))
            .collect())
    }

    /// Every instrument of the file, each with its [`Variant`] and its validated chart (an instrument whose chart is not valid is
    /// its own `Err`), from ONE read of the file: what [`Self::variants`] then [`Self::decode_variant`] for each give, without
    /// reading the text three more times.
    ///
    /// # Errors
    /// Returns an error if the file has no chip of any instrument.
    pub fn decode_all(data: &[u8]) -> RoxResult<Vec<(Variant, RoxResult<RoxChart>)>> {
        Ok(build_all(data, usize::MAX)?
            .into_iter()
            .enumerate()
            .map(|(index, built)| {
                let variant = variant_of(index, &built);
                (variant, built.chart.validate().map(|()| built.chart))
            })
            .collect())
    }

    /// Decodes instrument `index` (the numbering of [`Self::variants`]).
    ///
    /// # Errors
    /// Returns an error if the file has no chip, or no such instrument.
    pub fn decode_variant(data: &[u8], index: usize) -> RoxResult<RoxChart> {
        decode_nth(data, index).map(|built| built.chart)
    }

    /// [`Self::decode_variant`] and what the decoder chose, skipped or repaired.
    ///
    /// # Errors
    /// Returns an error if the file has no chip, or no such instrument.
    pub fn decode_with_report(data: &[u8], index: usize) -> RoxResult<(RoxChart, DtxReport)> {
        decode_nth(data, index).map(|built| (built.chart, built.report))
    }

    /// [`Self::decode_with_report`] plus the [`AudioPlan`]: the background sounds (channel `01` and the effect channels), and the
    /// sound of every chip of the instrument (also the ones the chart drops: the left pedal folded onto the bass drum, a note
    /// inside a hold), at their chart times. The chips of the other instruments, hidden chips, blank-hit settings and wailing
    /// chips make no sound. Each event carries its `#VOLUMEzz` (when not 100); `#PANzz` is not used (a plan has no pan).
    ///
    /// # Errors
    /// Returns an error if the file has no chip, or no such instrument.
    pub fn decode_with_audio_plan(
        data: &[u8],
        index: usize,
    ) -> RoxResult<(RoxChart, DtxReport, AudioPlan)> {
        decode_nth(data, index).map(|built| (built.chart, built.report, built.plan))
    }
}

fn variant_of(index: usize, built: &Built) -> Variant {
    Variant {
        index,
        part: built.part,
        level: built.level,
        notes: built.chart.notes.len(),
        holds: built.holds,
        key_count: built.chart.key_count,
        duration_us: built.chart.duration_us(),
    }
}

fn decode_nth(data: &[u8], index: usize) -> RoxResult<Built> {
    let mut built = build_all(data, index.saturating_add(1))?;
    if index < built.len() {
        let one = built.swap_remove(index);
        one.chart.validate()?;
        Ok(one)
    } else {
        Err(RoxError::InvalidFormat(format!(
            "dtx: {} instruments with chips, not instrument {index}",
            built.len()
        )))
    }
}

/// What every instrument of a file shares: the text, the tables and the tempo map.
struct Song {
    charset: &'static str,
    file: DtxFile,
    beats: Beats,
    timeline: Timeline,
    ignored_bpm_changes: usize,
}

impl Song {
    fn new(data: &[u8]) -> Self {
        let text = source::decode(data);
        let file = parser::parse(&text.text);
        // The length of a measure holds until the next channel-`02` line (unlike BMS, where it is the measure's alone).
        let mut lengths = BTreeMap::new();
        let mut length = 1.0;
        for measure in 0..3600u16 {
            if let Some(set) = file.measure_len.get(&measure) {
                length = *set;
            }
            if (length - 1.0_f64).abs() > f64::EPSILON {
                lengths.insert(measure, length);
            }
        }
        let beats = Beats::new(&lengths);
        let base = file.base_bpm.unwrap_or(0.0);
        let mut ignored = 0;
        let mut changes: Vec<(f64, u8, u32, Change)> = Vec::new();
        for chip in &file.chips {
            match classify(chip.channel) {
                Role::Bpm => {
                    let change = Change::Bpm(base + f64::from(chip.id));
                    changes.push((beat_of(&beats, chip), 0, chip.seq, change));
                }
                Role::BpmTable => match file.bpm.at(chip.id, chip.seq) {
                    Some(bpm) => {
                        let bpm = if chip.id == 0 { *bpm } else { base + bpm };
                        changes.push((beat_of(&beats, chip), 1, chip.seq, Change::Bpm(bpm)));
                    }
                    None => ignored += 1,
                },
                _ => {}
            }
        }
        changes.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let initial = file.bpm.first(0).copied().unwrap_or(DEFAULT_BPM);
        let changes: Vec<(f64, Change)> = changes
            .into_iter()
            .filter(|(_, _, _, change)| matches!(change, Change::Bpm(bpm) if bpm.is_finite() && *bpm > 0.0))
            .map(|(beat, _, _, change)| (beat, change))
            .collect();
        let timeline = Timeline::new(initial, &changes);
        Self {
            charset: text.charset,
            ignored_bpm_changes: file.report.ignored_bpm_changes + ignored,
            file,
            beats,
            timeline,
        }
    }

    /// Seconds of a chip on the chart's axis (the start of measure 000 is 0).
    fn seconds(&self, chip: &Chip) -> f64 {
        self.timeline.seconds(beat_of(&self.beats, chip))
    }

    fn sound(&self, chip: &Chip) -> Option<&Sound> {
        self.file
            .wav
            .at(chip.id, chip.seq)
            .filter(|sound| !sound.name.is_empty())
    }
}

fn beat_of(beats: &Beats, chip: &Chip) -> f64 {
    beats.beat(chip.measure, Frac::new(u32::from(chip.tick), 384))
}

/// Times are kept within about 36 000 years so the sums of the converter can not overflow (a tempo close to 0 sends them far).
const MAX_US: i64 = 1 << 60;

fn to_us(seconds: f64) -> i64 {
    #[allow(clippy::cast_possible_truncation)] // seconds to µs, clamped just below
    let us = (seconds * 1_000_000.0).round() as i64;
    us.clamp(-MAX_US, MAX_US)
}

/// The instruments that have chips, converted, stopping once `want` of them are built.
fn build_all(data: &[u8], want: usize) -> RoxResult<Vec<Built>> {
    let song = Song::new(data);
    let mut present = [false; 3];
    for chip in &song.file.chips {
        match classify(chip.channel) {
            Role::Drum(_) => present[Part::Drums.slot()] = true,
            Role::Fret { part, .. } => present[part.slot()] = true,
            _ => {}
        }
    }
    if !present.iter().any(|p| *p) {
        return Err(RoxError::InvalidFormat(
            "DTX file with no chip of an instrument (drums 11-1C, guitar 20-27, bass A0-A7 and their extensions)".into(),
        ));
    }
    let mut out = Vec::new();
    for part in Part::ALL {
        if out.len() >= want {
            break;
        }
        if present[part.slot()] {
            out.push(convert(&song, part));
        }
    }
    Ok(out)
}

/// A chip of the chosen instrument before columns.
struct Hit<'a> {
    chip: Chip,
    /// `measure * 384 + tick`.
    pos: u32,
    time_us: i64,
    /// The drum slot (`DRUM_LANES`), or the buttons of a guitar or bass chip.
    what: u8,
    sound: Option<&'a Sound>,
}

fn position(chip: &Chip) -> u32 {
    u32::from(chip.measure) * 384 + u32::from(chip.tick)
}

/// The slot of a drum channel in `DRUM_LANES`.
fn drum_slot(drum: Pad) -> u8 {
    match drum {
        Pad::LeftCymbal => 0,
        Pad::HiHatClose | Pad::HiHatOpen => 1,
        Pad::Snare => 2,
        Pad::HighTom => 3,
        Pad::BassDrum | Pad::LeftPedal | Pad::LeftBassDrum => 4,
        Pad::LowTom => 5,
        Pad::FloorTom => 6,
        Pad::Cymbal => 7,
        Pad::Ride => 8,
    }
}

fn convert(song: &Song, part: Part) -> Built {
    let mut report = report_of(song, part);
    let mut chips = PartChips::sort(song, part, &mut report);

    // The audio of the chart: the one background sound, when it is the only sound of the chart (see `mod.rs`).
    let single = match chips.background[..] {
        [only] if chips.hits.iter().all(|h| h.sound.is_none()) => Some(only),
        _ => None,
    };
    report.single_audio = single.is_some();
    let shift_us = single.map_or(0, |(chip, _)| -to_us(song.seconds(&chip)));
    for hit in &mut chips.hits {
        hit.time_us = to_us(song.seconds(&hit.chip)) + shift_us;
    }
    chips.hits.sort_by(|a, b| a.pos.cmp(&b.pos).then(a.chip.channel.cmp(&b.chip.channel)));

    let columns = Columns::new(part, &chips.hits);
    report.layout.clone_from(&columns.description);
    let mut rox = RoxChart::new(columns.key_count);
    rox.metadata = metadata(song, part, single.map(|(_, sound)| sound));
    for point in song.timeline.bpm_points() {
        #[allow(clippy::cast_possible_truncation)] // a BPM fits f32
        rox.timing_points.push(TimingPoint::bpm(to_us(point.seconds) + shift_us, point.bpm as f32));
    }

    // Holds (guitar and bass).
    let ends = holds(song, &chips.hits, &chips.markers);
    report.hold_markers_dropped = chips.markers.len() - 2 * ends.len();
    let end_us = |index: usize| ends.get(&index).map(|end| to_us(song.timeline.seconds(*end)) + shift_us);
    add_notes(&mut rox, &chips.hits, &columns, end_us, &mut report);
    let holds = rox.notes.iter().filter(|note| note.is_hold()).count();
    let plan = audio_plan(song, &chips, shift_us);
    Built { chart: rox, report, plan, part, level: level_of(song, part), holds }
}

/// The report of an instrument, with what the parser counted for the whole file.
fn report_of(song: &Song, part: Part) -> DtxReport {
    let parsed = &song.file.report;
    DtxReport {
        charset: song.charset,
        part,
        branches_skipped: parsed.branches_skipped,
        invalid_data_lines: parsed.invalid_data_lines,
        odd_data_lines: parsed.odd_data_lines,
        ignored_bar_lengths: parsed.ignored_bar_lengths,
        ignored_bpm_changes: song.ignored_bpm_changes,
        hidden_chips: parsed.hidden_chips,
        blank_hit_sounds: parsed.blank_hit_sounds,
        wailing_chips: parsed.wailing_chips,
        fill_in_chips: parsed.fill_in_chips,
        visual_chips: parsed.visual_chips,
        inert_chips: parsed.inert_chips,
        unknown_chips: parsed.unknown_chips,
        unsafe_names: parsed.unsafe_names,
        dtxv_play_speed_ignored: parsed.dtxv_play_speed_ignored,
        path_wav_ignored: parsed.path_wav_ignored,
        ..DtxReport::default()
    }
}

/// The chips one instrument reads: background sounds, the instrument's own chips, its hold markers.
struct PartChips<'a> {
    background: Vec<(Chip, &'a Sound)>,
    hits: Vec<Hit<'a>>,
    markers: Vec<Chip>,
}

impl<'a> PartChips<'a> {
    fn sort(song: &'a Song, part: Part, report: &mut DtxReport) -> Self {
        let mut chips = Self { background: Vec::new(), hits: Vec::new(), markers: Vec::new() };
        for chip in &song.file.chips {
            let hit = |what: u8| Hit { chip: *chip, pos: position(chip), time_us: 0, what, sound: song.sound(chip) };
            match classify(chip.channel) {
                Role::Background | Role::Effect => chips.background.extend(song.sound(chip).map(|sound| (*chip, sound))),
                Role::Drum(drum) if part == Part::Drums => {
                    match drum {
                        Pad::HiHatOpen => report.hi_hat_open_notes += 1,
                        Pad::LeftPedal | Pad::LeftBassDrum => report.left_pedal_notes_folded += 1,
                        _ => {}
                    }
                    chips.hits.push(hit(drum_slot(drum)));
                }
                Role::Fret { part: of, buttons } if of == part => chips.hits.push(hit(buttons)),
                Role::Drum(_) | Role::Fret { .. } => report.other_instrument_chips += 1,
                Role::HoldMarker(of) if of == part => chips.markers.push(*chip),
                _ => {}
            }
        }
        report.background_sounds = chips.background.len();
        chips
    }
}

/// One note per column a hit plays (a hold when `end_us` gives its end), sorted, the overlaps of a column dropped (counted);
/// each kept note gets its chip's sound.
fn add_notes(rox: &mut RoxChart, hits: &[Hit], columns: &Columns, end_us: impl Fn(usize) -> Option<i64>, report: &mut DtxReport) {
    let mut built: Vec<(Note, Option<&Sound>)> = Vec::with_capacity(hits.len());
    for (index, hit) in hits.iter().enumerate() {
        let end = end_us(index);
        for column in columns.of(hit.what) {
            let note = match end {
                Some(end) if end > hit.time_us => Note::hold(hit.time_us, end.saturating_sub(hit.time_us), column),
                _ => Note::tap(hit.time_us, column),
            };
            built.push((note, hit.sound));
        }
    }
    built.sort_by_key(|(note, _)| (note.time_us, !note.is_hold(), note.column));
    let mut last = vec![(i64::MIN, i64::MIN); usize::from(columns.key_count)];
    let mut sounds: HashMap<(&str, u8), u16> = HashMap::new();
    for (mut note, sound) in built {
        let (start, end) = &mut last[usize::from(note.column)];
        if note.time_us < *end || note.time_us == *start {
            report.overlapping_notes_dropped += 1;
            continue;
        }
        (*start, *end) = (note.time_us, note.end_time_us());
        if let Some(sound) = sound {
            let index = *sounds.entry((sound.name.as_str(), sound.volume)).or_insert_with(|| {
                rox.hitsounds.push(if sound.volume == 100 {
                    Hitsound::new(sound.name.as_str())
                } else {
                    Hitsound::with_volume(sound.name.as_str(), sound.volume)
                });
                u16::try_from(rox.hitsounds.len() - 1).unwrap_or(u16::MAX)
            });
            note.hitsound_index = Some(index);
        }
        rox.notes.push(note);
    }
}

/// Every sound of the chart: the background ones, then each hit's.
fn audio_plan(song: &Song, chips: &PartChips, shift_us: i64) -> AudioPlan {
    let event = |time_us: i64, sound: &Sound| {
        let mut event = SoundEvent::whole(time_us, sound.name.as_str());
        event.volume = (sound.volume != 100).then_some(sound.volume);
        event
    };
    let background = chips.background.iter().map(|(chip, sound)| event(to_us(song.seconds(chip)) + shift_us, sound));
    let hits = chips.hits.iter().filter_map(|hit| hit.sound.map(|sound| event(hit.time_us, sound)));
    AudioPlan::new(background.chain(hits).collect())
}

/// The holds of a guitar or bass chart, as `CDTX.cs` pairs them: a marker (channel `2C`/`2D`, its value is not read) is
/// the start of a hold when it sits on a button chip (not an open one), and the next marker is its end, when no chip of the
/// instrument, open ones included, lies after the start and up to that marker (a chip at the very position of the end
/// breaks the hold too). A broken pair is dropped, and the marker that broke it is used up (a marker that is no part of a hold is counted in
/// `hold_markers_dropped`). Returns, by index into `hits`
/// (sorted by position then channel), the beat where the hold of that chip ends.
fn holds(song: &Song, hits: &[Hit], markers: &[Chip]) -> HashMap<usize, f64> {
    let mut ends = HashMap::new();
    let mut markers: Vec<&Chip> = markers.iter().collect();
    markers.sort_by_key(|chip| (position(chip), chip.seq));
    let mut candidate: Option<usize> = None;
    for marker in markers {
        let pos = position(marker);
        match candidate.take() {
            None => {
                let first = hits.partition_point(|hit| hit.pos < pos);
                candidate = hits[first..]
                    .iter()
                    .take_while(|hit| hit.pos == pos)
                    .position(|hit| hit.what != OPEN)
                    .map(|offset| first + offset);
            }
            Some(start) => {
                let start_pos = hits[start].pos;
                let next = hits.partition_point(|hit| hit.pos <= start_pos);
                let broken = hits.get(next).is_some_and(|hit| hit.pos <= pos) || pos <= start_pos;
                if !broken {
                    ends.insert(start, beat_of(&song.beats, marker));
                }
            }
        }
    }
    ends
}

/// Columns of the ROX chart.
struct Columns {
    part: Part,
    /// The column of each slot: a drum slot (`DRUM_LANES`) or a button (`RED`, `GREEN`, `BLUE`, `YELLOW`, `PINK`, `OPEN`: bit index).
    slot: [Option<u8>; 9],
    key_count: u8,
    description: String,
}

impl Columns {
    /// Drums: the lanes the chart uses (after the folds), in the order of `DRUM_LANES`.
    ///
    /// Guitar and bass: the open column, then red, green, blue; yellow and pink too when any chip of the instrument uses one of
    /// them (the XG five-button chart), so a chart has 4 or 6 columns whatever it uses.
    fn new(part: Part, hits: &[Hit]) -> Self {
        let mut slot = [None; 9];
        let mut names: Vec<&str> = Vec::new();
        if part == Part::Drums {
            let mut used = [false; 9];
            for hit in hits {
                used[usize::from(hit.what)] = true;
            }
            for (lane, used) in used.iter().enumerate() {
                if *used {
                    slot[lane] = Some(u8::try_from(names.len()).unwrap_or(0));
                    names.push(DRUM_LANES[lane]);
                }
            }
        } else {
            let five = hits.iter().any(|hit| hit.what & (YELLOW | PINK) != 0);
            let buttons: &[(u8, &str)] = if five {
                &[
                    (OPEN, "Open"),
                    (RED, "R"),
                    (GREEN, "G"),
                    (BLUE, "B"),
                    (YELLOW, "Y"),
                    (PINK, "P"),
                ]
            } else {
                &[(OPEN, "Open"), (RED, "R"), (GREEN, "G"), (BLUE, "B")]
            };
            for (index, (button, name)) in buttons.iter().enumerate() {
                slot[button.trailing_zeros() as usize] = Some(u8::try_from(index).unwrap_or(0));
                names.push(name);
            }
        }
        let key_count = u8::try_from(names.len()).unwrap_or(u8::MAX);
        Self {
            part,
            slot,
            key_count,
            description: format!("{} {} columns: {}", part.name(), key_count, names.join(" ")),
        }
    }

    /// The columns of a chip: its drum lane, or one per button of the chord (the open pick alone).
    fn of(&self, what: u8) -> Vec<u8> {
        if self.part == Part::Drums {
            return self.slot[usize::from(what)].into_iter().collect();
        }
        (0..6)
            .filter(|bit| what & (1 << bit) != 0)
            .filter_map(|bit| self.slot[bit])
            .collect()
    }
}

fn level_of(song: &Song, part: Part) -> Option<f32> {
    let level = song.file.levels[part.slot()];
    let value = level.value.filter(|v| *v > 0 || level.decimal > 0)?;
    #[allow(clippy::cast_precision_loss)] // 0..=100 and 0..=10
    let shown = value as f32 / 10.0
        + if level.decimal == 0 {
            0.0
        } else {
            level.decimal as f32 / 100.0
        };
    Some(shown)
}

/// Title, artist, genre, the level of the instrument and the image the chart names.
fn metadata(song: &Song, part: Part, audio: Option<&Sound>) -> Metadata {
    let header = |name: &str| {
        song.file
            .headers
            .get(name)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
    };
    // The song list of the game shows `#BACKGROUND`, else `#BACKGROUND_GR`; the guitar screen prefers `_GR`.
    let background = if part == Part::Drums {
        header("BACKGROUND").or_else(|| header("BACKGROUND_GR"))
    } else {
        header("BACKGROUND_GR").or_else(|| header("BACKGROUND"))
    };
    Metadata {
        title: header("TITLE").unwrap_or_default().into(),
        artist: header("ARTIST").unwrap_or_default().into(),
        difficulty_name: part.name().into(),
        difficulty_value: level_of(song, part),
        genre: header("GENRE").map(Into::into),
        background_file: background
            .or_else(|| header("STAGEFILE"))
            .or_else(|| header("PREIMAGE"))
            .map(Into::into),
        audio_file: audio
            .map(|sound| sound.name.as_str())
            .unwrap_or_default()
            .into(),
        ..Metadata::default()
    }
}
