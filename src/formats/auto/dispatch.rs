#![warn(clippy::pedantic)]

use std::borrow::Cow;
use std::path::Path;

use rox::codec::{Decoder, Encoder};
use rox::error::{RoxError, RoxResult};
use rox::model::RoxChart;

use super::detect::{DetectedFormat, detect, detect_content, is_content_candidate};
use crate::formats::{
    BmsDecoder, BmsonDecoder, FnfDecoder, FnfEncoder, JroxDecoder, JroxEncoder, McDecoder,
    McEncoder, OjnDecoder, OsuDecoder, OsuEncoder, PmsDecoder, QuaDecoder, QuaEncoder,
    RoxNativeCodec, SmDecoder, SmEncoder, SscDecoder, SscEncoder, YroxDecoder, YroxEncoder,
};
use crate::formats::VSliceDecoder;
use crate::formats::{ChartDecoder, MidDecoder, SngPackage};
use crate::formats::{SsqDecoder, StepDecoder};
use crate::formats::{DwiDecoder, KsfContext, KsfDecoder, MsdDecoder};
use crate::formats::{SusDecoder, UscDecoder};
use crate::formats::{BestdoriDecoder, SimulatorDecoder};
use crate::formats::ImdDecoder;
use crate::formats::{DtxDecoder, GdaDecoder};
use crate::formats::KshDecoder;
use crate::formats::BdonDecoder;
use crate::formats::XmkDecoder;
use crate::formats::FscDecoder;
use crate::formats::SwfDecoder;
use crate::formats::CodenameDecoder;

/// Decode a file to a `RoxChart`, detecting the format from the file extension. The BanG Dream formats are told by content (a `.txt` that is a
/// BanG! Simulator score, a `.json` that is a Bestdori or BDON chart, whatever its name): any other `.txt` or `.json` is read, or refused, as before.
///
/// # Errors
///
/// Returns `UnsupportedFormat` if the extension is unknown, or a decode error on failure.
pub fn auto_decode(path: impl AsRef<Path>) -> RoxResult<RoxChart> {
    let path = path.as_ref();
    if let Some(data) = guitar_hero_mid(path)? {
        return MidDecoder::decode_guitar_hero(&data, 0, &crate::formats::ssq::decoder::title_of(path));
    }
    let detected = detect(path);
    match detected {
        Ok(DetectedFormat::Edit) => return Err(edit_needs_its_song()),
        Ok(DetectedFormat::Ssq) => return SsqDecoder::decode_file(path, 0),
        Ok(DetectedFormat::Step) => return StepDecoder::decode_file(path, 0),
        Ok(DetectedFormat::CloneHero) => return ChartDecoder::decode_file(path, 0),
        Ok(DetectedFormat::CloneHeroMid) => return MidDecoder::decode_file(path, 0),
        Ok(fmt) => {
            if let Some(decoded) = decode_single_file(fmt, path) {
                return decoded;
            }
        }
        Err(_) => {}
    }
    if let Some(chart) = decode_vslice_file(path, 0)? {
        return Ok(chart);
    }
    if is_content_candidate(path) {
        let data = std::fs::read(path)?;
        if let Some(fmt) = detect_content(path, sniffed(&data)) {
            return decode_named(fmt, path, || Ok(Cow::Borrowed(&data))).unwrap_or_else(|| decode_bytes(fmt, &data));
        }
        // A Codename Engine chart says so after its notes: the whole file is looked at, and the song folder around it read.
        if crate::formats::fnf_codename::looks_like_codename(&data) {
            return crate::formats::fnf_codename::decode_file(path, &data);
        }
        // A Ludum Dare song file names its section bitmaps, read beside it.
        if crate::formats::fnf_ludum_dare::looks_like_ludum_dare(&data) {
            return crate::formats::fnf_ludum_dare::decode_file(path, &data);
        }
    }
    let fmt = detected?;
    decode_bytes(fmt, &std::fs::read(path)?)
}

/// The formats of one chart per file whose decoder reads the file itself (the title, the lanes or the step type come from its name);
/// `None` for the others.
fn decode_single_file(fmt: DetectedFormat, path: &Path) -> Option<RoxResult<RoxChart>> {
    if matches!(fmt, DetectedFormat::Ssq | DetectedFormat::Step) {
        return None; // several charts per file
    }
    decode_named(fmt, path, || Ok(Cow::Owned(std::fs::read(path)?)))
}

