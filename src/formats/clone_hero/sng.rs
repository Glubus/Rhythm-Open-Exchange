#![warn(clippy::pedantic)]
//! `.sng` packages: a whole song folder (chart, stems, images) in one masked container.
//!
//! Layout (little-endian): the 6 bytes `SNGPKG`, a `u32` version (1), 16 mask bytes, then three sections each led by
//! a `u64` length of the bytes after it: metadata (`u64` count, then `i32` length + bytes for each key and value),
//! file index (`u64` count, then per file a one-byte name length, the name, a `u64` length and a `u64` ABSOLUTE offset
//! into the `.sng`) and file data. File byte `i` is stored as `plain[i] ^ mask[i % 16] ^ (i & 0xFF)`.
//!
//! Input is untrusted: every length is checked against the bytes that remain before anything is allocated, a count
//! is checked against the smallest size an item can have, an entry must lie inside the file-data section.

use rox::audio_plan::AudioPlan;
use rox::error::{RoxError, RoxResult};

use super::chart::MAX_TEXT_BYTES;
use super::decoder::{
    Built, ChartReport, Converted, Variant, convert, nth, plan_events, variants_of,
};
use super::ini::SongIni;
use super::plan::Listing;
use crate::formats::bms::bmson::safe_name;
use rox::model::RoxChart;

const MAGIC: [u8; 6] = *b"SNGPKG";
/// The only version of the format.
const VERSION: u32 = 1;
const HEADER: usize = 6 + 4 + 16;
/// Smallest file entry: name length byte, empty name, two `u64`.
const MIN_ENTRY: u64 = 1 + 16;
/// Smallest metadata pair: two `i32` lengths.
const MIN_PAIR: u64 = 8;

/// A file inside a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SngEntry {
    /// Relative path in the song folder, `/` separated.
    pub name: String,
    /// Length of the file, bytes.
    pub len: u64,
    offset: usize,
}

/// An opened `.sng`: header, metadata and file index. The file bytes stay in the package until [`SngPackage::read`].
#[derive(Debug, Clone)]
pub struct SngPackage<'a> {
    data: &'a [u8],
    mask: [u8; 16],
    /// The metadata pairs (the tags of a `song.ini`), in file order.
    pub metadata: Vec<(String, String)>,
    /// The files, in file order: the number of an entry is the `index` of the audio plan's events.
    pub entries: Vec<SngEntry>,
}

fn u64_at(bytes: &[u8], at: usize) -> RoxResult<u64> {
    bytes
        .get(at..at + 8)
        .map(|b| u64::from_le_bytes(b.try_into().expect("8 bytes")))
        .ok_or_else(|| err("cut short"))
}

fn i32_len(bytes: &[u8], at: usize) -> RoxResult<usize> {
    let raw = bytes
        .get(at..at + 4)
        .map(|b| i32::from_le_bytes(b.try_into().expect("4 bytes")))
        .ok_or_else(|| err("cut short"))?;
    usize::try_from(raw).map_err(|_| err("negative length"))
}

fn err(message: &str) -> RoxError {
    RoxError::InvalidFormat(format!("sng: {message}"))
}

/// The bytes of a section starting at `at`: its `u64` length, then that many bytes; and the position after it.
fn section(data: &[u8], at: usize) -> RoxResult<(&[u8], usize)> {
    let len = usize::try_from(u64_at(data, at)?).map_err(|_| err("section too long"))?;
    let start = at + 8;
    let end = start
        .checked_add(len)
        .ok_or_else(|| err("section too long"))?;
    let body = data
        .get(start..end)
        .ok_or_else(|| err("section runs past the end"))?;
    Ok((body, end))
}

