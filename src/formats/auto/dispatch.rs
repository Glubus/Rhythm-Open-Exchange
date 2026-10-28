#![warn(clippy::pedantic)]

use std::path::Path;

use rox::codec::{Decoder, Encoder};
use rox::error::{RoxError, RoxResult};
use rox::model::RoxChart;

use super::detect::{DetectedFormat, detect};
use crate::formats::{
    BmsDecoder, BmsonDecoder, FnfDecoder, FnfEncoder, JroxDecoder, JroxEncoder, McDecoder,
    McEncoder, OjnDecoder, OsuDecoder, OsuEncoder, PmsDecoder, QuaDecoder, QuaEncoder,
    RoxNativeCodec, SmDecoder, SmEncoder, SscDecoder, SscEncoder, YroxDecoder, YroxEncoder,
};
use crate::formats::VSliceDecoder;
use crate::formats::{ChartDecoder, SngPackage};
use crate::formats::SsqDecoder;
use crate::formats::{DwiDecoder, KsfContext, KsfDecoder};

/// Decode a file to a `RoxChart`, detecting the format from the file extension.
///
/// # Errors
///
/// Returns `UnsupportedFormat` if the extension is unknown, or a decode error on failure.
pub fn auto_decode(path: impl AsRef<Path>) -> RoxResult<RoxChart> {
    let path = path.as_ref();
    match detect(path) {
        Ok(DetectedFormat::Edit) => return Err(edit_needs_its_song()),
        Ok(DetectedFormat::Ksf) => return decode_ksf_bytes(path, &std::fs::read(path)?),
        Ok(DetectedFormat::Ssq) => return SsqDecoder::decode_file(path, 0),
        _ => {}
    }
    if detect(path).is_ok_and(|fmt| fmt == DetectedFormat::CloneHero) {
        return ChartDecoder::decode_file(path, 0);
    }
    if let Some(chart) = decode_vslice_file(path, 0)? {
        return Ok(chart);
    }
    let fmt = detect(path)?;
    let data = std::fs::read(path)?;
    decode_bytes(fmt, &data)
}

/// Decode chart `index` of a file: the `index`-th playable chart of a `.sm`/`.ssc` file (the
/// numbering `sm::parse_file` gives), the `index`-th playable difficulty of an `.ojn` file (the numbering
/// of `OjnDecoder::variants`), of a V-Slice chart, a `.fnfc` (default variation), a Clone Hero `.chart` or `.sng` (the numbering of
/// their `variants`), a `.ssq` (the numbering of `SsqDecoder::variants`; the file stem is the title), a `.dwi` (the numbering of
/// `DwiDecoder::variants`), a `.ksf` (one chart: its file name gives the step type and the difficulty, its folder the title and artist
/// `#TITLE` lacks), the only chart of any other format.
/// An `.edit` fails with `InvalidFormat("edit needs its song")`: it holds no timing or metadata, read it with `EditDecoder`.
///
/// # Errors
///
/// Returns an error if the file does not decode or has no such chart.
pub fn auto_decode_variant(path: impl AsRef<Path>, index: usize) -> RoxResult<RoxChart> {
    let path = path.as_ref();
    match detect(path) {
        Ok(DetectedFormat::Edit) => return Err(edit_needs_its_song()),
        Ok(DetectedFormat::Ssq) => return SsqDecoder::decode_file(path, index),
        Ok(DetectedFormat::Dwi) => return DwiDecoder::decode_variant(&std::fs::read(path)?, index),
        Ok(DetectedFormat::Ksf) if index > 0 => {
            return Err(RoxError::InvalidFormat(format!(
                "{} has a single chart, not chart {index}",
                path.display()
            )));
        }
        Ok(DetectedFormat::Ksf) => return decode_ksf_bytes(path, &std::fs::read(path)?),
        _ => {}
    }
    if detect(path).is_ok_and(|fmt| fmt == DetectedFormat::CloneHero) {
        return ChartDecoder::decode_file(path, index);
    }
    if detect(path).is_ok_and(|fmt| fmt == DetectedFormat::Fnfc) {
        return VSliceDecoder::decode_fnfc_variant(&std::fs::read(path)?, "default", index);
    }
    if detect(path).is_ok_and(|fmt| fmt == DetectedFormat::Sng) {
        return SngPackage::open(&std::fs::read(path)?)?.decode_variant(index);
    }
    if let Some(chart) = decode_vslice_file(path, index)? {
        return Ok(chart);
    }
    if detect(path).is_ok_and(|fmt| fmt == DetectedFormat::Ojn) {
        return OjnDecoder::decode_variant(&std::fs::read(path)?, index);
    }
    if !crate::formats::sm::is_stepmania(path) {
        return if index == 0 {
            auto_decode(path)
        } else {
            Err(RoxError::InvalidFormat(format!(
                "{} has a single chart, not chart {index}",
                path.display()
            )))
        };
    }
    let file = crate::formats::sm::parse_file(path)?;
    match file.charts.get(index) {
        Some(chart) => Ok(crate::formats::sm::decoder::from_chart(&file, chart)),
        None if index == 0 => crate::formats::sm::ssc_decoder::first_chart(&file),
        None => Err(RoxError::InvalidFormat(format!(
            "{} has {} playable charts, not chart {index}",
            path.display(),
            file.charts.len()
        ))),
    }
}

