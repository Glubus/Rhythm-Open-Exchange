#![warn(clippy::pedantic)]
//! What `.sus` and `.usc` share: the report, the lane-to-column rule, the tempo map and the assembly of the ROX chart. See `mod.rs`
//! for the decisions.

use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, Note, RoxChart, TimingPoint, drop_stacked};

/// Columns of a chart: the 12 lanes of the game.
pub const COLUMNS: u8 = 12;

/// Files above this size are refused.
pub(super) const MAX_FILE_SIZE: usize = 64 * 1024 * 1024;
/// Most note objects read from one file.
pub(super) const MAX_OBJECTS: usize = 4_000_000;
/// Times further than this many seconds from the start of the audio are not read.
const MAX_SECONDS: f64 = 10_000_000.0;
/// Largest BPM read (a BPM is a `f32` in the model).
const MAX_BPM: f64 = 10_000_000.0;

/// What the decoder dropped, skipped or repaired in a file. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // one flag, the rest are counters
pub struct SekaiReport {
    /// The text was not valid UTF-8 and was read lossily (a `.sus`; the notes say it is always UTF-8).
    pub invalid_text: bool,
    /// Ticks per beat of the `.sus` (480 when the file gives none or an invalid one); 0 for a `.usc`.
    pub ticks_per_beat: u32,
    /// The shift applied to every time, in seconds: `-#WAVEOFFSET` of a `.sus`, `offset` of a `.usc`.
    pub shift_seconds: f64,
    /// BPM changes used (after merging the ones on the same beat).
    pub bpm_changes: usize,
    /// BPM definitions or `.usc` `bpm` objects that are not a finite number in `(0, 1e7]`: ignored.
    pub invalid_bpm: usize,
    /// `#mmm08` slots that name a BPM that was never defined (or was invalid): ignored.
    pub unknown_bpm_ids: usize,
    /// `#mmm02` lines whose value is not a number in `(0, 1024]`: ignored.
    pub invalid_measure_lengths: usize,
    /// Scroll speed changes (`#TIL`, `timeScale`, `timeScaleGroup`): ignored.
    pub time_scale_changes: usize,

    /// Critical taps and slides: read as normal ones.
    pub critical_flags: usize,
    /// Taps with a flick or a direction: kept as taps.
    pub flicks: usize,
    /// Trace notes (critical ones included): dropped.
    pub traces: usize,
    /// Damage notes (`.usc` `damage`): dropped.
    pub damage_notes: usize,
    /// Skill activations (SUS tap kind 4, `.usc` `skill`): dropped.
    pub skill_markers: usize,
    /// Fever starts and ends (SUS tap on lane `f`, `.usc` `feverChance`/`feverStart`): dropped.
    pub fever_markers: usize,
    /// Guide slides (decorative): dropped.
    pub guides: usize,
    /// Overlapped notes on a guide's steps (SUS): dropped with the guide.
    pub guide_modifiers: usize,
    /// Joints strictly between the first and the last of the slides kept as holds: dropped.
    pub slide_joints_dropped: usize,
    /// Notes overlapped on a slide's joints (critical, trace, elaser, ease, attach, flick end): not notes, dropped.
    pub slide_modifiers: usize,
    /// Slides whose last joint is on their first's time: kept as taps.
    pub empty_slides_as_taps: usize,
    /// Slides with no end (SUS), or whose last joint is not an end (`.usc`): dropped.
    pub unterminated_slides: usize,
    /// Joints of a SUS channel with no start before them: dropped.
    pub slide_points_without_start: usize,
    /// Slides that go back in time (`.usc`), have no joint or have a joint that is not a number: dropped.
    pub invalid_slides: usize,

    /// Notes (and slides) not entirely on the 12 lanes: dropped.
    pub notes_outside_lanes: usize,
    /// `.usc` notes whose lane is not a whole number of lanes from the left of the stage: floored.
    pub fractional_lanes: usize,
    /// Notes with a width of 0 or less: dropped.
    pub zero_width_notes: usize,
    /// Two taps on the same column and time: the first stays.
    pub duplicate_taps: usize,
    /// Notes inside a hold of their column: dropped.
    pub overlapping_notes_dropped: usize,
    /// Notes later than 10^7 seconds from the start of the audio: skipped.
    pub notes_out_of_range: usize,
    /// Directionals, flicks, elasers and the like with no note beneath: dropped.
    pub stray_overlays: usize,

