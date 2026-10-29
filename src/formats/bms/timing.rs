#![warn(clippy::pedantic)]
//! Beats and seconds of a BMS chart.
//!
//! A position is (measure, fraction of the measure). A measure is 4 beats times its channel-`02` length (default 1).
//! `STOP` values are in 1/192 of a whole note, i.e. 1/48 of a beat, and are NOT scaled by the measure length.
//!
//! At one position the order is: objects are placed first, then the BPM change (channel `03`, then channel `08`: the
//! last one wins), then the STOP, whose duration uses the BPM in effect after the change. An object at the position of
//! a STOP is therefore at the START of the pause.

use std::collections::BTreeMap;

use super::parser::Frac;

/// Number of measures: BMS numbers them with three decimal digits (000..=999), DTX with a base-36 hundreds digit (000..=Z99 = 3599).
const MEASURES: usize = 3600;

/// Beats at the start of every measure, `starts[m]`, and the length in beats of each.
pub(crate) struct Beats {
    starts: Vec<f64>,
    lengths: Vec<f64>,
}

impl Beats {
    pub fn new(measure_len: &BTreeMap<u16, f64>) -> Self {
        let mut starts = Vec::with_capacity(MEASURES + 1);
        let mut lengths = Vec::with_capacity(MEASURES);
        let mut beat = 0.0;
        for measure in 0..MEASURES {
            #[allow(clippy::cast_possible_truncation)] // MEASURES is 3600
            let length = 4.0 * measure_len.get(&(measure as u16)).copied().unwrap_or(1.0);
            starts.push(beat);
            lengths.push(length);
            beat += length;
        }
        starts.push(beat);
        Self { starts, lengths }
    }

    /// Beat of the position `frac` of `measure`.
    pub fn beat(&self, measure: u16, frac: Frac) -> f64 {
        let m = usize::from(measure).min(MEASURES - 1);
        self.starts[m] + frac.as_f64() * self.lengths[m]
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Change {
    /// New BPM (already checked to be finite and positive).
    Bpm(f64),
    /// STOP in 1/48 beat units (already checked to be finite and positive).
    Stop(f64),
}

/// A beat where the tempo changes or the time stops.
#[derive(Debug, Clone, Copy)]
struct Anchor {
    beat: f64,
    /// Seconds at the beat, before a STOP placed there.
    before: f64,
    /// Seconds at the beat, after a STOP placed there.
    after: f64,
    /// BPM from the beat on.
    bpm: f64,
}

pub(crate) struct Timeline {
    anchors: Vec<Anchor>,
}

/// One BPM point of the result: seconds at the beat (before any STOP there) and the BPM from there on.
pub(crate) struct BpmPoint {
    pub seconds: f64,
    pub bpm: f64,
}

impl Timeline {
    /// `changes` must be sorted by beat (stable: BPM before STOP at one beat).
    pub fn new(initial_bpm: f64, changes: &[(f64, Change)]) -> Self {
        let mut anchors = vec![Anchor {
            beat: 0.0,
            before: 0.0,
            after: 0.0,
            bpm: initial_bpm,
        }];
        for &(beat, change) in changes {
            let last = *anchors.last().expect("one anchor at least");
            if beat > last.beat {
                anchors.push(Anchor {
                    beat,
                    before: last.after + (beat - last.beat) * 60.0 / last.bpm,
                    after: last.after + (beat - last.beat) * 60.0 / last.bpm,
                    bpm: last.bpm,
                });
            }
            let anchor = anchors.last_mut().expect("one anchor at least");
            match change {
                Change::Bpm(bpm) => anchor.bpm = bpm,
                Change::Stop(units) => anchor.after += units / 48.0 * 60.0 / anchor.bpm,
            }
        }
        Self { anchors }
    }

    /// Seconds of `beat`.
    pub fn seconds(&self, beat: f64) -> f64 {
        let at = self.anchors.partition_point(|a| a.beat <= beat) - 1;
        let anchor = self.anchors[at];
        if beat <= anchor.beat {
            anchor.before
        } else {
            anchor.after + (beat - anchor.beat) * 60.0 / anchor.bpm
        }
    }

    /// The BPM points, each only where the BPM differs from the one before.
    pub fn bpm_points(&self) -> Vec<BpmPoint> {
        let mut points: Vec<BpmPoint> = Vec::new();
        for anchor in &self.anchors {
            if points
                .last()
                .is_none_or(|p| (p.bpm - anchor.bpm).abs() > f64::EPSILON)
            {
                points.push(BpmPoint {
                    seconds: anchor.before,
                    bpm: anchor.bpm,
                });
            }
        }
        points
    }
}
