#![warn(clippy::pedantic)]
//! `.chart` decoder: one 5-lane track of the file to a five-column ROX chart, plus the audio plan.
//!
//! See `mod.rs` for the decisions. The steps are: the sections of the text (`chart.rs`), `[Song]` and `[SyncTrack]`,
//! the tempo map, then one pass per track that keeps the notes of the five frets and settles the sustains.

use rox::audio_plan::{AudioPlan, SoundEvent};
use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, Note, RoxChart, TimingPoint};

use super::chart::{
    DEFAULT_RESOLUTION, MAX_TEXT_BYTES, MAX_US, Section, SongSection, TempoMap, decode_text,
    parse_song, parse_sync, sections, split_event,
};
use super::ini::SongIni;
use super::plan::{Listing, Streams, stem_events};
use crate::formats::bms::bmson::safe_name;

/// Decoder of `.chart` files: the first playable track of the file (see [`ChartDecoder::variants`] for the others).
pub struct ChartDecoder;
crate::impl_format!(ChartDecoder, ["chart"]);

/// Columns of a decoded chart: the frets 0..=4.
const KEYS: u8 = 5;

/// The 5-lane instruments of a `.chart`, in the order of [`ChartDecoder::variants`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Instrument {
    /// `Single`: lead guitar.
    Guitar,
    /// `DoubleGuitar`: co-op guitar.
    GuitarCoop,
    /// `DoubleBass`: bass.
    Bass,
    /// `DoubleRhythm`: rhythm guitar.
    Rhythm,
    /// `Keyboard`: 5-lane keys.
    Keys,
}

impl Instrument {
    pub const ALL: [Self; 5] = [
        Self::Guitar,
        Self::GuitarCoop,
        Self::Bass,
        Self::Rhythm,
        Self::Keys,
    ];

    /// The end of the section name: `Single`, `DoubleGuitar`, `DoubleBass`, `DoubleRhythm`, `Keyboard`.
    #[must_use]
    pub fn section_suffix(self) -> &'static str {
        match self {
            Self::Guitar => "Single",
            Self::GuitarCoop => "DoubleGuitar",
            Self::Bass => "DoubleBass",
            Self::Rhythm => "DoubleRhythm",
            Self::Keys => "Keyboard",
        }
    }

    /// Name shown in the chart's difficulty name.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Guitar => "Guitar",
            Self::GuitarCoop => "Guitar Co-op",
            Self::Bass => "Bass",
            Self::Rhythm => "Rhythm",
            Self::Keys => "Keys",
        }
    }

    /// The `song.ini` tag that rates the track.
    #[must_use]
    pub fn ini_tag(self) -> &'static str {
        match self {
            Self::Guitar => "diff_guitar",
            Self::GuitarCoop => "diff_guitar_coop",
            Self::Bass => "diff_bass",
            Self::Rhythm => "diff_rhythm",
            Self::Keys => "diff_keys",
        }
    }
}

/// The four difficulties, hardest first (the order of [`ChartDecoder::variants`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Difficulty {
    Expert,
    Hard,
    Medium,
    Easy,
}

impl Difficulty {
    pub const ALL: [Self; 4] = [Self::Expert, Self::Hard, Self::Medium, Self::Easy];

    /// The start of the section name.
    #[must_use]
    pub fn section_prefix(self) -> &'static str {
        match self {
            Self::Expert => "Expert",
            Self::Hard => "Hard",
            Self::Medium => "Medium",
            Self::Easy => "Easy",
        }
    }
}

/// A playable track of a `.chart`. A track without a note on the five frets is not a variant, so `index` counts the
/// playable ones from 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// Number to give to [`ChartDecoder::decode_variant`]: `0..variants.len()`.
    pub index: usize,
    pub instrument: Instrument,
    pub difficulty: Difficulty,
    /// Name of the section, for example `ExpertSingle`.
    pub section: String,
    /// Notes of the decoded chart (a sustain is one).
    pub notes: usize,
    /// Sustains among them.
    pub holds: usize,
    pub key_count: u8,
    pub duration_us: i64,
}

/// Where the audio shift of a chart came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShiftSource {
    /// No shift.
    #[default]
    None,
    /// `delay` of the `song.ini` (milliseconds, takes precedence).
    IniDelay,
    /// `Offset` of `[Song]` (seconds).
    ChartOffset,
}

