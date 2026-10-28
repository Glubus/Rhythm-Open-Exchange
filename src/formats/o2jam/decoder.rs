#![warn(clippy::pedantic)]
//! `.ojn` decoder: events to a seven-column ROX chart, plus the audio plan.
//!
//! See `mod.rs` for the decisions. The steps are: header and section bytes (`ojn.rs`), the events sorted by position,
//! the measure lengths and the tempo (here), then one pass over the sorted events that pairs long notes lane by lane
//! and collects every sound.

use std::collections::BTreeMap;

use rox::audio_plan::{AudioPlan, SoundEvent};
use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, Note, RoxChart, TimingPoint};

use super::ojn::{OjnHeader, RawEvent, Slot, scan_section};
use crate::formats::bms::bmson::safe_name;

/// Decoder of `.ojn` files: the first playable difficulty of the file (see [`OjnDecoder::variants`] for the others).
pub struct OjnDecoder;
crate::impl_format!(OjnDecoder, ["ojn"]);

/// Lanes of an `O2Jam` chart.
const KEYS: u8 = 7;
/// Measures above this number are not read (real charts have a few hundred).
const MAX_MEASURE: u32 = 100_000;
/// Events later than this many seconds into the chart are not read (a tempo close to 0 would send them out of range).
const MAX_SECONDS: f64 = 1.0e7;

/// The three difficulties of an `.ojn`, in file order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    const ALL: [Self; 3] = [Self::Easy, Self::Normal, Self::Hard];

    /// The name `O2Jam` gives it: `EX`, `NX`, `HX`.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Easy => "EX",
            Self::Normal => "NX",
            Self::Hard => "HX",
        }
    }
}

/// A playable difficulty of an `.ojn` file. A difficulty without a single playable note is not a variant, so
/// `index` counts the playable ones from 0 and `source_index` is the place in the file (0 Easy, 1 Normal, 2 Hard).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// Number to give to [`OjnDecoder::decode_variant`]: `0..variants.len()`.
    pub index: usize,
    /// Place in the file: 0 Easy, 1 Normal, 2 Hard.
    pub source_index: usize,
    pub name: Difficulty,
    /// Level written in the header.
    pub level: i16,
    /// Notes of the decoded chart (a long note is one).
    pub notes: usize,
    /// Long notes among them.
    pub holds: usize,
    pub key_count: u8,
    pub duration_us: i64,
}

/// What the decoder chose, skipped or repaired for one difficulty. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OjnReport {
    /// Place of the difficulty in the file: 0 Easy, 1 Normal, 2 Hard.
    pub source_index: usize,
    /// Notes the header counts for the difficulty (`note_count`).
    pub header_note_count: i32,
    /// Non-empty slots found on the seven note lanes (a long note is two: head and tail); this is what the header counts.
    pub lane_events: usize,
    /// The sample container named by the header, sanitised; `None` when the name is empty or unsafe.
    pub ojm_file: Option<String>,
    /// The sample container name was refused: absolute, a drive, or climbing out of the folder.
    pub unsafe_names: usize,
    /// The section ended inside a package: the packages read before it are used.
    pub truncated_section: bool,
    /// Packages with a negative measure: skipped.
    pub negative_measures: usize,
    /// Events after measure 100000, or later than 10^7 seconds: skipped.
    pub events_out_of_range: usize,
    /// Note events with a negative sample value: skipped.
    pub negative_sample_values: usize,
    /// BPM events that are not positive or not a number: ignored.
    pub ignored_bpm_events: usize,
    /// BPM events equal to the BPM before them: dropped.
    pub repeated_bpm_events: usize,
    /// The header BPM is not positive and no BPM event sits at beat 0: the first BPM event is the initial tempo.
    pub header_bpm_unusable: bool,
    /// Measure-fraction events that are negative or not a number: ignored.
    pub ignored_measure_fractions: usize,
    /// Notes of type 1 (named "unused" by the format notes): played as normal notes, as `Open2Jam` does.
    pub type_one_notes: usize,
    /// Normal notes and long-note heads inside a long note of their lane: moved to the background sounds.
    pub notes_inside_holds_to_bgm: usize,
    /// Long-note tails with no head: removed (no sound).
    pub stray_hold_ends: usize,
    /// Long-note heads never closed: decoded as taps.
    pub unclosed_holds_as_taps: usize,
    /// Long notes that end at or before their start: decoded as taps.
    pub empty_holds_as_taps: usize,
    /// Long-note tails on a background channel: removed (no sound).
    pub background_hold_ends_removed: usize,
    /// Notes that start inside a long note or at the same time on the same column as another note: dropped from the chart (their sound stays in the plan).
    pub overlapping_notes_dropped: usize,
}

