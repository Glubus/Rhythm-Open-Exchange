//! Pump It Up `.not` step files: `NOT4` (no signature, obfuscated sparse steps, no holds) and `NOT5` (`pump 5.0`, plain arrays with
//! holds). One chart per file.
//!
//! Format notes: `vsrg-format-docs/vsrg/pump-it-up/not`, from `UCSConverterTool` (`not_file.dart`, `not_converter.dart`; read for facts),
//! checked on the corpus files (The Fusion 3rd Dance Floor, all `NOT4`).
//!
//! - **Tempo**: BPMs (3 in `NOT4`, 10 in `NOT5`), start times and "bunki" times (centiseconds; a bunki is when the next BPM takes over, 0
//!   ends the list, a repeated one is skipped), lines per beat, beats per measure. Line `p` of tempo segment `j` plays at
//!   `start[j] + p / (BPM[j] / 6000 × split)` centiseconds; a line belongs to the last segment whose bunki it has reached. (The converter
//!   moves later segments by empirical amounts, 16 or 14 cs, to fit the UCS block model; that is not reproduced: no corpus file changes
//!   tempo.)
//! - **`NOT4` steps** (from 0x88, each byte XOR `0xD2 + 0x95 × i`): 4-byte records `u16` line, `u16` mask; bits 6 to 15 are the
//!   panels, bit 15 the left-most of 1P, bit 6 the right-most of 2P; the low bits are stage lights. Every panel bit is a step.
//! - **`NOT5` steps** (from 0xD8): three arrays of one `u16` per line, steps, hold starts, hold ends (10-bit masks, bit 9 the left-most of
//!   1P). A step of a held panel is the hold's body.
//! - **Columns**: 5 (1P), or 10 when the file name has `_XD` or `_DB` (the converter's rule) or a step uses a 2P panel.

use rox::error::{RoxError, RoxResult};
use rox::model::{Note, RoxChart, TimingPoint};

/// Decoder of `.not` charts: one chart per file. The file name says whether the chart is double; use [`NotDecoder::decode_named`].
pub struct NotDecoder;
crate::impl_format!(NotDecoder, ["not", "not5"]);

/// Files above this size are refused (a chart is a few kilobytes).
const MAX_FILE_SIZE: usize = 16 * 1024 * 1024;
const NOT5_MAGIC: &[u8] = b"pump 5.0";
/// Most lines a chart may have.
const MAX_LINES: u64 = 1 << 24;

/// What the decoder dropped. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotReport {
    /// Whether the file is a `NOT5` (else `NOT4`).
    pub not5: bool,
    /// Hold starts on a held panel, hold ends on a free one and holds never ended: dropped.
    pub broken_holds: usize,
    /// A step record cut short at the end of a `NOT4`: dropped, as the converter does.
    pub truncated_record: bool,
}

fn invalid(reason: impl std::fmt::Display) -> RoxError {
    RoxError::InvalidFormat(format!("not: {reason}"))
}

fn u32_at(data: &[u8], at: usize) -> RoxResult<u32> {
    let bytes = data.get(at..at + 4).ok_or_else(|| invalid("cut short"))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4 bytes")))
}

fn f32_at(data: &[u8], at: usize) -> RoxResult<f64> {
    Ok(f64::from(f32::from_bits(u32_at(data, at)?)))
}

/// The tempo of a file: segments `(start cs, lines per cs, BPM)`, their bunkis, beats per measure.
struct Tempo {
    segments: Vec<(f64, f64, f64)>,
    bunkis: Vec<f64>,
    beats_per_measure: u8,
}

impl Tempo {
    fn read(data: &[u8], count: usize, bpm_at: usize, start_at: usize, bunki_at: usize, split_at: usize, default_measure: u32) -> RoxResult<Self> {
        let split = f64::from(u32_at(data, split_at)?);
        if split == 0.0 {
            return Err(invalid("0 lines per beat"));
        }
        let measure = match u32_at(data, split_at + 4)? {
            0 => default_measure,
            measure => measure,
        };
        let segment = |index: usize| -> RoxResult<(f64, f64, f64)> {
            let bpm = f32_at(data, bpm_at + 4 * index)?;
            if !(bpm.is_finite() && bpm > 0.0) {
                return Err(invalid(format!("BPM {bpm}")));
            }
            Ok((f64::from(u32_at(data, start_at + 4 * index)?), bpm / 6000.0 * split, bpm))
        };
        let mut segments = vec![segment(0)?];
        let mut bunkis = Vec::new();
        for index in 0..count - 1 {
            let bunki = u32_at(data, bunki_at + 4 * index)?;
            if bunki == 0 {
                break;
            }
            if bunkis.last() == Some(&f64::from(bunki)) {
                continue;
            }
            segments.push(segment(index + 1)?);
            bunkis.push(f64::from(bunki));
        }
        Ok(Self { segments, bunkis, beats_per_measure: u8::try_from(measure).unwrap_or(4) })
    }