/// What the decoder chose, skipped or repaired for one track. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChartReport {
    /// Section of the track.
    pub section: String,
    pub resolution: u32,
    /// `[Song]` has no `Resolution`: 192 is used.
    pub resolution_defaulted: bool,
    /// Time added to every tick time (notes and tempos) so that times count from the start of the audio, µs.
    pub shift_us: i64,
    pub shift_source: ShiftSource,
    /// The shift is beyond 10^7 seconds: not applied.
    pub shift_out_of_range: bool,
    /// Strum/HOPO flip modifiers (`N 5`): no timing change, not kept.
    pub forced_flags: usize,
    /// Tap modifiers (`N 6`): no timing change, not kept.
    pub tap_flags: usize,
    /// Open notes (`N 7`): dropped, a chart has five columns.
    pub open_notes_dropped: usize,
    /// `N` events whose type is none of 0..=7: dropped.
    pub unknown_note_types: usize,
    /// A second note on the same tick and fret: dropped, the first stays.
    pub duplicate_notes_dropped: usize,
    /// Sustains shorter than the `sustain_cutoff_threshold` of the `song.ini`: made taps.
    pub sustains_cut_to_taps: usize,
    /// Sustains that reached the next note of their fret: shortened to end where it starts.
    pub sustains_clamped: usize,
    /// Sustains that rounded to no duration (or ended out of range): made taps.
    pub sustains_collapsed_to_taps: usize,
    /// Notes later than 10^7 seconds: dropped.
    pub events_out_of_range: usize,
    /// Track lines that are not `<tick> = ...`.
    pub malformed_lines: usize,
    /// `[SyncTrack]` lines that are not `<tick> = <code> ...`.
    pub malformed_sync_lines: usize,
    /// Tempos of 0 (or beyond `u32`) and time signatures with a numerator of 0 or an exponent above 31: ignored.
    pub ignored_sync_events: usize,
    /// Tempos or signatures replaced by a later one on the same tick.
    pub replaced_sync_events: usize,
    /// The file has no `[SyncTrack]` (or it holds no tempo at tick 0): 120 BPM until the first `B`.
    pub default_tempo_used: bool,
    /// Sections named `<Difficulty><Instrument>` that this decoder does not read (drums, GHL, `SingleBass`, ...).
    pub unsupported_tracks: Vec<String>,
    /// Stream tags or listed names that are absolute or climb out of the folder: refused.
    pub unsafe_names: usize,
    /// `.mid` only: note on events of a key already sounding and note off events with no note on, plus the notes
    /// and phrase markers still open at the end of the track: ignored.
    pub unpaired_midi_events: usize,
    /// `.mid` only: tempo and time signature events met in an instrument track (the tempo map is the first track's
    /// alone, as in the format notes): ignored.
    pub sync_events_off_first_track: usize,
    /// `.mid` only: a second track of an instrument that is already read: ignored.
    pub duplicate_tracks_ignored: usize,
}

/// What a caller knows beside the chart bytes.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChartContext<'a> {
    /// Bytes of the `song.ini` beside the chart.
    pub ini: Option<&'a [u8]>,
    /// Names of the files beside the chart (relative to its folder, `/` separated). With them the audio plan holds
    /// only files that exist; without them it holds every stem a song may have as an optional event.
    pub files: Option<&'a [String]>,
}

/// A track, converted.
pub(super) struct Built {
    pub chart: RoxChart,
    pub report: ChartReport,
    pub instrument: Instrument,
    pub difficulty: Difficulty,
    pub holds: usize,
}

/// Everything a file holds: its `[Song]`, its playable tracks and the sections left out.
pub(super) struct Converted {
    pub song: SongSection,
    pub tracks: Vec<Built>,
}

impl Decoder for ChartDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        nth(convert(data, None)?, 0).map(|built| built.chart)
    }
}

impl ChartDecoder {
    /// The playable tracks of the file: instrument (`Single` first), then difficulty (Expert, Hard, Medium, Easy). A
    /// section without a note on the five frets is left out.
    ///
    /// # Errors
    /// Returns an error if the file has no `[Song]` section or an invalid `Resolution`.
    pub fn variants(data: &[u8]) -> RoxResult<Vec<Variant>> {
        Ok(variants_of(&convert(data, None)?))
    }

