#![warn(clippy::pedantic)]
//! `.mid` decoder: one 5-lane instrument track at one difficulty of a Clone Hero / Rock Band MIDI chart to a
//! five-column ROX chart, plus the audio plan.
//!
//! See `mod.rs` for the decisions. The steps: the file is cut into chunks (`smf.rs`), the FIRST chunk gives the tempo
//! map and the time signatures, every other chunk is named by its track-name event and the 5-lane ones are read in one
//! pass (notes paired on/off, forcing and tap markers counted, open notes found), then each difficulty goes through the
//! same sustain rules as a `.chart` (`settle_notes`).

use rox::audio_plan::AudioPlan;
use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{RoxChart, TimingPoint};

use super::chart::{SongSection, last_of_tick, quarter_beats};
use super::decoder::{
    Built, ChartContext, ChartReport, Converted, Difficulty, Instrument, Variant, audio_shift, built, metadata, nth,
    plan_events, settle_notes, variants_of,
};
use super::ini::SongIni;
use super::plan::Listing;
use super::smf::{self, Event, Events, Smf};

/// Decoder of Clone Hero `.mid` files: the first playable track of the file (see [`MidDecoder::variants`]).
pub struct MidDecoder;
crate::impl_format!(MidDecoder, ["mid"]);

/// Notes and markers kept per instrument track before the track is refused (a real track holds a few thousand).
const MAX_KEPT: usize = 1_000_000;
/// Tempo and time-signature events of the first track kept before the file is refused.
const MAX_SYNC_EVENTS: usize = 1_000_000;
/// Tempo before the first tempo event: 120 BPM, in µs per quarter note (the MIDI default).
const DEFAULT_TEMPO: u32 = 500_000;
/// First note of each difficulty (green), in the order of [`Difficulty::ALL`]: Expert, Hard, Medium, Easy.
const BASE: [u8; 4] = [96, 84, 72, 60];
/// The note-based tap marker (all difficulties).
const TAP_MARKER: u8 = 104;
/// `PART GUITAR` is the one track a later track of the same name replaces (the reader of `YARG.Core` does this
/// for the guitar alone); for every other name the first track stays.
const GUITAR_NAME: &[u8] = b"PART GUITAR";

/// Whether a file NAME is the name of a Clone Hero MIDI chart: `notes.mid` in any case. `.mid` is the extension of every
/// MIDI file, so no other name is taken for a chart (the readers of the format only look for `notes.mid`).
#[must_use]
pub fn is_mid_chart_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("notes.mid")
}

/// Whether a `.mid` file is read as Clone Hero's `notes.mid` or as a Guitar Hero 1/2 chart (`<song>.mid`): the latter also reads
/// Guitar Hero 1's lead track, `T1 GEMS`, as the guitar.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Flavour {
    CloneHero,
    GuitarHero,
}

/// Files above this size are not taken for a Guitar Hero chart (a real one is a few hundred kilobytes).
const MAX_GUITAR_HERO_FILE: u64 = 16 * 1024 * 1024;

/// Whether a MIDI file's bytes are a Guitar Hero 1/2 chart: a Standard MIDI File with a 5-lane instrument track (`T1 GEMS`,
/// `PART GUITAR`, `PART GUITAR COOP`, `PART BASS`, `PART RHYTHM`, `PART KEYS`). Any other MIDI file (a song, a ringtone) is not one.
#[must_use]
pub fn looks_like_guitar_hero_mid(data: &[u8]) -> bool {
    smf::parse(data).is_ok_and(|smf| smf.tracks.iter().any(|track| track_name(track, Flavour::GuitarHero).is_some_and(|name| instrument_of(name, Flavour::GuitarHero).is_some())))
}