    /// Pairs of a data line that are not two base-36 digits: skipped.
    pub invalid_pairs: usize,
    /// Data lines of an odd length: the last character is not read.
    pub odd_data_lines: usize,
    /// Notes of kinds this decoder does not read (generic holds and slides 2, unknown kinds, unknown `.usc` connections): dropped.
    pub unsupported_notes: usize,
    /// Data lines whose header is not one of the known ones: skipped.
    pub unsupported_lines: usize,
    /// Directives that do not matter here (`#ATTRIBUTE`, `#HISPEED`, `#SUBTITLE`, ...): ignored.
    pub ignored_directives: usize,
    /// Directives with a value that does not parse (`#WAVEOFFSET`, `#REQUEST`, `#MEASUREBS`, `#PLAYLEVEL`): ignored.
    pub invalid_directives: usize,
    /// `.usc` objects of a type this decoder does not know, or that are not objects: dropped.
    pub unsupported_objects: usize,
    /// `.usc` notes with a field missing or not a number: dropped.
    pub invalid_notes: usize,
}

/// A BPM change, in beats.
pub(super) struct BpmChange {
    pub beat: f64,
    pub bpm: f64,
    /// Beats per measure at the change.
    pub signature: u8,
}

/// One note of the chart, before it has a time.
pub(super) struct ScoreNote {
    pub beat: f64,
    /// `Some` for a hold: the beat of its last joint, after `beat`.
    pub end_beat: Option<f64>,
    pub column: u8,
}

/// A file read to beats.
pub(super) struct Score {
    pub metadata: Metadata,
    /// Seconds added to every time (`-#WAVEOFFSET`).
    pub shift_seconds: f64,
    pub bpm: Vec<BpmChange>,
    pub notes: Vec<ScoreNote>,
}

/// `true` for a finite BPM the model can hold.
pub(super) fn valid_bpm(bpm: f64) -> bool {
    bpm.is_finite() && bpm > 0.0 && bpm <= MAX_BPM
}

/// The column of a note from its leftmost lane (0 is the leftmost lane of the stage) and its width in lanes; `None` when it is not
/// entirely on the 12 lanes.
pub(super) fn column_of(left: i64, width: i64) -> Option<u8> {
    if left < 0 || width < 1 || left + width > i64::from(COLUMNS) {
        return None;
    }
    u8::try_from(left).ok()
}

/// Beats to seconds through the BPM changes.
struct TempoMap {
    /// `(beat, seconds at that beat, seconds per beat)`, sorted by beat.
    segments: Vec<(f64, f64, f64)>,
}

impl TempoMap {
    /// `changes` as `merged_changes` returns them: sorted, one per beat.
    fn new(changes: &[&BpmChange]) -> Option<Self> {
        let first = *changes.first()?;
        // Before the first change the first BPM goes on: the origin is beat 0 (or the first change when it is earlier).
        let mut segments = vec![(first.beat, first.beat * 60.0 / first.bpm, 60.0 / first.bpm)];
        for change in &changes[1..] {
            let &(beat, seconds, per_beat) = segments.last()?;
            segments.push((
                change.beat,
                seconds + (change.beat - beat) * per_beat,
                60.0 / change.bpm,
            ));
        }
        Some(Self { segments })
    }

    fn seconds(&self, beat: f64) -> f64 {
        let at = self
            .segments
            .partition_point(|segment| segment.0 <= beat)
            .saturating_sub(1);
        let (start, seconds, per_beat) = self.segments[at];
        seconds + (beat - start) * per_beat
    }
}

/// A note with its times, before the overlaps are settled.
struct Timed {
    time_us: i64,
    duration_us: Option<i64>,
    column: u8,
}