/// A difficulty, converted.
struct Built {
    chart: RoxChart,
    report: OjnReport,
    plan: AudioPlan,
    source_index: usize,
    level: i16,
    holds: usize,
}

impl Decoder for OjnDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode_nth(data, 0).map(|built| built.chart)
    }
}

impl OjnDecoder {
    /// The playable difficulties of the file, in file order. A difficulty with no playable note is left out.
    ///
    /// # Errors
    /// Returns an error if the header is not an `.ojn` header.
    pub fn variants(data: &[u8]) -> RoxResult<Vec<Variant>> {
        Ok(build_all(data, usize::MAX)?
            .1
            .iter()
            .enumerate()
            .map(|(index, built)| Variant {
                index,
                source_index: built.source_index,
                name: Difficulty::ALL[built.source_index],
                level: built.level,
                notes: built.chart.notes.len(),
                holds: built.holds,
                key_count: KEYS,
                duration_us: built.chart.duration_us(),
            })
            .collect())
    }

    /// Decodes playable difficulty `index` (the numbering of [`Self::variants`]).
    ///
    /// # Errors
    /// Returns an error if the file is not an `.ojn`, or has no such playable difficulty.
    pub fn decode_variant(data: &[u8], index: usize) -> RoxResult<RoxChart> {
        decode_nth(data, index).map(|built| built.chart)
    }

    /// [`Self::decode_variant`] and what the decoder chose, skipped or repaired.
    ///
    /// # Errors
    /// Returns an error if the file is not an `.ojn`, or has no such playable difficulty.
    pub fn decode_with_report(data: &[u8], index: usize) -> RoxResult<(RoxChart, OjnReport)> {
        decode_nth(data, index).map(|built| (built.chart, built.report))
    }

    /// [`Self::decode_with_report`] plus the [`AudioPlan`]: every playable note's sample and every background sound, at
    /// their chart times, as events on the sample container the header names (`index` = the sample id in that `.ojm`).
    /// A long-note tail makes no sound. The volume nibble of a note becomes the event's volume; the pan nibble is not
    /// used (a plan has no pan).
    ///
    /// # Errors
    /// Returns an error if the file is not an `.ojn`, or has no such playable difficulty.
    pub fn decode_with_audio_plan(
        data: &[u8],
        index: usize,
    ) -> RoxResult<(RoxChart, OjnReport, AudioPlan)> {
        decode_nth(data, index).map(|built| (built.chart, built.report, built.plan))
    }
}

fn decode_nth(data: &[u8], index: usize) -> RoxResult<Built> {
    let (_, mut built) = build_all(data, index.saturating_add(1))?;
    if index < built.len() {
        let one = built.swap_remove(index);
        one.chart.validate()?;
        Ok(one)
    } else {
        Err(RoxError::InvalidFormat(format!(
            "ojn: {} playable difficulties, not difficulty {index}",
            built.len()
        )))
    }
}

/// The playable difficulties in file order, stopping once `want` of them are found.
fn build_all(data: &[u8], want: usize) -> RoxResult<(OjnHeader, Vec<Built>)> {
    let header = OjnHeader::parse(data)?;
    let mut all = Vec::new();
    for source_index in 0..3 {
        if all.len() >= want {
            break;
        }
        // A difficulty that cannot give a chart (no usable tempo) is as good as empty.
        if let Ok(built) = convert(&header, data, source_index)
            && !built.chart.notes.is_empty()
        {
            all.push(built);
        }
    }
    Ok((header, all))
}

/// Total order of two positions: the measure, then `index / count` compared without dividing.
fn position_order(a: &RawEvent, b: &RawEvent) -> std::cmp::Ordering {
    a.measure.cmp(&b.measure).then_with(|| {
        (u32::from(a.index) * u32::from(b.count)).cmp(&(u32::from(b.index) * u32::from(a.count)))
    })
}

/// Beats at the start of the measures whose length is not 4.
struct Beats {
    /// `(measure, beats added before the measure after it)`, by measure.
    extra: Vec<(u32, f64)>,
}

impl Beats {
    fn new(lengths: &BTreeMap<u32, f64>) -> Self {
        let mut total = 0.0;
        let extra = lengths
            .iter()
            .map(|(&measure, &length)| {
                total += 4.0 * (length - 1.0);
                (measure, total)
            })
            .collect();
        Self { extra }
    }

    /// Beat of slot `index` of `count` in `measure`: the slots are spread on a full 4-beat measure, whatever the
    /// length of the measure (the next measure starts after the shortened length), as `Open2Jam` and Raindrop do.
    fn beat(&self, event: &RawEvent) -> f64 {
        let before = self.extra.partition_point(|&(m, _)| m < event.measure);
        let shift = if before == 0 {
            0.0
        } else {
            self.extra[before - 1].1
        };
        4.0 * f64::from(event.measure)
            + shift
            + 4.0 * f64::from(event.index) / f64::from(event.count)
    }
}