impl<'a> SngPackage<'a> {
    /// Opens a package.
    ///
    /// # Errors
    /// Returns an error if the signature or version is wrong, a section or an item runs past its bytes, a file name is
    /// empty, absolute, holds `..` or a control character or `\`, or a file lies outside the file-data section.
    pub fn open(data: &'a [u8]) -> RoxResult<Self> {
        if data.len() < HEADER || data[..6] != MAGIC {
            return Err(err("not a SNGPKG file"));
        }
        let version = u32::from_le_bytes([data[6], data[7], data[8], data[9]]);
        if version != VERSION {
            return Err(err(&format!("version {version} is not supported (only 1)")));
        }
        let mut mask = [0u8; 16];
        mask.copy_from_slice(&data[10..HEADER]);

        let (meta, at) = section(data, HEADER)?;
        let metadata = read_metadata(meta)?;
        let (index, at) = section(data, at)?;
        let raw_entries = read_index(index)?;
        let data_len = u64_at(data, at)?;
        let data_start = at + 8;
        let data_end = usize::try_from(data_len)
            .ok()
            .and_then(|l| data_start.checked_add(l))
            .filter(|&e| e <= data.len())
            .ok_or_else(|| err("file data runs past the end"))?;

        let mut entries = Vec::with_capacity(raw_entries.len());
        for (name, len, offset) in raw_entries {
            let offset = usize::try_from(offset).map_err(|_| err("offset too large"))?;
            let end = usize::try_from(len)
                .ok()
                .and_then(|l| offset.checked_add(l))
                .ok_or_else(|| err("file too long"))?;
            if offset < data_start || end > data_end {
                return Err(err(&format!("`{name}` lies outside the file data")));
            }
            entries.push(SngEntry { name, len, offset });
        }
        Ok(Self {
            data,
            mask,
            metadata,
            entries,
        })
    }

    /// Entry number of the first file named `name` (case-insensitive).
    #[must_use]
    pub fn find(&self, name: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.name.eq_ignore_ascii_case(name))
    }

    /// The unmasked bytes of entry `index`.
    ///
    /// # Errors
    /// Returns an error if there is no such entry.
    pub fn read(&self, index: usize) -> RoxResult<Vec<u8>> {
        let entry = self
            .entries
            .get(index)
            .ok_or_else(|| err(&format!("no entry {index}")))?;
        // `open` checked the range; `len` fits `usize` there.
        let len = usize::try_from(entry.len).map_err(|_| err("file too long"))?;
        let masked = &self.data[entry.offset..entry.offset + len];
        Ok(masked
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ self.mask[i % 16] ^ (i & 0xFF).to_le_bytes()[0])
            .collect())
    }

    /// The metadata as the tags of a `song.ini`.
    #[must_use]
    pub fn ini(&self) -> SongIni {
        SongIni::from_pairs(self.metadata.iter().map(|(k, v)| (k.as_str(), v.as_str())))
    }

    /// The names of the files, for the plan's listing.
    fn listing(&self) -> Listing {
        let mut listing = Listing::default();
        for (i, entry) in self.entries.iter().enumerate() {
            listing.push(&entry.name, u32::try_from(i).ok());
        }
        listing
    }

    /// The chart of the package: `notes.mid` when there is one, else `notes.chart` (the order of the reader of
    /// `YARG.Core`, which takes the first of `notes.mid`, `notes.midi`, `notes.chart`; `notes.midi` is not read here).
    fn converted(&self) -> RoxResult<Converted> {
        if let Some(at) = self.find("notes.mid") {
            if self.entries[at].len > super::smf::MAX_FILE_BYTES as u64 {
                return Err(err("notes.mid is too large"));
            }
            return super::mid::convert(&self.read(at)?, Some(&self.ini()), super::mid::Flavour::CloneHero);
        }
        let Some(at) = self.find("notes.chart") else {
            return Err(err("no notes.chart or notes.mid"));
        };
        if self.entries[at].len > MAX_TEXT_BYTES as u64 {
            return Err(err("notes.chart is too large"));
        }
        convert(&self.read(at)?, Some(&self.ini()))
    }

    /// The playable tracks of `notes.chart` (see `ChartDecoder::variants`), with the metadata of the package as the
    /// `song.ini`.
    ///
    /// # Errors
    /// Returns an error if the package has no `notes.chart` or it does not parse.
    pub fn variants(&self) -> RoxResult<Vec<Variant>> {
        Ok(variants_of(&self.converted()?))
    }

    /// Decodes track `index` of `notes.chart` (the numbering of [`SngPackage::variants`]).
    ///
    /// # Errors
    /// Returns an error if the package has no `notes.chart`, it does not parse or has no such track.
    pub fn decode_variant(&self, index: usize) -> RoxResult<RoxChart> {
        Ok(nth(self.converted()?, index)?.chart)
    }

    /// [`SngPackage::decode_variant`] plus the report and the audio plan: one event per stem entry, `file` = `sng_name`
    /// (the package's own name, relative to the chart's folder, like an `.ojm`) and `index` = the entry number (see
    /// [`SngPackage::read`]). The package holds its files, so the plan has exactly the stems that exist.
    ///
    /// # Errors
    /// Returns an error if the package has no `notes.chart`, it does not parse or has no such track, or `sng_name` is
    /// not a relative path.
    pub fn decode_with_audio_plan(
        &self,
        sng_name: &str,
        index: usize,
    ) -> RoxResult<(RoxChart, ChartReport, AudioPlan)> {
        let sng_name = safe_name(sng_name, &mut 0)
            .ok_or_else(|| err("the package name must be a relative path"))?;
        let converted = self.converted()?;
        let song = converted.song.clone();
        let mut built: Built = nth(converted, index)?;
        let events = plan_events(
            &song,
            Some(&self.listing()),
            Some(&sng_name),
            &mut built.report,
        );
        Ok((built.chart, built.report, AudioPlan::new(events)))
    }
}