/// Microseconds of `seconds` plus the shift; `None` when it is out of range or not finite.
fn micros(seconds: f64, shift: f64) -> Option<i64> {
    let total = seconds + shift;
    if !total.is_finite() || total.abs() > MAX_SECONDS {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)] // |total| <= 1e7 s, so the value is at most 1e13
    Some((total * 1_000_000.0).round() as i64)
}

/// Builds the chart from a score: times, the model's rules on overlaps, the timing points.
///
/// # Errors
/// Returns an error if there is no valid BPM or no playable note left.
pub(super) fn build(score: Score, report: &mut SekaiReport, format: &str) -> RoxResult<RoxChart> {
    let changes = merged_changes(&score.bpm);
    let tempo = TempoMap::new(&changes)
        .ok_or_else(|| RoxError::InvalidFormat(format!("{format}: no valid BPM")))?;
    report.shift_seconds = score.shift_seconds;
    let mut timed = Vec::with_capacity(score.notes.len());
    for note in &score.notes {
        let start = micros(tempo.seconds(note.beat), score.shift_seconds);
        let duration = match note.end_beat {
            Some(end) => micros(tempo.seconds(end), score.shift_seconds)
                .zip(start)
                .map(|(end, start)| end - start),
            None => Some(0),
        };
        let (Some(time_us), Some(duration)) = (start, duration) else {
            report.notes_out_of_range += 1;
            continue;
        };
        let duration_us = if note.end_beat.is_none() {
            None
        } else if duration > 0 {
            Some(duration)
        } else {
            report.empty_slides_as_taps += 1;
            None
        };
        timed.push(Timed {
            time_us,
            duration_us,
            column: note.column,
        });
    }
    let notes = settle(timed, report);
    if notes.is_empty() {
        return Err(RoxError::InvalidFormat(format!(
            "{format}: no playable note"
        )));
    }
    let mut chart = RoxChart::new(COLUMNS);
    chart.metadata = score.metadata;
    let first_note = notes.first().map_or(0, |note| note.time_us);
    chart.notes = notes;
    let mut points = 0;
    for (index, change) in changes.iter().enumerate() {
        // The first change sits no later than the first note, as the model requires.
        let at = micros(tempo.seconds(change.beat), score.shift_seconds).unwrap_or(first_note);
        let at = if index == 0 { at.min(first_note) } else { at };
        #[allow(clippy::cast_possible_truncation)]
        // `valid_bpm` bounds it by 1e7, exact in an f32 up to rounding
        let bpm = change.bpm as f32;
        chart.timing_points.push(TimingPoint::Bpm {
            time_us: at,
            bpm,
            signature: change.signature.max(1),
        });
        points += 1;
    }
    report.bpm_changes = points;
    Ok(chart)
}

/// The BPM changes sorted by beat with the last of each beat kept (the ones `TempoMap` uses).
fn merged_changes(changes: &[BpmChange]) -> Vec<&BpmChange> {
    let mut sorted: Vec<&BpmChange> = changes.iter().collect();
    sorted.sort_by(|a, b| a.beat.total_cmp(&b.beat));
    let mut merged: Vec<&BpmChange> = Vec::with_capacity(sorted.len());
    for change in sorted {
        if merged
            .last()
            .is_some_and(|last| last.beat.total_cmp(&change.beat).is_eq())
        {
            merged.pop();
        }
        merged.push(change);
    }
    merged
}

/// The notes, sorted, without the ones the model forbids: inside a hold of their column, and a second tap on the same column and time.
fn settle(timed: Vec<Timed>, report: &mut SekaiReport) -> Vec<Note> {
    let mut notes: Vec<Note> = timed
        .into_iter()
        .map(|t| t.duration_us.map_or_else(|| Note::tap(t.time_us, t.column), |d| Note::hold(t.time_us, d, t.column)))
        .collect();
    let dropped = drop_stacked(&mut notes, COLUMNS, |note| note);
    report.overlapping_notes_dropped += dropped.overlapping;
    report.duplicate_taps += dropped.taps;
    notes
}