/// Whether the file at `path` is a Guitar Hero 1/2 chart: a `.mid` not named `notes.mid` (that one is Clone Hero's) that
/// [`looks_like_guitar_hero_mid`]. The file is read.
#[must_use]
pub fn is_guitar_hero_mid_file(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("mid"))
        && !path.file_name().and_then(|name| name.to_str()).is_some_and(is_mid_chart_name)
        && std::fs::metadata(path).is_ok_and(|file| file.len() <= MAX_GUITAR_HERO_FILE)
        && std::fs::read(path).is_ok_and(|data| looks_like_guitar_hero_mid(&data))
}

impl Decoder for MidDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        nth(convert(data, None, Flavour::CloneHero)?, 0).map(|built| built.chart)
    }
}

impl MidDecoder {
    /// The playable tracks of the file: instrument (`PART GUITAR` first), then difficulty (Expert, Hard, Medium, Easy).
    /// A track or difficulty without a note on the five frets is left out.
    ///
    /// # Errors
    /// Returns an error if the file is not a readable Standard MIDI File (format 0 or 1, ticks per quarter note) or one
    /// of its instrument tracks is cut short or malformed.
    pub fn variants(data: &[u8]) -> RoxResult<Vec<Variant>> {
        Ok(variants_of(&convert(data, None, Flavour::CloneHero)?))
    }

    /// The playable tracks of a Guitar Hero 1/2 chart (`T1 GEMS` read as the guitar), numbered as [`Self::variants`] numbers them.
    ///
    /// # Errors
    /// Same as [`Self::variants`].
    pub fn guitar_hero_variants(data: &[u8]) -> RoxResult<Vec<Variant>> {
        Ok(variants_of(&convert(data, None, Flavour::GuitarHero)?))
    }

    /// Decodes playable track `index` of a Guitar Hero 1/2 chart (numbered by [`Self::guitar_hero_variants`]). The file names no
    /// song: `title` is the caller's (the file stem); nothing else of the metadata is known.
    ///
    /// # Errors
    /// Returns an error if the file does not parse or has no such track.
    pub fn decode_guitar_hero(data: &[u8], index: usize, title: &str) -> RoxResult<RoxChart> {
        let mut chart = nth(convert(data, None, Flavour::GuitarHero)?, index)?.chart;
        chart.metadata.title = title.into();
        Ok(chart)
    }

    /// Decodes playable track `index` (the numbering of [`Self::variants`]) without a `song.ini`.
    ///
    /// # Errors
    /// Returns an error if the file does not parse or has no such track.
    pub fn decode_variant(data: &[u8], index: usize) -> RoxResult<RoxChart> {
        Self::decode_with_ini(data, None, index)
    }

    /// Decodes track `index` of the `.mid` at `path`, reading the `song.ini` of the same folder when there is one (found
    /// by name without regard to case). A `song.ini` that is not readable is ignored; one that does not parse (larger
    /// than 64 MiB) is an error.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read, does not parse or has no such track.
    pub fn decode_file(path: &std::path::Path, index: usize) -> RoxResult<RoxChart> {
        let data = std::fs::read(path)?;
        let ini = super::decoder::read_sibling_ini(path);
        Self::decode_with_ini(&data, ini.as_deref(), index)
    }

    /// [`Self::decode_variant`] with the bytes of the `song.ini` beside the chart: it gives the metadata (a `.mid` has
    /// none), the `delay`, and the `sustain_cutoff_threshold` that replaces the default (a third of the resolution).
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
        let built = nth(convert(data, ini.as_ref(), Flavour::CloneHero)?, index)?;
        Ok((built.chart, built.report))
    }

    /// [`Self::decode_with_report`] plus the [`AudioPlan`]: every stem of the song as a whole file at time 0, as for a
    /// `.chart` (a `.mid` names no stream, so the plan has the reserved names only). See [`ChartContext`].
    ///
    /// # Errors
    /// Same as [`Self::decode_with_ini`].
    pub fn decode_with_audio_plan(
        data: &[u8],
        ctx: &ChartContext<'_>,
        index: usize,
    ) -> RoxResult<(RoxChart, ChartReport, AudioPlan)> {
        let ini = ctx.ini.map(SongIni::parse).transpose()?;
        let converted = convert(data, ini.as_ref(), Flavour::CloneHero)?;
        let song = converted.song.clone();
        let mut built = nth(converted, index)?;
        let listing = ctx.files.map(Listing::from_names);
        let events = plan_events(&song, listing.as_ref(), None, &mut built.report);
        Ok((built.chart, built.report, AudioPlan::new(events)))
    }
}

