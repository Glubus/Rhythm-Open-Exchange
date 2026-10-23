#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{format, string::ToString};

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::RoxChart;

use super::decoder::from_chart;
use super::ssc_parser;
use super::types::SmFile;

/// Decodes the first playable chart of an `.ssc` file; use [`ssc_parser::parse`] and
/// [`from_chart`] to reach the others.
pub struct SscDecoder;
crate::impl_format!(SscDecoder, ["ssc"]);

impl Decoder for SscDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        first_chart(&ssc_parser::parse(data)?)
    }
}

/// The first chart of a parsed file, or why there is none.
///
/// # Errors
/// Returns an error naming the skipped charts when the file has none that can be played.
pub fn first_chart(file: &SmFile) -> RoxResult<RoxChart> {
    match file.charts.first() {
        Some(chart) => Ok(from_chart(file, chart)),
        None if file.skipped.is_empty() => {
            Err(RoxError::InvalidFormat("No charts found".to_string()))
        }
        None => Err(RoxError::InvalidFormat(format!(
            "No playable chart: {}",
            file.skipped.join("; ")
        ))),
    }
}
