#![warn(clippy::pedantic)]
//! What the Bestdori JSON and the BanG! Simulator `.txt` share: the report, the lane-to-column rule, the tempo map and the assembly of
//! the ROX chart. See `mod.rs` for the decisions.

use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, Note, RoxChart, TimingPoint};

/// Columns of a chart: the 7 lanes of the game.
pub const COLUMNS: u8 = 7;

/// Most note objects (JSON elements, simulator lines) read from one file.
pub(super) const MAX_OBJECTS: usize = 4_000_000;
/// Times further than this many seconds from the start of the audio are not read.
const MAX_SECONDS: f64 = 10_000_000.0;
/// Largest BPM read (a BPM is a `f32` in the model).
const MAX_BPM: f64 = 10_000_000.0;
/// The tempo before the first BPM event: `BanGConverter` writes 120 as the simulator's BPM line when a Bestdori chart has no BPM at beat 0.
pub(super) const DEFAULT_BPM: f64 = 120.0;
/// A lane this close to a whole number is that lane (the lanes of both formats are whole numbers; a float in a JSON is just how it was
/// written).
const LANE_TOLERANCE: f64 = 1e-4;
/// Lane values are clamped to this before they become an integer, so no arithmetic on them can overflow.
const LANE_BOUND: f64 = 1_000_000.0;
/// Beats per measure of a timing point: the formats say nothing about measures, the game is in 4/4.
const SIGNATURE: u8 = 4;

/// What the decoder dropped, skipped or repaired in a file. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BandoriReport {
    /// The text was not valid UTF-8 and was read lossily (a simulator `.txt`).
    pub invalid_text: bool,
    /// The simulator's first line (the "note offset", 46 in the files `BanGConverter` writes): READ, NOT APPLIED. The notes give no unit
    /// (milliseconds is an inference) and no sign, and it is the simulator's calibration of its own latency, not a property of the chart.
    /// `None` for a Bestdori JSON, which has none.
    pub note_offset: Option<f64>,
    /// BPM changes used (after merging the ones on the same beat).
    pub bpm_changes: usize,
    /// BPM events that are not a finite number in `(0, 1e7]` (after the absolute value), or lack their beat: ignored.
    pub invalid_bpm: usize,
    /// Bestdori BPM events with a negative value: the absolute value is used (`ayachan` does the same).
    pub negative_bpms: usize,
    /// Simulator `0/0/0` lines after the first: ignored (the first says where the music starts).
    pub extra_music_starts: usize,

    /// Notes with a flick (a tap with `flick`, a simulator type 2, a slide or long note that ends with a flick): kept as taps or holds.
    pub flicks: usize,
    /// Directional flicks (`Directional`, simulator types 51 to 57 and 61 to 67): kept as taps, the direction and the width are dropped.
    pub directional_flicks: usize,
    /// Notes with the skill flag (`skill`, simulator types 11, 31 to 36, 75, 76): kept, the flag is dropped.
    pub skill_flags: usize,
    /// Notes with the fever charge flag (`charge`, simulator type 10): kept, the flag is dropped.
    pub charge_flags: usize,
    /// Joints strictly between the first and the last of the slides and long notes kept as holds (hidden ones included): dropped.
    pub slide_joints_dropped: usize,
    /// Slides whose path leaves the lane of their first joint: kept as a hold on that lane (the path is lost).
    pub slides_with_lane_changes: usize,
    /// `Long` and `Slide` notes with a single connection: kept as a tap.
    pub single_connection_slides: usize,
    /// Slides and long notes whose last joint is on their first's beat: kept as taps.
    pub empty_slides_as_taps: usize,
    /// Simulator slides and long notes with no end (a new start before it, or the end of the file): dropped.
    pub unterminated_slides: usize,
    /// Simulator slide or long note joints (among or end) with no start before them: dropped.
    pub slide_points_without_start: usize,
    /// Bestdori `Long`/`Slide` notes with no connection, a connection without a beat or a lane, or that go back in time: dropped.
    pub invalid_slides: usize,

    /// Notes (and slides) on a lane outside the 7 of the game (Bestdori 0..=6, simulator 1..=7): dropped.
    pub lanes_out_of_range: usize,
    /// Notes (and slides) on a lane that is not a whole number: dropped.
    pub fractional_lanes: usize,
    /// Notes (and slides) before the start of the audio (a negative beat, or before the simulator's `0/0/0`): dropped.
    pub notes_before_start: usize,
    /// Notes whose time is beyond 10^7 seconds: dropped.
    pub notes_out_of_range: usize,
    /// Notes that fall inside a hold of their column: dropped (the model forbids it, and a slide of the game does pass over other lanes).
    pub overlapping_notes_dropped: usize,
    /// A second tap on the same column and time: dropped.
    pub duplicate_taps: usize,

    /// JSON elements with a `type` this decoder does not know (a Bestdori v1 `System`/`Note` included), or none; simulator types it does not know: dropped.
    pub unknown_types: usize,
    /// JSON elements that are not objects: dropped.
    pub invalid_objects: usize,
    /// Notes with a field missing or not a number (JSON), lines that are not `beat/type/lane` (simulator): dropped.
    pub invalid_notes: usize,
}