fn invalid(message: &str) -> RoxError {
    RoxError::InvalidFormat(format!("mid: {message}"))
}

/// The tempo map: µs per quarter note from each tempo event; time = sum of ticks x tempo / ticks per quarter note.
#[derive(Debug)]
struct MidiTempo {
    /// `(tick, µs per quarter note, µs of the chart at tick (unrounded))`.
    segments: Vec<(u64, u32, f64)>,
    ticks_per_quarter: f64,
}

impl MidiTempo {
    /// `tempos` sorted, one per tick. A file with no tempo at tick 0 starts at 120 BPM.
    #[allow(clippy::cast_precision_loss)] // ticks of a file stay far below 2^53
    fn new(ticks_per_quarter: u32, tempos: &[(u64, u32)]) -> Self {
        let tpq = f64::from(ticks_per_quarter);
        let mut pairs: Vec<(u64, u32)> = Vec::with_capacity(tempos.len() + 1);
        if tempos.first().is_none_or(|first| first.0 > 0) {
            pairs.push((0, DEFAULT_TEMPO));
        }
        pairs.extend_from_slice(tempos);
        let mut segments: Vec<(u64, u32, f64)> = Vec::with_capacity(pairs.len());
        for (tick, tempo) in pairs {
            let start = segments
                .last()
                .map_or(0.0, |&(prev_tick, prev_tempo, prev_us)| {
                    prev_us + (tick - prev_tick) as f64 * f64::from(prev_tempo) / tpq
                });
            segments.push((tick, tempo, start));
        }
        Self {
            segments,
            ticks_per_quarter: tpq,
        }
    }

    /// Time of `tick` in µs, unrounded.
    #[allow(clippy::cast_precision_loss)] // see `new`
    fn us(&self, tick: u64) -> f64 {
        let at = self.segments.partition_point(|s| s.0 <= tick) - 1;
        let (seg_tick, tempo, start) = self.segments[at];
        start + (tick - seg_tick) as f64 * f64::from(tempo) / self.ticks_per_quarter
    }
}

/// What the first track holds.
#[derive(Default)]
struct Sync {
    tempos: Vec<(u64, u32)>,
    signatures: Vec<(u64, u8)>,
    ignored: usize,
    replaced: usize,
}

fn read_sync(track: &[u8]) -> RoxResult<Sync> {
    let mut sync = Sync::default();
    for item in Events::new(track) {
        let (tick, event) = item.map_err(|m| invalid(&format!("tempo track: {m}")))?;
        match event {
            Event::Tempo(0) | Event::MalformedSync => sync.ignored += 1,
            Event::Tempo(us) => sync.tempos.push((tick, us)),
            Event::TimeSignature {
                numerator,
                exponent,
            } => {
                if numerator > 0 && exponent <= 31 {
                    sync.signatures.push((
                        tick,
                        quarter_beats(u32::from(numerator), u32::from(exponent)),
                    ));
                } else {
                    sync.ignored += 1;
                }
            }
            _ => {}
        }
        if sync.tempos.len() + sync.signatures.len() > MAX_SYNC_EVENTS {
            return Err(invalid("too many tempo or time signature events"));
        }
    }
    sync.replaced = last_of_tick(&mut sync.tempos) + last_of_tick(&mut sync.signatures);
    Ok(sync)
}

