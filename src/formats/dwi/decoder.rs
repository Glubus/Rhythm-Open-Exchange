#![warn(clippy::pedantic)]
//! `.dwi` decoder: every playable chart of the file is a variant.
use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::RoxChart;

use super::parser::{Dialect, Parsed, parse_detailed, parse_dialect};
use crate::formats::sm::decoder::from_chart;
use crate::formats::sm::ssc_decoder::first_chart;
use crate::formats::sm::types::SmFile;

/// Decoder of `.dwi` files: the first playable chart of the file (see [`DwiDecoder::variants`]
/// for the others).
pub struct DwiDecoder;
crate::impl_format!(DwiDecoder, ["dwi"]);

/// A playable chart of a `.dwi` file. `index` counts the playable charts from 0, in file order
/// (the numbering of [`DwiDecoder::decode_variant`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub index: usize,
    /// `dance-single`, `dance-solo` or `dance-double`.
    pub stepstype: String,
    /// The difficulty as the `.sm` reader names it (`Easy`, `Medium`, `Hard`, `Challenge`,
    /// `Beginner`, `Edit`); the name as written in the file when it is none of them.
    pub difficulty: String,
    /// The difficulty as written in the file (`BASIC`, `ANOTHER`, `MANIAC`, `SMANIAC`...).
    pub name: String,
    /// The feet (1 when empty).
    pub meter: u32,
    pub key_count: u8,
    pub notes: usize,
    pub holds: usize,
    pub duration_us: i64,
}

/// What reading a `.dwi` skipped or repaired. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DwiReport {
    /// Charts left out of the file (`COUPLE` steps, no steps, timing that cannot be honoured).
    pub skipped: Vec<String>,
    /// Tags and chart contents that were ignored or repaired (invalid `#FREEZE`, step
    /// characters, panels the step type lacks, holds never closed).
    pub warnings: Vec<String>,
    /// Playable charts of the file.
    pub playable_charts: usize,
}

impl Decoder for DwiDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        first_chart(&parse_detailed(data)?.file)
    }
}

impl DwiDecoder {
    /// The `.sm` types of the file: its metadata, timing and playable charts (see
    /// [`super::parser::parse`]).
    ///
    /// # Errors
    /// Returns an error if the data is too large.
    pub fn parse(data: &[u8]) -> RoxResult<SmFile> {
        super::parser::parse(data)
    }

    /// The playable charts of the file, in file order.
    ///
    /// # Errors
    /// Returns an error if the data is too large.
    pub fn variants(data: &[u8]) -> RoxResult<Vec<Variant>> {
        variants_of(&parse_dialect(data, Dialect::Dwi)?.file)
    }

    /// Decodes playable chart `index` (the numbering of [`Self::variants`]).
    ///
    /// # Errors
    /// Returns an error if the file has no such chart.
    pub fn decode_variant(data: &[u8], index: usize) -> RoxResult<RoxChart> {
        Self::decode_with_report(data, index).map(|(chart, _)| chart)
    }

    /// [`Self::decode_variant`] and what the reader skipped or repaired.
    ///
    /// # Errors
    /// Returns an error if the file has no such chart.
    pub fn decode_with_report(data: &[u8], index: usize) -> RoxResult<(RoxChart, DwiReport)> {
        decode_nth(parse_dialect(data, Dialect::Dwi)?, index, "dwi")
    }
}

/// Decoder of `.msd` files (the DDR'99 simulator script DWI grew from; see [`Dialect::Msd`]): the first playable chart of the file.
pub struct MsdDecoder;
crate::impl_format!(MsdDecoder, ["msd"]);

impl Decoder for MsdDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        first_chart(&parse_dialect(data, Dialect::Msd)?.file)
    }
}

impl MsdDecoder {
    /// The playable charts of the file, in file order.
    ///
    /// # Errors
    /// Returns an error if the data is too large.
    pub fn variants(data: &[u8]) -> RoxResult<Vec<Variant>> {
        variants_of(&parse_dialect(data, Dialect::Msd)?.file)
    }

    /// Decodes playable chart `index` (the numbering of [`Self::variants`]).
    ///
    /// # Errors
    /// Returns an error if the file has no such chart.
    pub fn decode_variant(data: &[u8], index: usize) -> RoxResult<RoxChart> {
        Self::decode_with_report(data, index).map(|(chart, _)| chart)
    }

    /// [`Self::decode_variant`] and what the reader skipped or repaired.
    ///
    /// # Errors
    /// Returns an error if the file has no such chart.
    pub fn decode_with_report(data: &[u8], index: usize) -> RoxResult<(RoxChart, DwiReport)> {
        decode_nth(parse_dialect(data, Dialect::Msd)?, index, "msd")
    }
}

#[allow(clippy::unnecessary_wraps)] // the same shape as the readers that call it
fn variants_of(file: &SmFile) -> RoxResult<Vec<Variant>> {
    Ok(file
        .charts
        .iter()
        .enumerate()
        .map(|(index, chart)| {
            let rox = from_chart(file, chart);
            Variant {
                index,
                stepstype: chart.stepstype.clone(),
                difficulty: chart.difficulty.clone(),
                name: chart.description.clone(),
                meter: chart.meter,
                key_count: chart.column_count,
                notes: rox.notes.len(),
                holds: rox.notes.iter().filter(|note| note.is_hold()).count(),
                duration_us: rox.duration_us(),
            }
        })
        .collect())
}

fn decode_nth(Parsed { file, warnings }: Parsed, index: usize, format: &str) -> RoxResult<(RoxChart, DwiReport)> {
    let Some(chart) = file.charts.get(index) else {
        return Err(RoxError::InvalidFormat(if file.charts.is_empty() {
            match first_chart(&file) {
                Err(error) => error.to_string(),
                Ok(_) => "No charts found".to_string(),
            }
        } else {
            format!("{format}: {} playable charts, not chart {index}", file.charts.len())
        }));
    };
    let rox = from_chart(&file, chart);
    rox.validate()?;
    Ok((rox, DwiReport { skipped: file.skipped.clone(), warnings, playable_charts: file.charts.len() }))
}