/// A BPM change, in beats.
pub(super) struct BpmChange {
    pub beat: f64,
    pub bpm: f64,
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
    /// The beat at which the audio starts (0, or the simulator's `0/0/0`): every time counts from it.
    pub start_beat: f64,
    pub bpm: Vec<BpmChange>,
    pub notes: Vec<ScoreNote>,
}

/// `true` for a finite BPM the model can hold.
pub(super) fn valid_bpm(bpm: f64) -> bool {
    bpm.is_finite() && bpm > 0.0 && bpm <= MAX_BPM
}

/// The column of a lane number (`first` is the number of the leftmost lane: 0 in a Bestdori JSON, 1 in a simulator `.txt`); `None`, counted,
/// when it is not a whole number or not one of the 7 lanes.
pub(super) fn column_of(lane: f64, first: i64, report: &mut BandoriReport) -> Option<u8> {
    let nearest = lane.round();
    if !lane.is_finite() || (lane - nearest).abs() > LANE_TOLERANCE {
        report.fractional_lanes += 1;
        return None;
    }
    #[allow(clippy::cast_possible_truncation)] // clamped to +-1e6
    let index = nearest.clamp(-LANE_BOUND, LANE_BOUND) as i64 - first;
    match u8::try_from(index) {
        Ok(column) if column < COLUMNS => Some(column),
        _ => {
            report.lanes_out_of_range += 1;
            None
        }
    }
}

/// Beats to seconds through the BPM changes.
struct TempoMap {
    /// `(beat, seconds at that beat, seconds per beat)`, sorted by beat; the first segment starts at beat 0.
    segments: Vec<(f64, f64, f64)>,
}

impl TempoMap {
    /// `initial` is the BPM from beat 0, `changes` are strictly after it, sorted, one per beat.
    fn new(initial: f64, changes: &[&BpmChange]) -> Self {
        let mut segments = vec![(0.0, 0.0, 60.0 / initial)];
        for change in changes {
            let &(beat, seconds, per_beat) = segments.last().unwrap_or(&(0.0, 0.0, 0.5));
            segments.push((
                change.beat,
                seconds + (change.beat - beat) * per_beat,
                60.0 / change.bpm,
            ));
        }
        Self { segments }
    }

    /// A negative beat is timed at the first segment's tempo, backwards.
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

/// Microseconds of `seconds`; `None` when it is out of range or not finite.
fn micros(seconds: f64) -> Option<i64> {
    if !seconds.is_finite() || seconds.abs() > MAX_SECONDS {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)] // |seconds| <= 1e7, so the value is at most 1e13
    Some((seconds * 1_000_000.0).round() as i64)
}