/// A BPM point per distinct tempo, signature = the last time signature at or before the tempo's tick.
fn timing_points(tempo: &MidiTempo, signatures: &[(u64, u8)], shift_us: i64) -> Vec<TimingPoint> {
    let mut out = Vec::with_capacity(tempo.segments.len());
    let mut last = 0;
    for &(tick, us_per_quarter, start_us) in &tempo.segments {
        if us_per_quarter == last {
            continue;
        }
        last = us_per_quarter;
        let at = signatures.partition_point(|s| s.0 <= tick);
        let signature = if at == 0 { 4 } else { signatures[at - 1].1 };
        #[allow(clippy::cast_possible_truncation)] // a tempo fits f32 closely enough
        out.push(TimingPoint::Bpm {
            time_us: start_us.round() as i64 + shift_us,
            bpm: (60.0e6 / f64::from(us_per_quarter)) as f32,
            signature,
        });
    }
    out
}

/// The 5-lane instrument of a track name, exactly as written.
fn instrument_of(name: &[u8], flavour: Flavour) -> Option<Instrument> {
    Some(match name {
        b"PART GUITAR" => Instrument::Guitar,
        b"T1 GEMS" if flavour == Flavour::GuitarHero => Instrument::Guitar,
        b"PART GUITAR COOP" => Instrument::GuitarCoop,
        b"PART BASS" => Instrument::Bass,
        b"PART RHYTHM" => Instrument::Rhythm,
        b"PART KEYS" => Instrument::Keys,
        _ => return None,
    })
}

/// The tracks that look like instruments but are not read: every `PART ...` name that is not one of the five (drums,
/// vocals, GHL, pro tracks, `PART DANCE`, ...), the harmony tracks and, in a `notes.mid`, the Guitar Hero 1 lead (`T1 GEMS`).
fn looks_like_instrument(name: &[u8]) -> bool {
    name.starts_with(b"PART ") || name.starts_with(b"HARM") || name == b"T1 GEMS"
}

/// The name of a track: the track-name events at tick 0; the first one that names a 5-lane instrument, else the first.
fn track_name(track: &[u8], flavour: Flavour) -> Option<&[u8]> {
    let mut first: Option<&[u8]> = None;
    for item in Events::new(track) {
        let Ok((tick, event)) = item else { break };
        if tick != 0 {
            break;
        }
        if let Event::TrackName(name) = event {
            if instrument_of(name, flavour).is_some() {
                return Some(name);
            }
            first.get_or_insert(name);
        }
    }
    first
}

/// What one difficulty of an instrument track holds.
#[derive(Default)]
struct DiffNotes {
    /// `(tick, fret, length in ticks)` of the notes on the five frets.
    frets: Vec<(u64, u8, u64)>,
    /// Force HOPO / strum markers (one per marker).
    forced: usize,
    /// Tap markers: the note-based marker (104) and the tap phrases (`SysEx`) that apply to this difficulty.
    taps: usize,
    /// Open notes by note (`[ENHANCED_OPENS]`), one per note.
    opens: usize,
    /// Open phrases (SysEx): `(first tick, last tick)` of the notes they turn open.
    open_ranges: Vec<(u64, u64)>,
}

/// An instrument track, read.
struct RawTrack {
    /// In the order of [`Difficulty::ALL`].
    diffs: [DiffNotes; 4],
    unpaired: usize,
    off_sync: usize,
}

/// What a MIDI note number means in a 5-lane track.
enum Key {
    /// Difficulty index, fret 0..=4.
    Fret(usize, u8),
    /// Difficulty index: the open note of the difficulty (only with `[ENHANCED_OPENS]`).
    Open(usize),
    /// Difficulty index: force HOPO / force strum.
    Force(usize),
    Tap,
}

