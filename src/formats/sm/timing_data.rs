#![warn(clippy::pedantic)]
//! Beat to time conversion of a StepMania chart: BPM changes, stops, delays and warps.
//!
//! - a **stop** freezes the song after the notes of its beat (a note on the stop's beat is hit
//!   before the freeze);
//! - a **delay** freezes it before the notes of its beat;
//! - a **warp** makes the beats it covers pass instantly (time does not advance), and the notes on
//!   those beats can not be hit (see [`TimingData::is_warped`]).
//!
//! Anything with a non-positive or non-finite value is ignored.
#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec};

/// Tolerance on beats computed from fractions of a measure.
const EPSILON: f64 = 1e-6;

/// What happens at a beat.
#[derive(Debug, Clone, Copy)]
enum Event {
    WarpEnd,
    Bpm(f64),
    WarpStart,
    Stop(f64),
    Delay(f64),
}

impl Event {
    /// Order of the events of one beat: a warp ends before the next begins, a BPM change
    /// applies before the freeze.
    fn rank(self) -> u8 {
        match self {
            Self::WarpEnd => 0,
            Self::Bpm(_) => 1,
            Self::WarpStart => 2,
            Self::Stop(_) => 3,
            Self::Delay(_) => 4,
        }
    }
}

/// The state of the timeline from one event beat to the next.
#[derive(Debug, Clone, Copy)]
struct Anchor {
    beat: f64,
    /// Seconds when the beat is reached.
    arrival: f64,
    /// Seconds of a note on the beat (after its delays).
    note: f64,
    /// Seconds when the beat is left (after its delays and stops).
    departure: f64,
    bpm: f64,
    warping: bool,
}

/// A chart's timeline: sorted events turned into anchors, searched by beat.
#[derive(Debug, Clone)]
pub struct TimingData {
    anchors: Vec<Anchor>,
    fakes: Vec<(f64, f64)>,
}

fn valid(beat: f64, value: f64) -> bool {
    beat.is_finite() && value.is_finite() && value > 0.0
}

impl TimingData {
    /// `bpms`, `stops` (seconds), `delays` (seconds), `warps` (beats) and `fakes` (beats) are
    /// `(beat, value)` pairs. The first BPM applies from beat 0 whatever its beat; without any
    /// BPM, 120 is assumed.
    #[must_use]
    pub fn new(
        bpms: &[(f64, f64)],
        stops: &[(f64, f64)],
        delays: &[(f64, f64)],
        warps: &[(f64, f64)],
        fakes: &[(f64, f64)],
    ) -> Self {
        let mut bpms: Vec<(f64, f64)> =
            bpms.iter().copied().filter(|&(b, v)| valid(b, v)).collect();
        bpms.sort_by(|a, b| a.0.total_cmp(&b.0));
        let first_bpm = bpms.first().map_or(120.0, |&(_, bpm)| bpm);
        let mut events: Vec<(f64, Event)> = Vec::new();
        events.extend(bpms.iter().map(|&(b, v)| (b.max(0.0), Event::Bpm(v))));
        events.extend(
            stops
                .iter()
                .filter(|&&(b, v)| valid(b, v))
                .map(|&(b, v)| (b.max(0.0), Event::Stop(v))),
        );
        events.extend(
            delays
                .iter()
                .filter(|&&(b, v)| valid(b, v))
                .map(|&(b, v)| (b.max(0.0), Event::Delay(v))),
        );
        for &(beat, length) in warps.iter().filter(|&&(b, v)| valid(b, v)) {
            events.push((beat.max(0.0), Event::WarpStart));
            events.push(((beat + length).max(0.0), Event::WarpEnd));
        }
        events.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.rank().cmp(&b.1.rank())));
        Self {
            anchors: sweep(first_bpm, &events),
            fakes: fakes
                .iter()
                .copied()
                .filter(|&(b, v)| valid(b, v))
                .map(|(b, v)| (b, b + v))
                .collect(),
        }
    }

    fn anchor(&self, beat: f64) -> &Anchor {
        let index = self
            .anchors
            .partition_point(|a| a.beat <= beat + EPSILON)
            .saturating_sub(1);
        &self.anchors[index]
    }

    fn seconds_after(anchor: &Anchor, beat: f64) -> f64 {
        if anchor.warping {
            anchor.departure
        } else {
            anchor.departure + (beat - anchor.beat) * 60.0 / anchor.bpm
        }
    }

    /// Time in microseconds of a note on `beat`.
    #[must_use]
    pub fn note_us(&self, beat: f64) -> i64 {
        let anchor = self.anchor(beat);
        let seconds = if (beat - anchor.beat).abs() <= EPSILON {
            anchor.note
        } else {
            Self::seconds_after(anchor, beat)
        };
        micros(seconds)
    }

    /// Time in microseconds when the song reaches `beat`, before any stop or delay of that beat
    /// (where a BPM change of the beat takes effect).
    #[must_use]
    pub fn arrival_us(&self, beat: f64) -> i64 {
        let anchor = self.anchor(beat);
        let seconds = if (beat - anchor.beat).abs() <= EPSILON {
            anchor.arrival
        } else {
            Self::seconds_after(anchor, beat)
        };
        micros(seconds)
    }

    /// Whether a note on `beat` lies inside a warp and can not be hit.
    #[must_use]
    pub fn is_warped(&self, beat: f64) -> bool {
        self.anchor(beat).warping
    }

    /// Whether a note on `beat` lies inside a fake region and is not judged.
    #[must_use]
    pub fn is_fake(&self, beat: f64) -> bool {
        self.fakes
            .iter()
            .any(|&(start, end)| beat + EPSILON >= start && beat + EPSILON < end)
    }
}

