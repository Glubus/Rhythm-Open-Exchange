use std::path::Path;

use crate::error::RoxResult;
use crate::model::RoxChart;

#[cfg(feature = "compression")]
use super::super::formats::RoxCodec;
use super::super::formats::{
    FnfEncoder, JroxEncoder, OsuEncoder, QuaEncoder, SmEncoder, YroxEncoder,
};
use super::super::Encoder;
use super::decode::auto_decode;
use super::types::OutputFormat;

/// Encode a chart to a file, auto-detecting the format from the extension.
///
/// # Example
/// ```ignore
/// use rox::codec::auto_encode;
///
/// auto_encode(&chart, "output.osu")?;  // Encodes as .osu
/// auto_encode(&chart, "output.sm")?;   // Encodes as .sm
/// auto_encode(&chart, "output.rox")?;  // Encodes as .rox
/// ```
///
/// # Errors
///
/// Returns an error if encoding fails or the extension is not recognized.
pub fn auto_encode(chart: &RoxChart, path: impl AsRef<Path>) -> RoxResult<()> {
    let path = path.as_ref();
    let format = OutputFormat::from_path(path)?;

    let data = match format {
        #[cfg(feature = "compression")]
        OutputFormat::Rox => RoxCodec::encode(chart)?,
        OutputFormat::Jrox => JroxEncoder::encode(chart)?,
        OutputFormat::Yrox => YroxEncoder::encode(chart)?,
        OutputFormat::Osu => OsuEncoder::encode(chart)?,
        OutputFormat::Sm => SmEncoder::encode(chart)?,
        OutputFormat::Qua => QuaEncoder::encode(chart)?,
        OutputFormat::Fnf => FnfEncoder::encode(chart)?,
    };

    std::fs::write(path, data)?;
    Ok(())
}

/// Encode a chart to bytes with a specific format.
///
/// # Errors
///
/// Returns an error if encoding fails.
pub fn encode_with_format(chart: &RoxChart, format: OutputFormat) -> RoxResult<Vec<u8>> {
    match format {
        #[cfg(feature = "compression")]
        OutputFormat::Rox => RoxCodec::encode(chart),
        OutputFormat::Jrox => JroxEncoder::encode(chart),
        OutputFormat::Yrox => YroxEncoder::encode(chart),
        OutputFormat::Osu => OsuEncoder::encode(chart),
        OutputFormat::Sm => SmEncoder::encode(chart),
        OutputFormat::Qua => QuaEncoder::encode(chart),
        OutputFormat::Fnf => FnfEncoder::encode(chart),
    }
}

/// Convert a file from one format to another, auto-detecting both formats.
///
/// # Example
/// ```ignore
/// use rox::codec::auto_convert;
///
/// auto_convert("chart.osu", "chart.sm")?;   // osu → sm
/// auto_convert("chart.sm", "chart.rox")?;   // sm → rox
/// auto_convert("chart.rox", "chart.osu")?;  // rox → osu
/// ```
///
/// # Errors
///
/// Returns an error if conversion fails or extensions are not recognized.
pub fn auto_convert(input: impl AsRef<Path>, output: impl AsRef<Path>) -> RoxResult<()> {
    let chart = auto_decode(input)?;
    auto_encode(&chart, output)
}