fn classify(key: u8) -> Option<Key> {
    if key == TAP_MARKER {
        return Some(Key::Tap);
    }
    for (d, base) in BASE.into_iter().enumerate() {
        if key + 1 == base {
            return Some(Key::Open(d));
        }
        if key >= base && key - base <= 4 {
            return Some(Key::Fret(d, key - base));
        }
        if key == base + 5 || key == base + 6 {
            return Some(Key::Force(d));
        }
    }
    None
}

/// `text` as the reader of `YARG.Core` normalizes it: what is between the first `[` and the first `]` (when they are
/// in that order), else the whole text; trimmed.
fn normalized(text: &[u8]) -> String {
    let text = String::from_utf8_lossy(text);
    if let (Some(open), Some(close)) = (text.find('['), text.find(']'))
        && open <= close
    {
        return text[open + 1..close].trim().to_owned();
    }
    text.trim().to_owned()
}

/// The `SysEx` modifier events the decoder reads: `PS \0 <type=0> <difficulty> <modifier> <value> F7`.
const PS_OPEN: u8 = 0x01;
const PS_TAP: u8 = 0x04;

fn read_instrument(track: &[u8]) -> RoxResult<RawTrack> {
    let mut reader = TrackReader {
        raw: RawTrack { diffs: Default::default(), unpaired: 0, off_sync: 0 },
        sounding: vec![None; 16 * 128],
        phrases: Vec::new(),
        enhanced_opens: false,
        kept: 0,
    };
    for item in Events::new(track) {
        let (tick, event) = item.map_err(|m| invalid(&format!("instrument track: {m}")))?;
        match event {
            Event::NoteOn { channel, key } => reader.note_on(channel, key, tick),
            Event::NoteOff { channel, key } => reader.note_off(channel, key, tick)?,
            Event::Text(text) => reader.enhanced_opens |= normalized(text) == "ENHANCED_OPENS",
            Event::SysEx(data) => reader.sysex(data, tick)?,
            Event::Tempo(_) | Event::TimeSignature { .. } | Event::MalformedSync => reader.raw.off_sync += 1,
            _ => {}
        }
    }
    let mut raw = reader.raw;
    raw.unpaired += reader.sounding.iter().flatten().count() + reader.phrases.len();
    Ok(raw)
}

/// The events of an instrument track read so far.
struct TrackReader {
    raw: RawTrack,
    /// Start tick of the sounding notes, by channel and key.
    sounding: Vec<Option<u64>>,
    /// Phrases between their start and end `SysEx`: (difficulty byte, modifier, start tick).
    phrases: Vec<(u8, u8, u64)>,
    enhanced_opens: bool,
    kept: usize,
}

impl TrackReader {
    fn keep(&mut self) -> RoxResult<()> {
        self.kept += 1;
        if self.kept > MAX_KEPT {
            return Err(invalid("too many notes in an instrument track"));
        }
        Ok(())
    }

    fn note_on(&mut self, channel: u8, key: u8, tick: u64) {
        if classify(key).is_none() {
            return;
        }
        let slot = &mut self.sounding[usize::from(channel) * 128 + usize::from(key)];
        if slot.is_some() {
            self.raw.unpaired += 1;
        } else {
            *slot = Some(tick);
        }
    }

    fn note_off(&mut self, channel: u8, key: u8, tick: u64) -> RoxResult<()> {
        let Some(kind) = classify(key) else { return Ok(()) };
        let Some(start) = self.sounding[usize::from(channel) * 128 + usize::from(key)].take() else {
            self.raw.unpaired += 1;
            return Ok(());
        };
        self.keep()?;
        let diffs = &mut self.raw.diffs;
        match kind {
            Key::Fret(d, fret) => diffs[d].frets.push((start, fret, tick - start)),
            Key::Open(d) if self.enhanced_opens => diffs[d].opens += 1,
            Key::Open(_) => {}
            Key::Force(d) => diffs[d].forced += 1,
            Key::Tap => diffs.iter_mut().for_each(|diff| diff.taps += 1),
        }
        Ok(())
    }