/// The formats whose decoder takes something from the file name (`path`): the title (`.sus`, `.usc`, `.ksh`, `.ssq`, BDON and the two BanG
/// Dream formats), the lanes and the difficulty (`.imd`), the step type (`.ksf`). `data` gives the bytes; `None` for the other formats.
fn decode_named<'a>(
    fmt: DetectedFormat,
    path: &Path,
    data: impl FnOnce() -> RoxResult<Cow<'a, [u8]>>,
) -> Option<RoxResult<RoxChart>> {
    let title = || crate::formats::ssq::decoder::title_of(path);
    let bandori_title = || crate::formats::bandori::title_of(path);
    let decode: ChartReader<'_> = match fmt {
        DetectedFormat::Ssq => Box::new(move |data| SsqDecoder::decode_with_title(data, 0, &title())),
        DetectedFormat::Step => Box::new(move |data| StepDecoder::decode_with_title(data, 0, &title())),
        DetectedFormat::Ksf => Box::new(|data| decode_ksf_bytes(path, data)),
        DetectedFormat::Sus => Box::new(move |data| SusDecoder::decode_with_title(data, &title())),
        DetectedFormat::Usc => Box::new(move |data| UscDecoder::decode_with_title(data, &title())),
        DetectedFormat::Ksh => Box::new(move |data| KshDecoder::decode_with_title(data, &title())),
        DetectedFormat::Bdon => Box::new(move |data| BdonDecoder::decode_with_title(data, &title())),
        DetectedFormat::Xmk => Box::new(|data| {
            let (title, difficulty) = crate::formats::djhero::names_of(path);
            XmkDecoder::decode_named(data, &title, &difficulty)
        }),
        DetectedFormat::Imd => Box::new(|data| {
            let file_name = path.file_name().map(|n| n.to_string_lossy());
            ImdDecoder::decode_with_name(data, file_name.as_deref().unwrap_or(""))
        }),
        DetectedFormat::BangSimulator => Box::new(move |data| SimulatorDecoder::decode_with_title(data, &bandori_title())),
        DetectedFormat::Bestdori => Box::new(move |data| BestdoriDecoder::decode_with_title(data, &bandori_title())),
        _ => return None,
    };
    Some(data().and_then(|data| decode(&data)))
}

/// A decoder of one format, given the bytes of a file.
type ChartReader<'p> = Box<dyn Fn(&[u8]) -> RoxResult<RoxChart> + 'p>;

