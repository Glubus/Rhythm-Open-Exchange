#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};

use core::fmt::Write;
use rox::codec::Encoder;
use rox::error::{RoxError, RoxResult};
use rox::model::RoxChart;

use super::encoder::{
    difficulty_class, meter, write_bpms_section, write_measures_section, write_metadata_section,
};

/// Writes one chart as an `.ssc` file (version 0.83, a single `#NOTEDATA` block), with the
/// header and the measures the `.sm` encoder writes.
pub struct SscEncoder;
crate::impl_format!(SscEncoder, ["ssc"]);

impl Encoder for SscEncoder {
    fn encode_inner(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        let stepstype = match chart.key_count {
            4 => "dance-single",
            5 => "pump-single",
            6 => "dance-solo",
            7 => "kb7-single",
            8 => "dance-double",
            other => {
                return Err(RoxError::InvalidFormat(format!(
                    ".ssc has no step type for {other} columns (4, 5, 6, 7 or 8)"
                )));
            }
        };
        let mut out = String::new();
        let _ = writeln!(out, "#VERSION:0.83;");
        write_metadata_section(&mut out, chart);
        write_bpms_section(&mut out, chart);
        let _ = writeln!(out, "#NOTEDATA:;");
        let _ = writeln!(out, "#STEPSTYPE:{stepstype};");
        let _ = writeln!(out, "#DESCRIPTION:{};", chart.metadata.difficulty_name);
        let _ = writeln!(out, "#DIFFICULTY:{};", difficulty_class(chart));
        let _ = writeln!(out, "#METER:{};", meter(chart));
        let _ = writeln!(out, "#CREDIT:{};", chart.metadata.creator);
        let _ = writeln!(out, "#NOTES:");
        write_measures_section(&mut out, chart);
        let _ = writeln!(out, ";");
        Ok(out.into_bytes())
    }
}