    /// Decodes playable track `index` (the numbering of [`Self::variants`]) without a `song.ini`.
    ///
    /// # Errors
    /// Returns an error if the file does not parse or has no such track.
    pub fn decode_variant(data: &[u8], index: usize) -> RoxResult<RoxChart> {
        Self::decode_with_ini(data, None, index)
    }

    /// Decodes track `index` of the `.chart` at `path`, reading the `song.ini` of the same folder when there is one
    /// (found by name without regard to case). A `song.ini` that is not readable is ignored; one that does not parse
    /// (larger than 64 MiB) is an error.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read, does not parse or has no such track.
    pub fn decode_file(path: &std::path::Path, index: usize) -> RoxResult<RoxChart> {
        let data = std::fs::read(path)?;
        let ini = read_sibling_ini(path);
        Self::decode_with_ini(&data, ini.as_deref(), index)
    }

    /// [`Self::decode_variant`] with the bytes of the `song.ini` beside the chart: its metadata takes precedence over
    /// `[Song]`, its `delay` over `Offset`, its `sustain_cutoff_threshold` applies to the sustains.
    ///
    /// # Errors
    /// Returns an error if the file or the `song.ini` does not parse, or has no such track.
    pub fn decode_with_ini(data: &[u8], ini: Option<&[u8]>, index: usize) -> RoxResult<RoxChart> {
        Self::decode_with_report(data, ini, index).map(|(chart, _)| chart)
    }

    /// [`Self::decode_with_ini`] and what the decoder chose, skipped or repaired.
    ///
    /// # Errors
    /// Same as [`Self::decode_with_ini`].
    pub fn decode_with_report(
        data: &[u8],
        ini: Option<&[u8]>,
        index: usize,
    ) -> RoxResult<(RoxChart, ChartReport)> {
        let ini = ini.map(SongIni::parse).transpose()?;
        let built = nth(convert(data, ini.as_ref())?, index)?;
        Ok((built.chart, built.report))
    }

    /// [`Self::decode_with_report`] plus the [`AudioPlan`]: every stem of the song as a whole file at time 0 (times of
    /// a chart count from the start of the audio, so the shift is in the chart). See [`ChartContext`] for what the
    /// plan holds with and without a file listing.
    ///
    /// # Errors
    /// Same as [`Self::decode_with_ini`].
    pub fn decode_with_audio_plan(
        data: &[u8],
        ctx: &ChartContext<'_>,
        index: usize,
    ) -> RoxResult<(RoxChart, ChartReport, AudioPlan)> {
        let ini = ctx.ini.map(SongIni::parse).transpose()?;
        let converted = convert(data, ini.as_ref())?;
        let song = converted.song.clone();
        let mut built = nth(converted, index)?;
        let listing = ctx.files.map(Listing::from_names);
        let events = plan_events(&song, listing.as_ref(), None, &mut built.report);
        Ok((built.chart, built.report, AudioPlan::new(events)))
    }
}

/// The plan of a song (`container` is the `.sng` for entries of a package).
pub(super) fn plan_events(
    song: &SongSection,
    listing: Option<&Listing>,
    container: Option<&str>,
    report: &mut ChartReport,
) -> Vec<SoundEvent> {
    stem_events(
        &Streams {
            tags: &song.streams,
        },
        listing,
        container,
        &mut report.unsafe_names,
    )
}

pub(super) fn variants_of(converted: &Converted) -> Vec<Variant> {
    converted
        .tracks
        .iter()
        .enumerate()
        .map(|(index, built)| Variant {
            index,
            instrument: built.instrument,
            difficulty: built.difficulty,
            section: built.report.section.clone(),
            notes: built.chart.notes.len(),
            holds: built.holds,
            key_count: KEYS,
            duration_us: built.chart.duration_us(),
        })
        .collect()
}

pub(super) fn nth(converted: Converted, index: usize) -> RoxResult<Built> {
    let count = converted.tracks.len();
    converted.tracks.into_iter().nth(index).ok_or_else(|| {
        RoxError::InvalidFormat(format!(
            "chart: {count} playable track(s), not track {index}"
        ))
    })
}