/// The BPM at beat 0 (the last event at or before it; 120 when there is none) and the changes after it (sorted, the last of each beat kept).
fn tempo_of(bpm: &[BpmChange]) -> (f64, Vec<&BpmChange>) {
    let mut sorted: Vec<&BpmChange> = bpm.iter().collect();
    sorted.sort_by(|a, b| a.beat.total_cmp(&b.beat));
    let split = sorted.partition_point(|change| change.beat <= 0.0);
    let initial = split
        .checked_sub(1)
        .map_or(DEFAULT_BPM, |last| sorted[last].bpm);
    let mut merged: Vec<&BpmChange> = Vec::with_capacity(sorted.len() - split);
    for change in &sorted[split..] {
        if merged
            .last()
            .is_some_and(|last| last.beat.total_cmp(&change.beat).is_eq())
        {
            merged.pop();
        }
        merged.push(change);
    }
    (initial, merged)
}

/// Builds the chart from a score: times, the model's rules on overlaps, the timing points.
///
/// # Errors
/// Returns an error if no playable note is left.
pub(super) fn build(score: Score, report: &mut BandoriReport, format: &str) -> RoxResult<RoxChart> {
    let (initial, changes) = tempo_of(&score.bpm);
    let tempo = TempoMap::new(initial, &changes);
    let origin = tempo.seconds(score.start_beat);
    let mut timed = Vec::with_capacity(score.notes.len());
    for note in &score.notes {
        if note.beat < score.start_beat {
            report.notes_before_start += 1;
            continue;
        }
        let start = micros(tempo.seconds(note.beat) - origin);
        let duration = match note.end_beat {
            Some(end) => micros(tempo.seconds(end) - origin)
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
    chart.notes = notes;
    // The tempo points as times; the ones before the audio collapse into the one in effect at time 0, which is the first point.
    let beats = std::iter::once((0.0, initial)).chain(changes.iter().map(|c| (c.beat, c.bpm)));
    let mut points: Vec<(i64, f64)> = beats
        .filter_map(|(beat, bpm)| Some((micros(tempo.seconds(beat) - origin)?, bpm)))
        .collect();
    let in_effect = points
        .partition_point(|point| point.0 <= 0)
        .saturating_sub(1);
    points.drain(..in_effect);
    if let Some(first) = points.first_mut() {
        // A tempo from before the first note goes on backwards: the model wants a first point no later than the first note.
        first.0 = 0;
    }
    for (time_us, bpm) in &points {
        #[allow(clippy::cast_possible_truncation)]
        // `valid_bpm` bounds it by 1e7, exact in an f32 up to rounding
        let bpm = *bpm as f32;
        chart.timing_points.push(TimingPoint::Bpm {
            time_us: *time_us,
            bpm,
            signature: SIGNATURE,
        });
    }
    report.bpm_changes = points.len();
    Ok(chart)
}

/// Sorts the notes by time and column (holds first at a tie) and drops the ones the model forbids: inside a hold of their column,
/// and a second tap on the same column and time.
fn settle(mut timed: Vec<Timed>, report: &mut BandoriReport) -> Vec<Note> {
    timed.sort_by(|a, b| {
        a.time_us
            .cmp(&b.time_us)
            .then(a.column.cmp(&b.column))
            .then(b.duration_us.is_some().cmp(&a.duration_us.is_some()))
    });
    let mut busy_until = [i64::MIN; COLUMNS as usize];
    let mut last_tap: [Option<i64>; COLUMNS as usize] = [None; COLUMNS as usize];
    let mut kept = Vec::with_capacity(timed.len());
    for note in timed {
        let column = usize::from(note.column);
        if note.time_us < busy_until[column] {
            report.overlapping_notes_dropped += 1;
            continue;
        }
        if note.duration_us.is_none() && last_tap[column] == Some(note.time_us) {
            report.duplicate_taps += 1;
            continue;
        }
        if let Some(duration) = note.duration_us {
            busy_until[column] = note.time_us.saturating_add(duration);
            last_tap[column] = None;
            kept.push(Note::hold(note.time_us, duration, note.column));
        } else {
            last_tap[column] = Some(note.time_us);
            kept.push(Note::tap(note.time_us, note.column));
        }
    }
    kept
}