/// Why an `.edit` can not be decoded from its path: it holds a chart and a `#SONG` name only, the song's `.sm`/`.ssc` supplies the
/// rest (see `EditDecoder`).
fn edit_needs_its_song() -> RoxError {
    RoxError::InvalidFormat("edit needs its song".to_string())
}

/// A `.ksf` decoded with its file name (the step type and the difficulty are in it) and its folder name (the title and the artist
/// when `#TITLE` gives none).
fn decode_ksf_bytes(path: &Path, data: &[u8]) -> RoxResult<RoxChart> {
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let folder_name = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|n| n.to_str());
    KsfDecoder::decode_with_report(
        data,
        &KsfContext {
            file_name,
            folder_name,
        },
    )
    .map(|(chart, _)| chart)
}

/// The V-Slice difficulty `index` of a `<song>-chart[-<variation>].json` file, read with its `<song>-metadata[-<variation>].json`
/// from the same folder; `None` when `path` is not a V-Slice chart (another name, or a legacy chart that happens to be
/// named like one: it holds a top-level `song`), so every other `.json` is read as before.
fn decode_vslice_file(path: &Path, index: usize) -> RoxResult<Option<RoxChart>> {
    let is_json = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
    let Some((chart, metadata)) = (if is_json { read_vslice_pair(path)? } else { None }) else {
        return Ok(None);
    };
    VSliceDecoder::decode_variant(&chart, &metadata, index).map(Some)
}

/// One difficulty of a file that holds several: its name and what decoding it gave.
#[derive(Debug)]
pub struct FileVariant {
    /// What identifies the difficulty in a message: the `.ojn` code (`EX`), the V-Slice difficulty id (`hard`), the Clone Hero
    /// section (`ExpertSingle`).
    pub name: String,
    pub chart: RoxResult<RoxChart>,
}