/// Parses a `.chart` and converts every 5-lane track that has a note on the five frets.
pub(super) fn convert(data: &[u8], ini: Option<&SongIni>) -> RoxResult<Converted> {
    if data.len() > MAX_TEXT_BYTES {
        return Err(RoxError::InvalidFormat(format!("chart: {} bytes, more than {MAX_TEXT_BYTES}", data.len())));
    }
    let text = decode_text(data);
    let sections = sections(&text);
    let song_section =
        sections.iter().find(|s| s.name == "Song").ok_or_else(|| RoxError::InvalidFormat("chart: no [Song] section".into()))?;
    let song = parse_song(&song_section.lines).map_err(RoxError::InvalidFormat)?;
    let resolution = song.resolution.unwrap_or(DEFAULT_RESOLUTION);
    let sync = sections.iter().find(|s| s.name == "SyncTrack").map(|s| parse_sync(&s.lines)).unwrap_or_default();
    let tempo = TempoMap::new(resolution, &sync.tempos);
    let mut shared = ChartReport {
        resolution,
        resolution_defaulted: song.resolution.is_none(),
        malformed_sync_lines: sync.malformed,
        ignored_sync_events: sync.ignored,
        replaced_sync_events: sync.replaced,
        default_tempo_used: sync.tempos.first().is_none_or(|first| first.0 > 0),
        unsupported_tracks: sections
            .iter()
            .filter(|s| is_track_name(s.name) && !is_supported_name(s.name))
            .map(|s| s.name.to_owned())
            .collect(),
        ..ChartReport::default()
    };
    let (shift_us, source) = audio_shift(&song, ini, &mut shared);
    shared.shift_us = shift_us;
    shared.shift_source = source;
    let cutoff = ini.and_then(|i| i.sustain_cutoff_threshold).and_then(|t| u64::try_from(t).ok()).unwrap_or(0);
    let timing = timing_points(&tempo, &sync.signatures, shift_us);

    let mut tracks = Vec::new();
    for instrument in Instrument::ALL {
        for difficulty in Difficulty::ALL {
            let name = format!("{}{}", difficulty.section_prefix(), instrument.section_suffix());
            let Some(section) = sections.iter().find(|s| s.name == name) else { continue };
            let mut report = shared.clone();
            report.section = name;
            let notes = track_notes(section, &tempo, shift_us, cutoff, &mut report);
            let chart = || metadata(&song, ini, instrument, difficulty);
            tracks.extend(built(notes, chart, &timing, report, instrument, difficulty));
        }
    }
    Ok(Converted { song, tracks })
}

/// The chart of one track and difficulty; `None` when it has no note. Shared by the `.chart` and the `.mid` decoders.
pub(super) fn built(
    notes: Vec<Note>,
    metadata: impl FnOnce() -> Metadata,
    timing: &[TimingPoint],
    report: ChartReport,
    instrument: Instrument,
    difficulty: Difficulty,
) -> Option<Built> {
    if notes.is_empty() {
        return None;
    }
    let holds = notes.iter().filter(|n| n.is_hold()).count();
    let mut chart = RoxChart::new(KEYS);
    chart.metadata = metadata();
    chart.timing_points = timing.to_vec();
    chart.notes = notes;
    Some(Built { chart, report, instrument, difficulty, holds })
}

/// `<Difficulty><Something>`.
fn is_track_name(name: &str) -> bool {
    Difficulty::ALL.iter().any(|d| {
        name.strip_prefix(d.section_prefix())
            .is_some_and(|rest| !rest.is_empty())
    })
}

/// `<Difficulty><5-lane instrument>`: a track this decoder reads (a variant when it has notes).
fn is_supported_name(name: &str) -> bool {
    Difficulty::ALL.iter().any(|d| {
        name.strip_prefix(d.section_prefix())
            .is_some_and(|rest| Instrument::ALL.iter().any(|i| i.section_suffix() == rest))
    })
}

