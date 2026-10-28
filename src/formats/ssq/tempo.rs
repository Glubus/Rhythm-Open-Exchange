#![warn(clippy::pedantic)]
//! The tempo chunk (type 1): ticks to audio time, and the BPM changes.
//!
//! The chunk is `N` measure-tick offsets (4096 per whole note) then `N` cumulative times in seconds-ticks (seconds x TPS).
//! The audio time of a tick is the linear interpolation between the two entries around it, which is what the game does at
//! run time; `tempo_data[0]` is the audio time of entry 0, so no separate offset is added. Two entries with the same offset
//! are a stop. There are no stored BPM numbers: they come from the deltas.

use rox::error::{RoxError, RoxResult};

use super::chunks::i32_at;

/// Ticks of one measure (a whole note).
const MEASURE_TICKS: i64 = 4096;

/// A stretch of the timeline between two entries with different offsets.
#[derive(Debug, Clone, Copy)]
struct Segment {
    m0: i64,
    m1: i64,
    s0: i64,
    s1: i64,
}

/// A tempo chunk, ready to place ticks in the audio.
#[derive(Debug)]
pub(super) struct TempoMap {
    tps: i64,
    /// First and last entry as `(offset, seconds-ticks)`.
    first: (i64, i64),
    last: (i64, i64),
    segments: Vec<Segment>,
    /// `(delta offset, delta seconds-ticks)` of the first and last segment that has a BPM: the slope used before the first
    /// entry and after the last one.
    first_slope: (i64, i64),
    last_slope: (i64, i64),
    /// Segments where the offset advances and the clock does not (seen in old charts): no BPM, no time elapses.
    pub instant_segments: usize,
}

/// A BPM, from a tick span and its time span (`tps` seconds-ticks per second).
fn bpm_of(tps: i64, delta_ticks: i64, delta_seconds_ticks: i64) -> f64 {
    #[allow(clippy::cast_precision_loss)] // i64 values below 2^33: exact in f64
    let bpm = 240.0 * tps as f64 * delta_ticks as f64
        / (MEASURE_TICKS as f64 * delta_seconds_ticks as f64);
    bpm
}

/// `numerator / denominator` rounded to the nearest integer (halves up), `denominator > 0`, saturated to `i64`.
fn div_round(numerator: i128, denominator: i128) -> i64 {
    let q = (2 * numerator + denominator).div_euclid(2 * denominator);
    i64::try_from(q).unwrap_or(if q < 0 { i64::MIN } else { i64::MAX })
}