    /// A Phase Shift modifier: the start (`value` 1) or end (0) of an open or tap phrase.
    fn sysex(&mut self, data: &[u8], tick: u64) -> RoxResult<()> {
        let &[b'P', b'S', 0, 0, difficulty, modifier, value, _] = data else { return Ok(()) };
        if modifier != PS_OPEN && modifier != PS_TAP || difficulty > 3 && difficulty != 0xFF {
            return Ok(());
        }
        let open = self.phrases.iter().position(|p| p.0 == difficulty && p.1 == modifier);
        match (value, open) {
            (1, Some(_)) | (0, None) => self.raw.unpaired += 1,
            (1, None) => {
                self.keep()?;
                self.phrases.push((difficulty, modifier, tick));
            }
            (0, Some(at)) => {
                let (_, _, start) = self.phrases.swap_remove(at);
                self.close_phrase(difficulty, modifier, start, tick);
            }
            _ => {}
        }
        Ok(())
    }

    fn close_phrase(&mut self, difficulty: u8, modifier: u8, start: u64, tick: u64) {
        // The last tick of a phrase is not part of it (a phrase of no length holds its start tick).
        let last = if tick > start { tick - 1 } else { start };
        for (d, diff) in self.raw.diffs.iter_mut().enumerate() {
            // Difficulty byte 0 easy .. 3 expert; the index counts expert first.
            if difficulty != 0xFF && usize::from(3 - difficulty) != d {
                continue;
            }
            if modifier == PS_OPEN {
                diff.open_ranges.push((start, last));
            } else {
                diff.taps += 1;
            }
        }
    }
}

/// Removes the notes inside the open phrases; returns how many.
fn drop_open_phrase_notes(frets: &mut Vec<(u64, u8, u64)>, ranges: &mut [(u64, u64)]) -> usize {
    if ranges.is_empty() {
        return 0;
    }
    ranges.sort_unstable();
    // Merge overlapping ranges so that each note is tested against one.
    let mut merged: Vec<(u64, u64)> = Vec::with_capacity(ranges.len());
    for &(start, last) in ranges.iter() {
        match merged.last_mut() {
            Some(prev) if start <= prev.1 => prev.1 = prev.1.max(last),
            _ => merged.push((start, last)),
        }
    }
    let before = frets.len();
    frets.retain(|&(tick, _, _)| {
        let at = merged.partition_point(|r| r.1 < tick);
        merged.get(at).is_none_or(|r| r.0 > tick)
    });
    before - frets.len()
}

/// Parses a `.mid` and converts every 5-lane track and difficulty that has a note on the five frets.
pub(super) fn convert(data: &[u8], ini: Option<&SongIni>, flavour: Flavour) -> RoxResult<Converted> {
    let Smf { format, ticks_per_quarter, tracks } = smf::parse(data).map_err(|m| invalid(&m))?;
    let Some(&sync_track) = tracks.first() else {
        return Err(invalid("no track"));
    };
    let sync = read_sync(sync_track)?;
    let tempo = MidiTempo::new(ticks_per_quarter, &sync.tempos);
    let mut shared = ChartReport {
        resolution: ticks_per_quarter,
        malformed_sync_lines: 0,
        ignored_sync_events: sync.ignored,
        replaced_sync_events: sync.replaced,
        default_tempo_used: sync.tempos.first().is_none_or(|first| first.0 > 0),
        ..ChartReport::default()
    };
    // A `.mid` has no `[Song]` `Offset`: only the `delay` of the `song.ini` shifts the times.
    let song = SongSection::default();
    let (shift_us, source) = audio_shift(&song, ini, &mut shared);
    shared.shift_us = shift_us;
    shared.shift_source = source;

    // In a type 1 file the first track is the tempo track alone; a type 0 file has no other.
    let picked = pick_tracks(&tracks[usize::from(format == 1)..], flavour)?;
    shared.duplicate_tracks_ignored = picked.duplicates;
    shared.unsupported_tracks = picked.unsupported;
    let song_charts = SongCharts {
        song: &song,
        ini,
        tempo: &tempo,
        timing: timing_points(&tempo, &sync.signatures, shift_us),
        cutoff: sustain_cutoff(ini, ticks_per_quarter),
        shift_us,
    };
    let mut built = Vec::new();
    for (instrument, slot) in Instrument::ALL.into_iter().zip(picked.slots) {
        let Some((name, mut raw)) = slot else { continue };
        let name = String::from_utf8_lossy(&name).into_owned();
        for (difficulty, diff) in Difficulty::ALL.into_iter().zip(&mut raw.diffs) {
            let mut report = shared.clone();
            report.section = format!("{name} {}", difficulty.section_prefix());
            report.unpaired_midi_events = raw.unpaired;
            report.sync_events_off_first_track = raw.off_sync;
            built.extend(song_charts.build(instrument, difficulty, diff, report));
        }
    }
    Ok(Converted { song, tracks: built })
}