/// The shift that makes tick times count from the start of the audio, µs.
///
/// `delay` of the `song.ini` (ms) when present and not 0; else `Offset` of `[Song]` (seconds). Both have the same
/// sign: positive = the audio starts before the chart, so the notes sound later in the audio.
pub(super) fn audio_shift(
    song: &SongSection,
    ini: Option<&SongIni>,
    report: &mut ChartReport,
) -> (i64, ShiftSource) {
    let max_ms = 1.0e10;
    if let Some(delay) = ini.and_then(|i| i.delay_ms).filter(|&d| d != 0) {
        #[allow(clippy::cast_precision_loss)] // a delay in ms is far below 2^53
        if (delay as f64).abs() > max_ms {
            report.shift_out_of_range = true;
            return (0, ShiftSource::None);
        }
        return (delay * 1000, ShiftSource::IniDelay);
    }
    if let Some(offset) = song.offset_s.filter(|&o| o != 0.0) {
        let us = (offset * 1.0e6).round();
        if us.abs() > MAX_US {
            report.shift_out_of_range = true;
            return (0, ShiftSource::None);
        }
        #[allow(clippy::cast_possible_truncation)] // bounded just above
        return (us as i64, ShiftSource::ChartOffset);
    }
    (0, ShiftSource::None)
}

/// A BPM point per distinct tempo, signature = the last `TS` at or before the tempo's tick.
fn timing_points(tempo: &TempoMap, signatures: &[(u64, u8)], shift_us: i64) -> Vec<TimingPoint> {
    let mut out = Vec::with_capacity(tempo.segments.len());
    let mut last_milli = 0;
    for seg in &tempo.segments {
        if seg.milli == last_milli {
            continue;
        }
        last_milli = seg.milli;
        let at = signatures.partition_point(|s| s.0 <= seg.tick);
        let signature = if at == 0 { 4 } else { signatures[at - 1].1 };
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        // a tempo fits f32 closely enough
        out.push(TimingPoint::Bpm {
            time_us: seg.start_us.round() as i64 + shift_us,
            bpm: (seg.milli as f64 / 1000.0) as f32,
            signature,
        });
    }
    out
}

/// The notes of a track section on the five frets, sustains settled, sorted by time.
fn track_notes(
    section: &Section<'_>,
    tempo: &TempoMap,
    shift_us: i64,
    cutoff: u64,
    report: &mut ChartReport,
) -> Vec<Note> {
    // `N` events: tick, fret, length.
    let mut events: Vec<(u64, u8, u64)> = Vec::new();
    for line in &section.lines {
        let Some((tick, rest)) = split_event(line) else {
            report.malformed_lines += 1;
            continue;
        };
        let mut words = rest.split_whitespace();
        if words.next() != Some("N") {
            continue;
        }
        let Some(kind) = words.next().and_then(|w| w.parse::<u32>().ok()) else {
            report.malformed_lines += 1;
            continue;
        };
        let length = words.next().map_or(Some(0), |w| w.parse::<u64>().ok());
        let Some(length) = length else {
            report.malformed_lines += 1;
            continue;
        };
        match kind {
            0..=4 => events.push((tick, u8::try_from(kind).unwrap_or(0), length)),
            5 => report.forced_flags += 1,
            6 => report.tap_flags += 1,
            7 => report.open_notes_dropped += 1,
            _ => report.unknown_note_types += 1,
        }
    }
    settle_notes(events, cutoff, |tick| tempo.us(tick), shift_us, report)
}