    /// The time of line `line` in microseconds and its segment.
    fn time(&self, line: u64) -> (i64, usize) {
        #[allow(clippy::cast_precision_loss)] // lines are bounded
        let line = line as f64;
        let mut chosen = 0;
        for (index, bunki) in self.bunkis.iter().enumerate() {
            let (start, rate, _) = self.segments[index + 1];
            if start + line / rate >= *bunki {
                chosen = index + 1;
            }
        }
        let (start, rate, _) = self.segments[chosen];
        #[allow(clippy::cast_possible_truncation)] // bounded lines and times
        ((((start + line / rate) * 10_000.0).round()) as i64, chosen)
    }
}

impl NotDecoder {
    /// Decodes a chart read from a file named `name`: a name with `_XD` or `_DB` is a double chart.
    ///
    /// # Errors
    /// Returns an error if the file is too large or cut short, a BPM or the lines per beat are not valid, or the chart has no step.
    pub fn decode_named(data: &[u8], name: &str) -> RoxResult<(RoxChart, NotReport)> {
        let upper = name.to_ascii_uppercase();
        decode(data, upper.contains("_XD") || upper.contains("_DB"))
    }
}

impl rox::codec::Decoder for NotDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode(data, false).map(|(chart, _)| chart)
    }
}

/// `(line, mask, hold start mask, hold end mask)`: the 10-bit masks, bit 9 the left-most panel of 1P.
type Line = (u64, u16, u16, u16);

fn not4_lines(data: &[u8], report: &mut NotReport) -> Vec<Line> {
    let body = data.get(0x88..).unwrap_or_default();
    #[allow(clippy::cast_possible_truncation)] // the key byte wraps by design
    let byte = |index: usize| body[index] ^ (0xD2u32 + 0x95 * (index as u32 % 256)) as u8;
    let records = body.len() / 4;
    report.truncated_record = body.len() % 4 != 0;
    (0..records)
        .map(|record| {
            let at = record * 4;
            let line = u16::from_le_bytes([byte(at), byte(at + 1)]);
            let mask = u16::from_le_bytes([byte(at + 2), byte(at + 3)]) >> 6;
            (u64::from(line), mask, 0, 0)
        })
        .filter(|&(_, mask, _, _)| mask != 0)
        .collect()
}

fn not5_lines(data: &[u8]) -> RoxResult<Vec<Line>> {
    let count = u64::from(u32_at(data, 10)?);
    if count > MAX_LINES {
        return Err(invalid(format!("{count} lines")));
    }
    #[allow(clippy::cast_possible_truncation)] // bounded above
    let count = count as usize;
    let arrays = data.get(0xD8..0xD8 + 6 * count).ok_or_else(|| invalid("step arrays cut short"))?;
    let mask = |array: usize, line: usize| {
        let at = (array * count + line) * 2;
        (u16::from(arrays[at + 1] & 3) << 8) | u16::from(arrays[at])
    };
    Ok((0..count).map(|line| (line as u64, mask(0, line), mask(1, line), mask(2, line))).filter(|&(_, step, start, end)| step | start | end != 0).collect())
}

fn decode(data: &[u8], named_double: bool) -> RoxResult<(RoxChart, NotReport)> {
    if data.len() > MAX_FILE_SIZE {
        return Err(invalid(format!("file too large: {} bytes", data.len())));
    }
    let not5 = data.starts_with(NOT5_MAGIC);
    let mut report = NotReport { not5, ..NotReport::default() };
    let (tempo, lines) = if not5 {
        (Tempo::read(data, 10, 16, 56, 96, 136, 16)?, not5_lines(data)?)
    } else {
        (Tempo::read(data, 3, 8, 24, 40, 56, 4)?, not4_lines(data, &mut report))
    };
    let uses_2p = lines.iter().any(|&(_, step, start, end)| (step | start | end) & 0b11111 != 0);
    let columns: u8 = if named_double || uses_2p { 10 } else { 5 };
    let mut chart = RoxChart::new(columns);
    let mut holding: [Option<i64>; 10] = [None; 10];
    let mut last_segment = None;
    for (line, step, start, end) in lines {
        let (time, segment) = tempo.time(line);
        if last_segment != Some(segment) {
            #[allow(clippy::cast_possible_truncation)] // a BPM fits f32
            chart.timing_points.push(TimingPoint::Bpm { time_us: time, bpm: tempo.segments[segment].2 as f32, signature: tempo.beats_per_measure });
            last_segment = Some(segment);
        }
        for column in 0..columns {
            let bit = 1 << (9 - column);
            let slot = &mut holding[usize::from(column)];
            match (start & bit != 0, end & bit != 0, *slot) {
                (true, true, _) | (true, false, Some(_)) | (false, true, None) => report.broken_holds += 1,
                (true, false, None) => *slot = Some(time),
                (false, true, Some(begin)) => {
                    chart.notes.push(Note::hold(begin, (time - begin).max(1), column));
                    *slot = None;
                }
                (false, false, None) if step & bit != 0 => chart.notes.push(Note::tap(time, column)),
                (false, false, _) => {}
            }
        }
    }
    report.broken_holds += holding.iter().flatten().count();
    if chart.notes.is_empty() {
        return Err(invalid("no step"));
    }
    chart.notes.sort_by_key(|note| (note.time_us, note.column));
    if chart.timing_points.first().is_some_and(|point| point.time_us() > chart.notes[0].time_us) {
        return Err(invalid("a step before the first tempo"));
    }
    chart.validate()?;
    Ok((chart, report))
}