/// The pairs of a metadata section (its count, then the pairs).
fn read_metadata(body: &[u8]) -> RoxResult<Vec<(String, String)>> {
    let count = u64_at(body, 0)?;
    if count > (body.len() as u64 - 8) / MIN_PAIR {
        return Err(err("metadata count larger than the section"));
    }
    let mut at = 8;
    let mut out = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
    for _ in 0..count {
        let mut pair = [String::new(), String::new()];
        for slot in &mut pair {
            let len = i32_len(body, at)?;
            at += 4;
            let end = at.checked_add(len).ok_or_else(|| err("string too long"))?;
            let bytes = body.get(at..end).ok_or_else(|| err("metadata cut short"))?;
            *slot = String::from_utf8_lossy(bytes).into_owned();
            at = end;
        }
        let [key, value] = pair;
        out.push((key, value));
    }
    Ok(out)
}

/// `(name, length, absolute offset)` of the file-index section.
fn read_index(body: &[u8]) -> RoxResult<Vec<(String, u64, u64)>> {
    let count = u64_at(body, 0)?;
    if count > (body.len() as u64 - 8) / MIN_ENTRY {
        return Err(err("file count larger than the section"));
    }
    let mut at = 8;
    let mut out = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
    for _ in 0..count {
        let name_len = usize::from(*body.get(at).ok_or_else(|| err("file index cut short"))?);
        at += 1;
        let name_bytes = body
            .get(at..at + name_len)
            .ok_or_else(|| err("file index cut short"))?;
        at += name_len;
        let len = u64_at(body, at)?;
        let offset = u64_at(body, at + 8)?;
        at += 16;
        let name = String::from_utf8_lossy(name_bytes).into_owned();
        if !valid_name(&name) {
            return Err(err(&format!("file name `{name}` is not allowed")));
        }
        out.push((name, len, offset));
    }
    Ok(out)
}

/// A relative path of the song folder: not empty, none of the characters the format forbids (`<>:"/\|?*` except `/` as
/// the separator, control characters), not absolute, no `..`.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name
            .chars()
            .any(|c| c.is_control() || "<>:\"|?*".contains(c))
        && !name.contains("..")
        && !name.split('/').any(str::is_empty)
}