/// Notes from `(tick, fret, length in ticks)` events: sorted, one per tick and fret (the first stays), a sustain shorter
/// than `cutoff` a tap, a sustain never past the next note of its fret, ticks to times with `us` (µs, unrounded) plus
/// `shift_us`. Shared by the `.chart` and the `.mid` decoders.
pub(super) fn settle_notes(
    mut events: Vec<(u64, u8, u64)>,
    cutoff: u64,
    us: impl Fn(u64) -> f64,
    shift_us: i64,
    report: &mut ChartReport,
) -> Vec<Note> {
    events.sort_by_key(|e| (e.0, e.1)); // stable: the first of a tick and fret stays first
    let before = events.len();
    events.dedup_by(|later, first| later.0 == first.0 && later.1 == first.1);
    report.duplicate_notes_dropped += before - events.len();

    // Sustains: shorter than the cutoff = tap, never past the next note of their fret.
    let mut next_tick: [Option<u64>; KEYS as usize] = [None; KEYS as usize];
    for event in events.iter_mut().rev() {
        let column = usize::from(event.1);
        if event.2 > 0 && event.2 < cutoff {
            event.2 = 0;
            report.sustains_cut_to_taps += 1;
        }
        if let Some(next) = next_tick[column]
            && event.2 > next - event.0
        {
            event.2 = next - event.0;
            report.sustains_clamped += 1;
        }
        next_tick[column] = Some(event.0);
    }

    let mut notes = Vec::with_capacity(events.len());
    for (tick, column, length) in events {
        let start = us(tick);
        if start > MAX_US {
            report.events_out_of_range += 1;
            continue;
        }
        #[allow(clippy::cast_possible_truncation)] // bounded by MAX_US
        let time_us = start.round() as i64 + shift_us;
        if length == 0 {
            notes.push(Note::tap(time_us, column));
            continue;
        }
        let end = us(tick + length);
        #[allow(clippy::cast_possible_truncation)] // checked against MAX_US first
        let duration = if end > MAX_US {
            0
        } else {
            end.round() as i64 + shift_us - time_us
        };
        if duration > 0 {
            notes.push(Note::hold(time_us, duration, column));
        } else {
            report.sustains_collapsed_to_taps += 1;
            notes.push(Note::tap(time_us, column));
        }
    }
    notes
}

pub(super) fn metadata(
    song: &SongSection,
    ini: Option<&SongIni>,
    instrument: Instrument,
    difficulty: Difficulty,
) -> Metadata {
    let pick = |from_ini: Option<&String>, from_chart: &Option<String>| -> String {
        from_ini
            .or(from_chart.as_ref())
            .cloned()
            .unwrap_or_default()
    };
    let mut meta = Metadata {
        title: pick(ini.and_then(|i| i.name.as_ref()), &song.name).into(),
        artist: pick(ini.and_then(|i| i.artist.as_ref()), &song.artist).into(),
        creator: pick(ini.and_then(|i| i.charter.as_ref()), &song.charter).into(),
        difficulty_name: format!("{} {}", instrument.label(), difficulty.section_prefix()).into(),
        // The stems are in the audio plan: the chart has no audio file. Times count from the start of the audio,
        // the shift is in the notes, so there is nothing left for a consumer to apply.
        audio_file: "".into(),
        audio_offset_us: 0,
        difficulty_value: ini
            .and_then(|i| i.difficulty(instrument.ini_tag()))
            .map(|d| {
                #[allow(clippy::cast_precision_loss)] // a rating
                let value = d as f32;
                value
            }),
        ..Metadata::default()
    };
    let genre = ini
        .and_then(|i| i.genre.clone())
        .or_else(|| song.genre.clone());
    meta.genre = genre.map(Into::into);
    meta.background_file = ini
        .and_then(|i| i.background.as_deref())
        .and_then(|b| safe_name(b, &mut 0))
        .map(Into::into);
    let start_us = ini
        .and_then(|i| i.preview_start_time_ms)
        .map(|ms| ms.saturating_mul(1000))
        .or_else(|| seconds_to_us(song.preview_start_s));
    let end_us = ini
        .and_then(|i| i.preview_end_time_ms)
        .map(|ms| ms.saturating_mul(1000))
        .or_else(|| seconds_to_us(song.preview_end_s));
    if let Some(start) = start_us.filter(|&s| s >= 0) {
        meta.preview_time_us = start;
        if let Some(end) = end_us.filter(|&e| e > start) {
            meta.preview_duration_us = end - start;
        }
    }
    meta
}

#[allow(clippy::cast_possible_truncation)] // a preview time in seconds is small; `as` saturates anyway
fn seconds_to_us(seconds: Option<f64>) -> Option<i64> {
    seconds.map(|s| (s * 1.0e6).round() as i64)
}

/// Bytes of the `song.ini` in the folder of `chart_path`: the file named exactly `song.ini`, else any file of that
/// folder named so without regard to case. `None` when there is none or it cannot be read.
#[must_use]
pub fn read_sibling_ini(chart_path: &std::path::Path) -> Option<Vec<u8>> {
    let dir = match chart_path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => std::path::Path::new("."),
    };
    let exact = dir.join("song.ini");
    if exact.is_file() {
        return std::fs::read(exact).ok();
    }
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("song.ini")
        })
        .and_then(|e| std::fs::read(e.path()).ok())
}