/// Every playable difficulty of a file of the formats that hold several, each decoded (a difficulty that fails is its own `Err`,
/// the others are still returned), from one read of the file: an `.ojn`, a V-Slice `<song>-chart[-<variation>].json` (with its
/// metadata file), a `.fnfc` (default variation), a Clone Hero `.chart` (with the `song.ini` beside it), a `.sng`, a `.ssq` or a `.dwi`. The order and
/// the numbers are those of `auto_decode_variant`. `Ok(None)` for any other file (a `.json` that is not a V-Slice chart included),
/// whose charts are read with `auto_decode_variant`.
///
/// # Errors
///
/// Returns an error if the file cannot be read or its list of difficulties cannot be built.
pub fn auto_decode_all(path: impl AsRef<Path>) -> RoxResult<Option<Vec<FileVariant>>> {
    let path = path.as_ref();
    let Ok(fmt) = detect(path) else {
        return Ok(None);
    };
    Ok(Some(match fmt {
        DetectedFormat::Ojn => {
            let data = std::fs::read(path)?;
            OjnDecoder::variants(&data)?
                .iter()
                .map(|v| FileVariant {
                    name: v.name.code().to_owned(),
                    chart: OjnDecoder::decode_variant(&data, v.index),
                })
                .collect()
        }
        DetectedFormat::Fnfc => {
            let data = std::fs::read(path)?;
            VSliceDecoder::fnfc_variants(&data, "default")?
                .iter()
                .map(|v| FileVariant {
                    name: v.difficulty.clone(),
                    chart: VSliceDecoder::decode_fnfc_variant(&data, "default", v.index),
                })
                .collect()
        }
        DetectedFormat::Sng => {
            let data = std::fs::read(path)?;
            let package = SngPackage::open(&data)?;
            package
                .variants()?
                .iter()
                .map(|v| FileVariant {
                    name: v.section.clone(),
                    chart: package.decode_variant(v.index),
                })
                .collect()
        }
        DetectedFormat::CloneHero => {
            let data = std::fs::read(path)?;
            let ini = crate::formats::clone_hero::decoder::read_sibling_ini(path);
            ChartDecoder::variants(&data)?
                .iter()
                .map(|v| FileVariant {
                    name: v.section.clone(),
                    chart: ChartDecoder::decode_with_ini(&data, ini.as_deref(), v.index),
                })
                .collect()
        }
        DetectedFormat::Ssq => {
            let data = std::fs::read(path)?;
            let title = crate::formats::ssq::decoder::title_of(path);
            SsqDecoder::variants(&data)?
                .iter()
                .map(|v| FileVariant {
                    name: v.name(),
                    chart: SsqDecoder::decode_with_title(&data, v.index, &title),
                })
                .collect()
        }
        DetectedFormat::Dwi => {
            let data = std::fs::read(path)?;
            DwiDecoder::variants(&data)?
                .iter()
                .map(|v| FileVariant {
                    name: format!("{} {}", v.stepstype, v.name),
                    chart: DwiDecoder::decode_variant(&data, v.index),
                })
                .collect()
        }
        DetectedFormat::Fnf => match read_vslice_pair(path)? {
            Some((chart, metadata)) => VSliceDecoder::variants(&chart, &metadata)?
                .iter()
                .map(|v| FileVariant {
                    name: v.difficulty.clone(),
                    chart: VSliceDecoder::decode_variant(&chart, &metadata, v.index),
                })
                .collect(),
            None => return Ok(None),
        },
        _ => return Ok(None),
    }))
}

/// The bytes of a V-Slice chart file and of its metadata file; `None` when `path` is not a V-Slice chart (another name, or a
/// legacy chart that happens to be named like one).
fn read_vslice_pair(path: &Path) -> RoxResult<Option<(Vec<u8>, Vec<u8>)>> {
    let Some(metadata_name) = path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(crate::formats::fnf_vslice::vslice_metadata_name)
    else {
        return Ok(None);
    };
    let chart = std::fs::read(path)?;
    if !crate::formats::fnf_vslice::files::looks_like_vslice_chart(&chart) {
        return Ok(None);
    }
    let metadata_path = path.with_file_name(&metadata_name);
    let metadata = std::fs::read(&metadata_path).map_err(|e| {
        RoxError::InvalidFormat(format!(
            "{} is a V-Slice chart: its metadata file {} cannot be read ({e})",
            path.display(),
            metadata_path.display()
        ))
    })?;
    Ok(Some((chart, metadata)))
}

/// Decode in-memory bytes to a `RoxChart`, detecting the format from a path hint
/// (the bytes are not read from disk; only the extension of `path_hint` is used).
///
/// # Errors
///
/// Returns `UnsupportedFormat` if the extension is unknown, or a decode error on failure.
pub fn auto_decode_bytes(path_hint: impl AsRef<Path>, bytes: &[u8]) -> RoxResult<RoxChart> {
    let fmt = detect(path_hint.as_ref())?;
    if fmt == DetectedFormat::Ssq {
        let title = crate::formats::ssq::decoder::title_of(path_hint.as_ref());
        return SsqDecoder::decode_with_title(bytes, 0, &title);
    }
    if fmt == DetectedFormat::Ksf {
        return decode_ksf_bytes(path_hint.as_ref(), bytes);
    }
    decode_bytes(fmt, bytes)
}