impl TempoMap {
    /// Reads `entries` entries from the body of a tempo chunk.
    ///
    /// # Errors
    /// `tps` is 0, the body is shorter than the entries, the offsets go backwards, the clock goes backwards or a stop is
    /// negative, or no segment has a BPM (nothing to derive a tempo from).
    pub fn parse(tps: u16, entries: usize, body: &[u8]) -> RoxResult<Self> {
        if tps == 0 {
            return Err(RoxError::InvalidFormat(
                "ssq: the tempo chunk has 0 ticks per second".to_string(),
            ));
        }
        if body.len() < entries * 8 {
            return Err(RoxError::InvalidFormat(format!(
                "ssq: the tempo chunk holds {} bytes, {entries} entries need {}",
                body.len(),
                entries * 8
            )));
        }
        let offset = |i: usize| i64::from(i32_at(body, i));
        let clock = |i: usize| i64::from(i32_at(body, entries + i));
        if entries == 0 {
            return Err(RoxError::InvalidFormat(
                "ssq: the tempo chunk has no entry".to_string(),
            ));
        }
        let mut segments = Vec::new();
        let mut instant_segments = 0;
        for i in 1..entries {
            let (delta_m, delta_s) = (offset(i) - offset(i - 1), clock(i) - clock(i - 1));
            if delta_m < 0 {
                return Err(RoxError::InvalidFormat(format!(
                    "ssq: tempo entry {i} goes back in ticks ({} after {})",
                    offset(i),
                    offset(i - 1)
                )));
            }
            if delta_s < 0 {
                return Err(RoxError::InvalidFormat(format!(
                    "ssq: tempo entry {i} goes back in time ({} after {})",
                    clock(i),
                    clock(i - 1)
                )));
            }
            if delta_m == 0 {
                continue; // a stop (or a repeated entry): the clock jump is in the next segment's start
            }
            if delta_s == 0 {
                instant_segments += 1;
            }
            segments.push(Segment {
                m0: offset(i - 1),
                m1: offset(i),
                s0: clock(i - 1),
                s1: clock(i),
            });
        }
        let slope = |segment: &Segment| (segment.m1 - segment.m0, segment.s1 - segment.s0);
        let first_slope = segments.iter().map(slope).find(|&(_, s)| s > 0);
        let last_slope = segments.iter().rev().map(slope).find(|&(_, s)| s > 0);
        let (Some(first_slope), Some(last_slope)) = (first_slope, last_slope) else {
            return Err(RoxError::InvalidFormat(
                "ssq: the tempo chunk has no segment with a BPM".to_string(),
            ));
        };
        Ok(Self {
            tps: i64::from(tps),
            first: (offset(0), clock(0)),
            last: (offset(entries - 1), clock(entries - 1)),
            segments,
            first_slope,
            last_slope,
            instant_segments,
        })
    }

    /// Audio time in µs of `base_s` seconds-ticks at `base_m`, moved to `tick` along the slope `(delta_m, delta_s)`.
    fn along(&self, base_m: i64, base_s: i64, slope: (i64, i64), tick: i64) -> i64 {
        let (delta_m, delta_s) = (i128::from(slope.0), i128::from(slope.1));
        let seconds_ticks = i128::from(base_s) * delta_m + i128::from(tick - base_m) * delta_s;
        div_round(seconds_ticks * 1_000_000, delta_m * i128::from(self.tps))
    }

    /// The audio time in µs of a note at `tick`. A note on a stop's tick is hit when the stop begins. Before the first entry
    /// and after the last one the nearest segment with a BPM is extended.
    pub fn time_us(&self, tick: i64) -> i64 {
        if tick <= self.first.0 {
            return self.along(self.first.0, self.first.1, self.first_slope, tick);
        }
        if tick > self.last.0 {
            return self.along(self.last.0, self.last.1, self.last_slope, tick);
        }
        // The first segment ending at or after `tick`: it starts before it, as `tick > first.0` and segments are contiguous.
        let index = self.segments.partition_point(|s| s.m1 < tick);
        let segment = &self.segments[index.min(self.segments.len() - 1)];
        self.along(
            segment.m0,
            segment.s0,
            (segment.m1 - segment.m0, segment.s1 - segment.s0),
            tick,
        )
    }

    /// The BPM changes as `(time_us, bpm)`: one per segment that has a BPM, a BPM equal to the one before dropped. The first
    /// change is moved back to tick 0 when the first segment starts after it.
    pub fn bpm_points(&self) -> Vec<(i64, f32)> {
        let mut points: Vec<(i64, f32)> = Vec::new();
        for segment in &self.segments {
            let (delta_m, delta_s) = (segment.m1 - segment.m0, segment.s1 - segment.s0);
            if delta_s <= 0 {
                continue;
            }
            #[allow(clippy::cast_possible_truncation)] // a BPM fits f32
            let bpm = bpm_of(self.tps, delta_m, delta_s) as f32;
            if points
                .last()
                .is_some_and(|&(_, last)| last.to_bits() == bpm.to_bits())
            {
                continue;
            }
            let time = div_round(i128::from(segment.s0) * 1_000_000, i128::from(self.tps));
            points.push((time, bpm));
        }
        if let Some(first) = points.first_mut() {
            first.0 = first.0.min(self.time_us(0));
        }
        points
    }
}
