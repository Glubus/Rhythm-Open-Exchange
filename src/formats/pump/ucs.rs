//! Pump It Up User Custom Step charts (`.ucs`).
//!
//! Format notes: `vsrg-format-docs/vsrg/pump-it-up/ucs`, from the UCS editors (`PIUUCSMaker`'s validator and player, `VainUCS`; read for
//! facts), checked on the corpus charts.
//!
//! - **Header**: `:Format=1`, then `:Mode=` `Single` or `S-Performance` (5 columns), `Double` or `D-Performance` (10 columns).
//! - **Blocks**: `:BPM=`, `:Delay=`, `:Beat=`, `:Split=` (in any order), then note rows of one character per column. A row lasts
//!   `60 / BPM / Split` seconds; the first block's `Delay` (ms) places the first row (the editors ignore the others' and so does this
//!   reader); `Beat` is the beats per measure.
//! - **Notes**: `X` a tap; `M` starts a hold, `H` continues it, `W` ends it; `.` nothing. A `W` or `H` without its `M`, an `M` never
//!   ended and any other character are dropped and counted.

use rox::error::{RoxError, RoxResult};
use rox::model::RoxChart;

use super::{Cell, Rows};

/// Decoder of `.ucs` charts: one chart per file.
pub struct UcsDecoder;
crate::impl_format!(UcsDecoder, ["ucs"]);

/// Files above this size are refused (a chart is a few hundred kilobytes).
const MAX_FILE_SIZE: usize = 32 * 1024 * 1024;

/// What the decoder dropped. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UcsReport {
    /// Hold parts without their start or end, and characters other than `. X M H W`: dropped.
    pub invalid_notes: usize,
    /// Rows of another width than the mode's, and header lines of another key: skipped.
    pub skipped_lines: usize,
}

impl rox::codec::Decoder for UcsDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode(data).map(|(chart, _)| chart)
    }
}

impl UcsDecoder {
    /// Decodes a chart and says what was dropped.
    ///
    /// # Errors
    /// Returns an error if the file is too large, not a UCS chart (`:Format=1`, a known `:Mode=`), a block lacks a valid BPM or
    /// split before its rows, or the chart has no note.
    pub fn decode_with_report(data: &[u8]) -> RoxResult<(RoxChart, UcsReport)> {
        decode(data)
    }
}

fn invalid(reason: impl std::fmt::Display) -> RoxError {
    RoxError::InvalidFormat(format!("ucs: {reason}"))
}

/// The settings of the current block.
#[derive(Default)]
struct Block {
    bpm: Option<f64>,
    split: Option<f64>,
    beat: Option<u8>,
    delay: Option<f64>,
}

fn cell(byte: u8) -> Cell {
    match byte {
        b'.' => Cell::Empty,
        b'X' => Cell::Tap,
        b'M' => Cell::HoldStart,
        b'H' => Cell::HoldBody,
        b'W' => Cell::HoldEnd,
        _ => Cell::Unknown,
    }
}

fn decode(data: &[u8]) -> RoxResult<(RoxChart, UcsReport)> {
    if data.len() > MAX_FILE_SIZE {
        return Err(invalid(format!("file too large: {} bytes", data.len())));
    }
    let text = String::from_utf8_lossy(data);
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    if lines.next() != Some(":Format=1") {
        return Err(invalid("no :Format=1"));
    }
    let width: u8 = match lines.next().and_then(|line| line.strip_prefix(":Mode=")) {
        Some("Single" | "S-Performance") => 5,
        Some("Double" | "D-Performance") => 10,
        other => return Err(invalid(format!("unknown mode {other:?}"))),
    };
    let mut report = UcsReport::default();
    let mut rows: Option<Rows> = None;
    let mut block = Block::default();
    // Whether the last line was a row: a header line after rows starts the next block.
    let mut in_rows = false;
    for line in lines {
        if let Some((key, value)) = line.strip_prefix(':').and_then(|line| line.split_once('=')) {
            if in_rows {
                block = Block::default();
                in_rows = false;
            }
            let number = value.trim().parse::<f64>().ok().filter(|value| value.is_finite());
            match key {
                "BPM" => block.bpm = number.filter(|bpm| *bpm > 0.0),
                "Split" => block.split = number.filter(|split| *split >= 1.0),
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // range checked
                "Beat" => block.beat = number.filter(|beat| (1.0..=255.0).contains(beat)).map(|beat| beat as u8),
                "Delay" => block.delay = number,
                _ => report.skipped_lines += 1,
            }
            continue;
        }
        let (Some(bpm), Some(split)) = (block.bpm, block.split) else {
            return Err(invalid("a row before its block's BPM and Split"));
        };
        in_rows = true;
        // Only the first block's delay places the chart (the editors ignore the others').
        let walk = rows.get_or_insert_with(|| Rows::new(width, block.delay.unwrap_or(0.0) / 1000.0));
        if line.len() != usize::from(width) {
            report.skipped_lines += 1;
            continue;
        }
        if !walk.row(line.bytes().map(cell), bpm, split, block.beat.unwrap_or(4)) {
            return Err(invalid("time out of range"));
        }
    }
    let (chart, invalid_notes) = rows.map_or((None, 0), Rows::finish);
    report.invalid_notes = invalid_notes;
    let chart = chart.ok_or_else(|| invalid("no note"))?;
    chart.validate()?;
    Ok((chart, report))
}