/// Tempo as anchors: beat, seconds at the beat, BPM from the beat on.
struct Tempo {
    anchors: Vec<(f64, f64, f64)>,
}

impl Tempo {
    fn new(initial: f64, changes: &[(f64, f64)]) -> Self {
        let mut anchors = vec![(0.0, 0.0, initial)];
        for &(beat, bpm) in changes {
            let last = *anchors.last().expect("one anchor at least");
            if beat > last.0 {
                anchors.push((beat, last.1 + (beat - last.0) * 60.0 / last.2, bpm));
            } else if let Some(anchor) = anchors.last_mut() {
                anchor.2 = bpm;
            }
        }
        Self { anchors }
    }

    fn seconds(&self, beat: f64) -> f64 {
        let at = self.anchors.partition_point(|a| a.0 <= beat).max(1) - 1;
        let (anchor_beat, anchor_seconds, bpm) = self.anchors[at];
        anchor_seconds + (beat - anchor_beat) * 60.0 / bpm
    }

    /// `None` when the time is out of the range the decoder reads.
    fn micros(&self, beat: f64) -> Option<i64> {
        let seconds = self.seconds(beat);
        #[allow(clippy::cast_possible_truncation)] // bounded by MAX_SECONDS just below
        (seconds.is_finite() && seconds <= MAX_SECONDS)
            .then(|| (seconds * 1_000_000.0).round() as i64)
    }
}

/// A playable note before columns are checked: a tap, or a hold with its end.
struct Draft {
    start: i64,
    end: Option<i64>,
    column: u8,
}

fn volume_percent(volume_pan: u8) -> Option<u8> {
    // High nibble 1..=15 is the volume in sixteenths; 0 means the maximum.
    match volume_pan >> 4 {
        0 => None,
        nibble => Some(u8::try_from((u32::from(nibble) * 100 + 8) / 16).unwrap_or(100)),
    }
}