/// The instrument tracks of the file, one per instrument: a later track of the same instrument is ignored, except that a later
/// `PART GUITAR` replaces an earlier guitar track (both counted).
struct Picked {
    slots: [Option<(Vec<u8>, RawTrack)>; 5],
    unsupported: Vec<String>,
    duplicates: usize,
}

fn pick_tracks(tracks: &[&[u8]], flavour: Flavour) -> RoxResult<Picked> {
    let mut picked = Picked { slots: Default::default(), unsupported: Vec::new(), duplicates: 0 };
    for &chunk in tracks {
        let Some(name) = track_name(chunk, flavour) else { continue };
        let Some(instrument) = instrument_of(name, flavour) else {
            if looks_like_instrument(name) {
                picked.unsupported.push(String::from_utf8_lossy(name).into_owned());
            }
            continue;
        };
        let slot = Instrument::ALL.iter().position(|&i| i == instrument).unwrap_or(0);
        if picked.slots[slot].is_some() {
            picked.duplicates += 1;
            if name != GUITAR_NAME {
                continue;
            }
        }
        picked.slots[slot] = Some((name.to_vec(), read_instrument(chunk)?));
    }
    Ok(picked)
}

/// What every chart of the file shares.
struct SongCharts<'a> {
    song: &'a SongSection,
    ini: Option<&'a SongIni>,
    tempo: &'a MidiTempo,
    timing: Vec<TimingPoint>,
    cutoff: u64,
    shift_us: i64,
}

impl SongCharts<'_> {
    /// The chart of one difficulty; `None` when it has no note.
    fn build(&self, instrument: Instrument, difficulty: Difficulty, diff: &mut DiffNotes, mut report: ChartReport) -> Option<Built> {
        report.forced_flags = diff.forced;
        report.tap_flags = diff.taps;
        report.open_notes_dropped = diff.opens + drop_open_phrase_notes(&mut diff.frets, &mut diff.open_ranges);
        let events = std::mem::take(&mut diff.frets);
        let notes = settle_notes(events, self.cutoff, |tick| self.tempo.us(tick), self.shift_us, &mut report);
        built(notes, || metadata(self.song, self.ini, instrument, difficulty), &self.timing, report, instrument, difficulty)
    }
}

/// The sustain cutoff, in ticks: the `song.ini` `sustain_cutoff_threshold` when it is given and not negative (a value of
/// 0 is read as 1: an ordinary note is 0 ticks long, any longer note is a sustain), else a third of the resolution
/// (`YARG.Core`; the format notes: "shorter than a 1/12th step").
fn sustain_cutoff(ini: Option<&SongIni>, ticks_per_quarter: u32) -> u64 {
    match ini.and_then(|i| i.sustain_cutoff_threshold) {
        Some(t) if t >= 0 => u64::try_from(t).unwrap_or(0).max(1),
        _ => u64::from(ticks_per_quarter / 3),
    }
}
