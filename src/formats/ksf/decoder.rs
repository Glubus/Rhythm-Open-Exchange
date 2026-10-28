#![warn(clippy::pedantic)]
//! `.ksf` decoder: the one chart of the file.
use rox::codec::Decoder;
use rox::error::RoxResult;
use rox::model::RoxChart;

use super::parser::{KsfContext, Parsed, parse_detailed};
use crate::formats::sm::ssc_decoder::first_chart;
use crate::formats::sm::types::SmFile;

/// Decoder of `.ksf` files. A `.ksf` holds ONE chart, and its step type and difficulty are in the
/// FILE NAME: use [`KsfDecoder::decode_named`] or [`KsfDecoder::decode_with_report`] with the name
/// (`Decoder::decode`, without it, reads a `pump-single` at difficulty Hard unless `#PLAYER` says
/// `double`).
pub struct KsfDecoder;
crate::impl_format!(KsfDecoder, ["ksf"]);

/// What reading a `.ksf` decided, skipped or repaired. Nothing here changes the chart; it
/// explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KsfReport {
    /// `pump-single`, `pump-halfdouble`, `pump-double` or `pump-couple`, from the file name.
    /// Empty when the file was refused before its step type was known (a bad `#TICKCOUNT`).
    pub stepstype: String,
    /// The difficulty the file name gives (`Beginner`, `Easy`, `Medium`, `Hard`, `Challenge`, `Edit`).
    pub difficulty: String,
    /// `#DIFFICULTY` (at least 1), else the default of the difficulty.
    pub meter: u32,
    /// Whether the file counts as "KIU compliant" (header BPM changes; no Direct Move rows).
    pub kiu_compliant: bool,
    /// Why the chart was left out (empty when it is played).
    pub skipped: Vec<String>,
    /// Rows and tags that were ignored or repaired.
    pub warnings: Vec<String>,
}

impl Decoder for KsfDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        first_chart(&parse_detailed(data, &KsfContext::default())?.file)
    }
}

impl KsfDecoder {
    /// The `.sm` types of the file: its metadata, timing and chart (see
    /// [`super::parser::parse`]); `skipped` says why `charts` is empty when it is.
    ///
    /// # Errors
    /// Returns an error if the data is too large.
    pub fn parse(data: &[u8], context: &KsfContext<'_>) -> RoxResult<SmFile> {
        super::parser::parse(data, context)
    }

    /// Decodes the chart of the file called `file_name`.
    ///
    /// # Errors
    /// Returns an error naming why the chart can not be played (`pump-double` and `pump-couple`
    /// have 10 columns, a row is too short, the timing is not valid).
    pub fn decode_named(data: &[u8], file_name: &str) -> RoxResult<RoxChart> {
        Self::decode_with_report(
            data,
            &KsfContext {
                file_name,
                folder_name: None,
            },
        )
        .map(|(chart, _)| chart)
    }

    /// [`Self::decode_named`] with the folder name for the metadata, and what the decoder decided,
    /// skipped or repaired.
    ///
    /// # Errors
    /// Returns an error naming why the chart can not be played.
    pub fn decode_with_report(
        data: &[u8],
        context: &KsfContext<'_>,
    ) -> RoxResult<(RoxChart, KsfReport)> {
        let Parsed {
            file,
            warnings,
            kiu_compliant,
        } = parse_detailed(data, context)?;
        let chart = first_chart(&file)?;
        chart.validate()?;
        let played = &file.charts[0];
        Ok((
            chart,
            KsfReport {
                stepstype: played.stepstype.clone(),
                difficulty: played.difficulty.clone(),
                meter: played.meter,
                kiu_compliant,
                skipped: file.skipped,
                warnings,
            },
        ))
    }
}