#[allow(clippy::too_many_lines)] // one pass: tempo, long notes, sounds
fn convert(header: &OjnHeader, data: &[u8], source_index: usize) -> RoxResult<Built> {
    let mut report = OjnReport {
        source_index,
        header_note_count: header.note_count[source_index],
        ..OjnReport::default()
    };
    let (mut events, scan) = scan_section(header.section(data, source_index));
    report.truncated_section = scan.truncated;
    report.negative_measures = scan.negative_measures;
    let before = events.len();
    events.retain(|event| event.measure <= MAX_MEASURE);
    report.events_out_of_range += before - events.len();
    events.sort_by(position_order);

    // Measure lengths: the last value of a measure wins.
    let mut lengths: BTreeMap<u32, f64> = BTreeMap::new();
    for event in &events {
        if let Slot::MeasureFraction(value) = event.slot {
            if value.is_finite() && value > 0.0 {
                lengths.insert(event.measure, f64::from(value));
            } else {
                report.ignored_measure_fractions += 1;
            }
        }
    }
    let beats = Beats::new(&lengths);

    // Tempo: the BPM events by beat; consecutive equal values are dropped; the header BPM is the one at beat 0 unless
    // an event sits there.
    let mut changes: Vec<(f64, f64)> = Vec::new();
    for event in &events {
        if let Slot::Bpm(value) = event.slot {
            if value.is_finite() && value > 0.0 {
                changes.push((beats.beat(event), f64::from(value)));
            } else {
                report.ignored_bpm_events += 1;
            }
        }
    }
    changes.sort_by(|a, b| a.0.total_cmp(&b.0));
    let header_bpm = f64::from(header.bpm);
    let initial = if header_bpm.is_finite() && header_bpm > 0.0 {
        header_bpm
    } else {
        report.header_bpm_unusable = true;
        changes
            .first()
            .map(|change| change.1)
            .ok_or_else(|| RoxError::InvalidFormat("ojn: no usable BPM".into()))?
    };
    let mut current = initial;
    changes.retain(|&(_, bpm)| {
        let repeated = (bpm - current).abs() < f64::EPSILON;
        if repeated {
            report.repeated_bpm_events += 1;
        }
        current = bpm;
        !repeated
    });
    let tempo = Tempo::new(initial, &changes);

    // One pass over the notes in position order.
    let ojm = safe_name(&header.ojm_file, &mut report.unsafe_names);
    report.ojm_file.clone_from(&ojm);
    let mut sounds: Vec<SoundEvent> = Vec::new();
    let mut drafts: Vec<Draft> = Vec::new();
    let mut open: [Option<usize>; 7] = [None; 7];
    for event in &events {
        let Slot::Note {
            lane,
            value,
            volume_pan,
            note_type,
        } = event.slot
        else {
            continue;
        };
        if lane != 0 {
            report.lane_events += 1;
        }
        let Ok(reference) = u32::try_from(value - 1) else {
            report.negative_sample_values += 1;
            continue;
        };
        // Bit 2 of the type: the sample is one of the Ogg ones (ids from 1000).
        let id = reference + if note_type % 8 > 3 { 1000 } else { 0 };
        let kind = note_type % 4;
        let Some(time) = tempo.micros(beats.beat(event)) else {
            report.events_out_of_range += 1;
            continue;
        };
        let push_sound = |sounds: &mut Vec<SoundEvent>| {
            if let Some(file) = &ojm {
                let mut event = SoundEvent::in_container(time, file.as_str(), id);
                event.volume = volume_percent(volume_pan);
                sounds.push(event);
            }
        };
        if lane == 0 {
            // Background sound. A long-note tail there is not a sound (`Open2Jam` removes it).
            if kind == 3 {
                report.background_hold_ends_removed += 1;
            } else {
                push_sound(&mut sounds);
            }
            continue;
        }
        let column = lane - 1;
        let slot = &mut open[usize::from(column)];
        match kind {
            0 | 1 => {
                if kind == 1 {
                    report.type_one_notes += 1;
                }
                push_sound(&mut sounds);
                if slot.is_some() {
                    report.notes_inside_holds_to_bgm += 1;
                } else {
                    drafts.push(Draft {
                        start: time,
                        end: None,
                        column,
                    });
                }
            }
            2 => {
                push_sound(&mut sounds);
                if slot.is_some() {
                    report.notes_inside_holds_to_bgm += 1;
                } else {
                    *slot = Some(drafts.len());
                    drafts.push(Draft {
                        start: time,
                        end: None,
                        column,
                    });
                }
            }
            _ => {
                if let Some(head) = slot.take() {
                    drafts[head].end = Some(time);
                } else {
                    report.stray_hold_ends += 1;
                }
            }
        }
    }
    report.unclosed_holds_as_taps = open.iter().flatten().count();

    let mut notes: Vec<Note> = Vec::with_capacity(drafts.len());
    let mut holds = 0usize;
    for draft in &drafts {
        match draft.end {
            Some(end) if end > draft.start => {
                notes.push(Note::hold(draft.start, end - draft.start, draft.column));
            }
            Some(_) => {
                report.empty_holds_as_taps += 1;
                notes.push(Note::tap(draft.start, draft.column));
            }
            None => notes.push(Note::tap(draft.start, draft.column)),
        }
    }
    notes.sort_by_key(|note| (note.time_us, u8::from(!note.is_hold()), note.column));
    let mut last_end = [i64::MIN; 7];
    let mut last_start = [i64::MIN; 7];
    let mut kept = Vec::with_capacity(notes.len());
    for note in notes {
        let column = usize::from(note.column);
        if note.time_us < last_end[column] || note.time_us == last_start[column] {
            report.overlapping_notes_dropped += 1;
            continue;
        }
        last_end[column] = note.end_time_us();
        last_start[column] = note.time_us;
        holds += usize::from(note.is_hold());
        kept.push(note);
    }

    let mut chart = RoxChart::new(KEYS);
    chart.metadata = metadata(header, source_index);
    let mut last_bpm = f64::NAN;
    for &(_, seconds, bpm) in &tempo.anchors {
        if bpm.to_bits() != last_bpm.to_bits() {
            #[allow(clippy::cast_possible_truncation)] // a BPM fits f32; seconds are bounded
            chart.timing_points.push(TimingPoint::bpm(
                (seconds * 1_000_000.0).round() as i64,
                bpm as f32,
            ));
            last_bpm = bpm;
        }
    }
    chart.notes = kept;
    Ok(Built {
        chart,
        report,
        plan: AudioPlan::new(sounds),
        source_index,
        level: header.level[source_index],
        holds,
    })
}

fn metadata(header: &OjnHeader, source_index: usize) -> Metadata {
    Metadata {
        title: header.title.as_str().into(),
        artist: header.artist.as_str().into(),
        creator: header.noter.as_str().into(),
        difficulty_name: Difficulty::ALL[source_index].code().into(),
        difficulty_value: Some(f32::from(header.level[source_index])),
        // The sounds live in the `.ojm`: the audio plan names it, the chart has no audio file.
        audio_file: "".into(),
        genre: Some(header.genre_name().into()),
        ..Metadata::default()
    }
}
