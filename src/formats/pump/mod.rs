#![warn(clippy::pedantic)]
//! Pump It Up step files: the User Custom Step text (`.ucs`, [`UcsDecoder`]), Andamiro's `STF4` (`.stx`, [`StxDecoder`]) and `NOT4`/`NOT5`
//! (`.not`, [`NotDecoder`]). Decode only.
//!
//! UCS and STX are rows of one cell per panel (5 for single, 10 for double), in blocks of a fixed BPM and rows per beat; a row lasts
//! `60 / BPM / split` seconds. A hold is a start cell, body cells and an end cell on its column. [`Rows`] turns such rows into notes.

mod not;
#[cfg(test)]
mod not_tests;
mod stx;
#[cfg(test)]
mod stx_tests;
mod ucs;
#[cfg(test)]
mod ucs_tests;

pub use not::{NotDecoder, NotReport};
pub use stx::{StxDecoder, StxReport, StxVariant};
pub use ucs::{UcsDecoder, UcsReport};

use rox::model::{Note, RoxChart, TimingPoint};

/// Times further than this many seconds from the start are not read.
const MAX_SECONDS: f64 = 1e7;

/// What one cell of a row holds.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cell {
    Empty,
    Tap,
    HoldStart,
    HoldBody,
    HoldEnd,
    /// Not a known cell: dropped and counted.
    Unknown,
}

/// The walk over the rows of a chart: time, tempo points and open holds.
struct Rows {
    chart: RoxChart,
    seconds: f64,
    holding: Vec<Option<i64>>,
    last_tempo: Option<(u32, u8)>,
    /// Hold parts without their start or end, and unknown cells.
    invalid: usize,
}

impl Rows {
    fn new(columns: u8, start_seconds: f64) -> Self {
        Self { chart: RoxChart::new(columns), seconds: start_seconds, holding: vec![None; usize::from(columns)], last_tempo: None, invalid: 0 }
    }

    /// Plays one row of the block of `bpm`, `split` rows per beat and `beat` beats per measure, then moves on by its length.
    /// `false` when the time ran out of range.
    fn row(&mut self, cells: impl Iterator<Item = Cell>, bpm: f64, split: f64, beat: u8) -> bool {
        if self.seconds.abs() > MAX_SECONDS {
            return false;
        }
        #[allow(clippy::cast_possible_truncation)] // bounded above
        let time = (self.seconds * 1e6).round() as i64;
        #[allow(clippy::cast_possible_truncation)] // a BPM fits f32
        let tempo = ((bpm as f32).to_bits(), beat);
        if self.last_tempo != Some(tempo) {
            self.chart.timing_points.push(TimingPoint::Bpm { time_us: time, bpm: f32::from_bits(tempo.0), signature: beat });
            self.last_tempo = Some(tempo);
        }
        for (column, cell) in cells.enumerate().take(self.holding.len()) {
            #[allow(clippy::cast_possible_truncation)] // at most 10 columns
            let lane = column as u8;
            match (cell, self.holding[column]) {
                (Cell::Empty, _) | (Cell::HoldBody, Some(_)) => {}
                (Cell::Tap, None) => self.chart.notes.push(Note::tap(time, lane)),
                (Cell::HoldStart, None) => self.holding[column] = Some(time),
                (Cell::HoldEnd, Some(start)) => {
                    self.chart.notes.push(Note::hold(start, (time - start).max(1), lane));
                    self.holding[column] = None;
                }
                _ => self.invalid += 1,
            }
        }
        self.seconds += 60.0 / bpm / split;
        true
    }

    /// The chart, its notes in order; `None` when it has none. Holds never ended are counted.
    fn finish(mut self) -> (Option<RoxChart>, usize) {
        let invalid = self.invalid + self.holding.iter().flatten().count();
        if self.chart.notes.is_empty() {
            return (None, invalid);
        }
        self.chart.notes.sort_by_key(|note| (note.time_us, note.column));
        (Some(self.chart), invalid)
    }
}