// Events are grouped by the exact beat they were parsed at (equal values, not computed ones): a margin would merge distinct beats.
#[allow(clippy::float_cmp)]
fn sweep(first_bpm: f64, events: &[(f64, Event)]) -> Vec<Anchor> {
    let mut anchors = vec![Anchor {
        beat: 0.0,
        arrival: 0.0,
        note: 0.0,
        departure: 0.0,
        bpm: first_bpm,
        warping: false,
    }];
    let (mut beat, mut seconds, mut bpm, mut warps) = (0.0_f64, 0.0_f64, first_bpm, 0_u32);
    let mut index = 0;
    while index < events.len() {
        let at = events[index].0;
        if warps == 0 {
            seconds += (at - beat) * 60.0 / bpm;
        }
        beat = at;
        let (mut delay, mut stop) = (0.0, 0.0);
        while index < events.len() && events[index].0 == at {
            match events[index].1 {
                Event::Bpm(value) => bpm = value,
                Event::WarpStart => warps += 1,
                Event::WarpEnd => warps = warps.saturating_sub(1),
                Event::Stop(value) => stop += value,
                Event::Delay(value) => delay += value,
            }
            index += 1;
        }
        let note = seconds + delay;
        let departure = note + stop;
        anchors.push(Anchor {
            beat,
            arrival: seconds,
            note,
            departure,
            bpm,
            warping: warps > 0,
        });
        seconds = departure;
    }
    anchors
}

fn micros(seconds: f64) -> i64 {
    #[allow(clippy::cast_possible_truncation)] // seconds→µs i64: safe for any realistic timestamp
    let us = (seconds * 1_000_000.0).round() as i64;
    us
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(bpms: &[(f64, f64)]) -> TimingData {
        TimingData::new(bpms, &[], &[], &[], &[])
    }

    #[test]
    fn constant_tempo_and_a_change() {
        let td = plain(&[(0.0, 120.0), (4.0, 60.0)]);
        assert_eq!(td.note_us(0.0), 0);
        assert_eq!(td.note_us(2.0), 1_000_000);
        assert_eq!(td.note_us(4.0), 2_000_000);
        assert_eq!(td.note_us(5.0), 3_000_000);
    }

    #[test]
    fn a_stop_freezes_after_its_beat_and_a_delay_before_it() {
        let stop = TimingData::new(&[(0.0, 120.0)], &[(2.0, 1.5)], &[], &[], &[]);
        assert_eq!(stop.note_us(2.0), 1_000_000);
        assert_eq!(stop.note_us(3.0), 1_000_000 + 1_500_000 + 500_000);
        let delay = TimingData::new(&[(0.0, 120.0)], &[], &[(2.0, 1.5)], &[], &[]);
        assert_eq!(delay.note_us(2.0), 2_500_000);
        assert_eq!(delay.arrival_us(2.0), 1_000_000);
    }

    #[test]
    fn a_warp_skips_its_beats_and_flags_their_notes() {
        let td = TimingData::new(&[(0.0, 120.0)], &[], &[], &[(4.0, 2.0)], &[]);
        assert!(!td.is_warped(3.5));
        assert!(td.is_warped(4.0) && td.is_warped(5.5));
        assert!(!td.is_warped(6.0));
        assert_eq!(td.note_us(6.0), td.note_us(4.0));
        assert_eq!(td.note_us(7.0), 2_500_000);
    }

    #[test]
    fn fakes_cover_a_half_open_range() {
        let td = TimingData::new(&[(0.0, 120.0)], &[], &[], &[], &[(8.0, 2.0)]);
        assert!(!td.is_fake(7.99) && td.is_fake(8.0) && td.is_fake(9.5) && !td.is_fake(10.0));
    }

    #[test]
    fn invalid_values_are_ignored_and_the_first_bpm_covers_the_start() {
        let td = TimingData::new(
            &[(8.0, 60.0), (9.0, -5.0), (10.0, f64::NAN)],
            &[(1.0, -1.0)],
            &[],
            &[(2.0, 0.0)],
            &[],
        );
        assert_eq!(td.note_us(1.0), 1_000_000);
        assert!(!td.is_warped(2.0));
        assert_eq!(
            TimingData::new(&[], &[], &[], &[], &[]).note_us(2.0),
            1_000_000
        );
    }
}