/// Decode chart `index` of a file: the `index`-th playable chart of a `.sm`/`.ssc` file (the
/// numbering `sm::parse_file` gives), the `index`-th playable difficulty of an `.ojn` file (the numbering
/// of `OjnDecoder::variants`), of a `.dtx` (the numbering of `DtxDecoder::variants`: Drums, Guitar, Bass), of a V-Slice chart, a `.fnfc` (default variation), a Clone Hero `.chart`, `notes.mid` or `.sng` (the numbering of
/// their `variants`), a `.ssq` (the numbering of `SsqDecoder::variants`; the file stem is the title), a `.dwi` (the numbering of
/// `DwiDecoder::variants`), a `.ksf` (one chart: its file name gives the step type and the difficulty, its folder the title and artist
/// `#TITLE` lacks), a `.sus` or `.usc` (one chart; the file stem is the title `#TITLE` lacks), an `.imd` (one chart; its file name gives the lanes, the difficulty and the title), a `.ksh` (one chart; the file stem is the title `title=` lacks), a Bestdori chart or a BanG! Simulator `.txt` (one chart;
/// the file stem is the title), the only chart of any other format.
/// An `.edit` fails with `InvalidFormat("edit needs its song")`: it holds no timing or metadata, read it with `EditDecoder`.
///
/// # Errors
///
/// Returns an error if the file does not decode or has no such chart.
pub fn auto_decode_variant(path: impl AsRef<Path>, index: usize) -> RoxResult<RoxChart> {
    let path = path.as_ref();
    let single = || RoxError::InvalidFormat(format!("{} has a single chart, not chart {index}", path.display()));
    let read = || std::fs::read(path);
    if let Some(data) = guitar_hero_mid(path)? {
        return MidDecoder::decode_guitar_hero(&data, index, &crate::formats::ssq::decoder::title_of(path));
    }
    match detect(path) {
        Ok(DetectedFormat::Edit) => return Err(edit_needs_its_song()),
        Ok(DetectedFormat::Ssq) => return SsqDecoder::decode_file(path, index),
        Ok(DetectedFormat::Step) => return StepDecoder::decode_file(path, index),
        Ok(DetectedFormat::Dwi) => return DwiDecoder::decode_variant(&read()?, index),
        Ok(DetectedFormat::Msd) => return MsdDecoder::decode_variant(&read()?, index),
        Ok(DetectedFormat::CloneHero) => return ChartDecoder::decode_file(path, index),
        Ok(DetectedFormat::CloneHeroMid) => return MidDecoder::decode_file(path, index),
        Ok(DetectedFormat::Fnfc) => return VSliceDecoder::decode_fnfc_variant(&read()?, "default", index),
        Ok(DetectedFormat::Sng) => return SngPackage::open(&read()?)?.decode_variant(index),
        Ok(DetectedFormat::Ojn) => return OjnDecoder::decode_variant(&read()?, index),
        Ok(DetectedFormat::Dtx) => return DtxDecoder::decode_variant(&read()?, index),
        Ok(DetectedFormat::Gda) => return GdaDecoder::decode_variant(&read()?, index),
        Ok(fmt) => {
            if let Some(decoded) = decode_single_file(fmt, path) {
                return if index == 0 { decoded } else { Err(single()) };
            }
        }
        Err(_) => {}
    }
    if let Some(chart) = decode_vslice_file(path, index)? {
        return Ok(chart);
    }
    if !crate::formats::sm::is_stepmania(path) {
        return if index == 0 { auto_decode(path) } else { Err(single()) };
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
    /// section (`ExpertSingle`, or `PART GUITAR Expert` for a `notes.mid`).
    pub name: String,
    pub chart: RoxResult<RoxChart>,
}

/// Every playable difficulty of a file of the formats that hold several, each decoded (a difficulty that fails is its own `Err`,
/// the others are still returned), from one read of the file: an `.ojn`, a `.dtx` (Drums, Guitar, Bass), a V-Slice `<song>-chart[-<variation>].json` (with its
/// metadata file), a `.fnfc` (default variation), a Clone Hero `.chart` or `notes.mid` (with the `song.ini` beside it), a `.sng`, a `.ssq` or a `.dwi`. The order and
/// the numbers are those of `auto_decode_variant`. `Ok(None)` for any other file (a `.json` that is not a V-Slice chart included),
/// whose charts are read with `auto_decode_variant`.
///
/// # Errors
///
/// Returns an error if the file cannot be read or its list of difficulties cannot be built.
pub fn auto_decode_all(path: impl AsRef<Path>) -> RoxResult<Option<Vec<FileVariant>>> {
    let path = path.as_ref();
    if let Some(data) = guitar_hero_mid(path)? {
        let title = crate::formats::ssq::decoder::title_of(path);
        return Ok(Some(each(&MidDecoder::guitar_hero_variants(&data)?, |v| v.section.clone(), |v| {
            MidDecoder::decode_guitar_hero(&data, v.index, &title)
        })));
    }
    let Ok(fmt) = detect(path) else {
        return Ok(None);
    };
    let read = || std::fs::read(path);
    let ini = || crate::formats::clone_hero::decoder::read_sibling_ini(path);
    Ok(Some(match fmt {
        DetectedFormat::Ojn => {
            let data = read()?;
            each(&OjnDecoder::variants(&data)?, |v| v.name.code().to_owned(), |v| OjnDecoder::decode_variant(&data, v.index))
        }
        DetectedFormat::Fnfc => {
            let data = read()?;
            each(
                &VSliceDecoder::fnfc_variants(&data, "default")?,
                |v| v.difficulty.clone(),
                |v| VSliceDecoder::decode_fnfc_variant(&data, "default", v.index),
            )
        }
        DetectedFormat::Sng => {
            let data = read()?;
            let package = SngPackage::open(&data)?;
            each(&package.variants()?, |v| v.section.clone(), |v| package.decode_variant(v.index))
        }
        DetectedFormat::CloneHero => {
            let (data, ini) = (read()?, ini());
            each(
                &ChartDecoder::variants(&data)?,
                |v| v.section.clone(),
                |v| ChartDecoder::decode_with_ini(&data, ini.as_deref(), v.index),
            )
        }
        DetectedFormat::CloneHeroMid => {
            let (data, ini) = (read()?, ini());
            each(&MidDecoder::variants(&data)?, |v| v.section.clone(), |v| MidDecoder::decode_with_ini(&data, ini.as_deref(), v.index))
        }
        DetectedFormat::Ssq => {
            let data = read()?;
            let title = crate::formats::ssq::decoder::title_of(path);
            each(&SsqDecoder::variants(&data)?, crate::formats::ssq::Variant::name, |v| {
                SsqDecoder::decode_with_title(&data, v.index, &title)
            })
        }
        DetectedFormat::Step => {
            let data = read()?;
            let title = crate::formats::ssq::decoder::title_of(path);
            each(&StepDecoder::variants(&data)?, crate::formats::ssq::StepVariant::name, |v| {
                StepDecoder::decode_with_title(&data, v.index, &title)
            })
        }
        DetectedFormat::Dtx | DetectedFormat::Gda => {
            let data = read()?;
            let all = if fmt == DetectedFormat::Dtx { DtxDecoder::decode_all(&data)? } else { GdaDecoder::decode_all(&data)? };
            all.into_iter().map(|(variant, chart)| FileVariant { name: variant.part.name().to_owned(), chart }).collect()
        }
        DetectedFormat::Dwi => {
            let data = read()?;
            each(&DwiDecoder::variants(&data)?, |v| format!("{} {}", v.stepstype, v.name), |v| DwiDecoder::decode_variant(&data, v.index))
        }
        DetectedFormat::Msd => {
            let data = read()?;
            each(&MsdDecoder::variants(&data)?, |v| format!("{} {}", v.stepstype, v.name), |v| MsdDecoder::decode_variant(&data, v.index))
        }
        DetectedFormat::Fnf => match read_vslice_pair(path)? {
            Some((chart, metadata)) => each(
                &VSliceDecoder::variants(&chart, &metadata)?,
                |v| v.difficulty.clone(),
                |v| VSliceDecoder::decode_variant(&chart, &metadata, v.index),
            ),
            None => return Ok(None),
        },
        _ => return Ok(None),
    }))
}

/// Every variant of a list, named and decoded.
fn each<V>(
    variants: &[V],
    name: impl Fn(&V) -> String,
    decode: impl Fn(&V) -> RoxResult<RoxChart>,
) -> Vec<FileVariant> {
    variants.iter().map(|v| FileVariant { name: name(v), chart: decode(v) }).collect()
}

/// The bytes of a Guitar Hero 1/2 chart: a `.mid` not named `notes.mid` that holds a 5-lane instrument track; `None` for any other
/// file (a `notes.mid` is Clone Hero's, another `.mid` an ordinary MIDI file).
fn guitar_hero_mid(path: &Path) -> RoxResult<Option<Vec<u8>>> {
    let is_mid = path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("mid"));
    if !is_mid || path.file_name().and_then(|n| n.to_str()).is_some_and(crate::formats::clone_hero::is_mid_chart_name) {
        return Ok(None);
    }
    let data = std::fs::read(path)?;
    Ok(crate::formats::clone_hero::looks_like_guitar_hero_mid(&data).then_some(data))
}