/// Encode a `RoxChart` to a file, detecting the format from the file extension.
///
/// # Errors
///
/// Returns `UnsupportedFormat` if the extension is unknown, or an encode error on failure.
pub fn auto_encode(chart: &RoxChart, path: impl AsRef<Path>) -> RoxResult<()> {
    let path = path.as_ref();
    let fmt = detect(path)?;
    let data = encode_bytes(fmt, chart)?;
    std::fs::write(path, data)?;
    Ok(())
}

/// Convert a file from one format to another, detecting both from file extensions.
///
/// # Errors
///
/// Returns `UnsupportedFormat` if either extension is unknown, or a codec error on failure.
pub fn auto_convert(input: impl AsRef<Path>, output: impl AsRef<Path>) -> RoxResult<()> {
    let chart = auto_decode(input)?;
    auto_encode(&chart, output)
}

fn decode_bytes(fmt: DetectedFormat, data: &[u8]) -> RoxResult<RoxChart> {
    match fmt {
        DetectedFormat::Osu => OsuDecoder::decode(data),
        DetectedFormat::Sm => SmDecoder::decode(data),
        DetectedFormat::Ssc => SscDecoder::decode(data),
        DetectedFormat::Qua => QuaDecoder::decode(data),
        DetectedFormat::Fnf => FnfDecoder::decode(data),
        DetectedFormat::Jrox => JroxDecoder::decode(data),
        DetectedFormat::Yrox => YroxDecoder::decode(data),
        DetectedFormat::Rox => RoxNativeCodec::decode(data),
        DetectedFormat::Mc => McDecoder::decode(data),
        DetectedFormat::Bms => BmsDecoder::decode(data),
        DetectedFormat::Pms => PmsDecoder::decode(data),
        DetectedFormat::Bmson => BmsonDecoder::decode(data),
        DetectedFormat::Ojn => OjnDecoder::decode(data),
        DetectedFormat::CloneHero => ChartDecoder::decode(data),
        DetectedFormat::Fnfc => VSliceDecoder::decode_fnfc_variant(data, "default", 0),
        DetectedFormat::Sng => SngPackage::open(data)?.decode_variant(0),
        DetectedFormat::Edit => Err(edit_needs_its_song()),
        DetectedFormat::Ssq => SsqDecoder::decode_variant(data, 0),
        DetectedFormat::Dwi => DwiDecoder::decode(data),
        DetectedFormat::Ksf => KsfDecoder::decode(data),
    }
}

fn encode_bytes(fmt: DetectedFormat, chart: &RoxChart) -> RoxResult<Vec<u8>> {
    match fmt {
        DetectedFormat::Osu => OsuEncoder::encode(chart),
        DetectedFormat::Sm => SmEncoder::encode(chart),
        DetectedFormat::Ssc => SscEncoder::encode(chart),
        DetectedFormat::Qua => QuaEncoder::encode(chart),
        DetectedFormat::Fnf => FnfEncoder::encode(chart),
        DetectedFormat::Jrox => JroxEncoder::encode(chart),
        DetectedFormat::Yrox => YroxEncoder::encode(chart),
        DetectedFormat::Rox => RoxNativeCodec::encode(chart),
        DetectedFormat::Mc => McEncoder::encode(chart),
        DetectedFormat::Bms
        | DetectedFormat::Pms
        | DetectedFormat::Bmson
        | DetectedFormat::Ojn
        | DetectedFormat::CloneHero
        | DetectedFormat::Sng
        | DetectedFormat::Edit
        | DetectedFormat::Ssq
        | DetectedFormat::Dwi
        | DetectedFormat::Ksf
        | DetectedFormat::Fnfc => Err(RoxError::UnsupportedFormat(
            "BMS/PMS/bmson/ojn/fnfc/sng/edit/ssq/dwi/ksf files are read, not written".to_string(),
        )),
    }
}