/// The bytes of a V-Slice chart file and of its metadata file; `None` when `path` is not a V-Slice chart (another name, or a
/// legacy or Codename chart that happens to be named like one).
fn read_vslice_pair(path: &Path) -> RoxResult<Option<(Vec<u8>, Vec<u8>)>> {
    let Some(metadata_name) = path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(crate::formats::fnf_vslice::vslice_metadata_name)
    else {
        return Ok(None);
    };
    let chart = std::fs::read(path)?;
    if !crate::formats::fnf_vslice::files::looks_like_vslice_chart(&chart) || crate::formats::fnf_codename::looks_like_codename(&chart) {
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
    let hint = path_hint.as_ref();
    let fmt = match detect_content(hint, sniffed(bytes)) {
        Some(fmt) => fmt,
        None if is_content_candidate(hint) && crate::formats::fnf_codename::looks_like_codename(bytes) => return CodenameDecoder::decode(bytes),
        None => detect(hint)?,
    };
    decode_named(fmt, hint, || Ok(Cow::Borrowed(bytes))).unwrap_or_else(|| decode_bytes(fmt, bytes))
}

/// The first bytes of a file, the ones `detect_content` looks at.
fn sniffed(data: &[u8]) -> &[u8] {
    &data[..data.len().min(crate::formats::sekai::SNIFF_BYTES)]
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
        DetectedFormat::CloneHeroMid => MidDecoder::decode(data),
        DetectedFormat::Fnfc => VSliceDecoder::decode_fnfc_variant(data, "default", 0),
        DetectedFormat::Sng => SngPackage::open(data)?.decode_variant(0),
        DetectedFormat::Edit => Err(edit_needs_its_song()),
        DetectedFormat::Ssq => SsqDecoder::decode_variant(data, 0),
        DetectedFormat::Step => StepDecoder::decode(data),
        DetectedFormat::Dwi => DwiDecoder::decode(data),
        DetectedFormat::Msd => MsdDecoder::decode(data),
        DetectedFormat::Xmk => XmkDecoder::decode(data),
        DetectedFormat::Ksf => KsfDecoder::decode(data),
        DetectedFormat::Sus => SusDecoder::decode(data),
        DetectedFormat::Usc => UscDecoder::decode(data),
        DetectedFormat::Bestdori => BestdoriDecoder::decode(data),
        DetectedFormat::Imd => ImdDecoder::decode(data),
        DetectedFormat::Ksh => KshDecoder::decode(data),
        DetectedFormat::Dtx => DtxDecoder::decode(data),
        DetectedFormat::Gda => GdaDecoder::decode(data),
        DetectedFormat::Fsc => FscDecoder::decode(data),
        DetectedFormat::Swf => SwfDecoder::decode(data),
        DetectedFormat::BangSimulator => SimulatorDecoder::decode(data),
        DetectedFormat::Bdon => BdonDecoder::decode(data),
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
        _ => Err(RoxError::UnsupportedFormat(format!("{fmt:?} files are read, not written"))),
    }
}
