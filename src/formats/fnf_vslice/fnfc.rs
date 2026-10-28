#![warn(clippy::pedantic)]
//! The `.fnfc` package: a ZIP with `manifest.json`, the chart and metadata JSON files and the `.ogg` stems at its root.
//! Nothing is extracted to disk; entries are read into memory, with hard limits.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use rox::audio_plan::AudioPlan;
use rox::error::RoxResult;
use rox::model::RoxChart;
use serde::Deserialize;
use zip::ZipArchive;

use super::audio::Stems;
use super::data::{MAX_JSON_BYTES, bad, parse_json};
use super::decoder::{VSliceDecoder, VSliceReport, Variant, plan_nth};
use super::files::{is_vslice_chart_name, suffix};
use crate::formats::bms::bmson::safe_name;

/// A package with more entries than this is refused (a song has a dozen).
pub const MAX_ENTRIES: usize = 1024;
/// A package whose entries declare more than this many bytes once unpacked is refused (stems are tens of MiB).
pub const MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// An entry larger than this cannot be read with [`FnfcPackage::read_entry`]. JSON entries are held to the smaller JSON limit.
pub const MAX_ENTRY_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    song_id: Option<String>,
}

/// An opened `.fnfc` package (see `mod.rs`).
pub struct FnfcPackage {
    archive: ZipArchive<Cursor<Vec<u8>>>,
    song_id: String,
    names: Vec<String>,
    by_name: HashMap<String, u32>,
}

fn zip_error(error: &zip::result::ZipError) -> rox::error::RoxError {
    bad(format!("fnfc is not a readable ZIP: {error}"))
}

/// Unpacks one entry into memory, never more than `limit` bytes (the declared size is not trusted).
fn read_limited(
    archive: &mut ZipArchive<Cursor<Vec<u8>>>,
    index: usize,
    limit: u64,
    what: &str,
) -> RoxResult<Vec<u8>> {
    let file = archive.by_index(index).map_err(|e| zip_error(&e))?;
    if file.size() > limit {
        return Err(bad(format!(
            "{what} is {} bytes unpacked, more than the {limit} allowed",
            file.size()
        )));
    }
    let mut out = Vec::with_capacity(usize::try_from(file.size()).unwrap_or(0));
    file.take(limit + 1)
        .read_to_end(&mut out)
        .map_err(|e| bad(format!("{what} cannot be unpacked: {e}")))?;
    if out.len() as u64 > limit {
        return Err(bad(format!("{what} unpacks to more than {limit} bytes")));
    }
    Ok(out)
}

impl FnfcPackage {
    /// Opens a package from its bytes.
    ///
    /// # Errors
    /// Returns an error if the bytes are not a ZIP, hold more than [`MAX_ENTRIES`] entries or declare more than
    /// [`MAX_TOTAL_BYTES`] unpacked, have an entry name that is absolute, holds `..` or NUL (or a repeated name), or lack a
    /// readable `manifest.json` with a `songId` that is a plain file-name part.
    pub fn open(bytes: impl Into<Vec<u8>>) -> RoxResult<Self> {
        let mut archive = ZipArchive::new(Cursor::new(bytes.into())).map_err(|e| zip_error(&e))?;
        if archive.len() > MAX_ENTRIES {
            return Err(bad(format!(
                "fnfc has {} entries, more than the {MAX_ENTRIES} allowed",
                archive.len()
            )));
        }
        let mut names = Vec::with_capacity(archive.len());
        let mut by_name = HashMap::with_capacity(archive.len());
        let mut total: u64 = 0;
        for index in 0..archive.len() {
            let entry = archive.by_index_raw(index).map_err(|e| zip_error(&e))?;
            total = total.saturating_add(entry.size());
            let name = entry.name().to_string();
            let mut refused = 0;
            if safe_name(&name, &mut refused).is_none() || name.contains('\\') {
                return Err(bad(format!(
                    "fnfc entry {index} has an unsafe name {name:?}"
                )));
            }
            let slot = u32::try_from(index).map_err(|_| bad("fnfc has too many entries"))?;
            if by_name.insert(name.clone(), slot).is_some() {
                return Err(bad(format!("fnfc has two entries named {name:?}")));
            }
            names.push(name);
        }
        if total > MAX_TOTAL_BYTES {
            return Err(bad(format!(
                "fnfc unpacks to {total} bytes, more than the {MAX_TOTAL_BYTES} allowed"
            )));
        }
        let mut package = Self {
            archive,
            song_id: String::new(),
            names,
            by_name,
        };
        let manifest = package.read_json_entry("manifest.json")?;
        let manifest: Manifest = parse_json(&manifest, "manifest.json")?;
        let song_id = manifest.song_id.unwrap_or_default();
        if song_id.is_empty() || song_id.contains(['/', '\\', '\0']) || song_id == ".." {
            return Err(bad(format!(
                "manifest.json songId {song_id:?} is not a plain file-name part"
            )));
        }
        package.song_id = song_id;
        Ok(package)
    }

    /// `songId` of `manifest.json`: the prefix of the chart and metadata file names.
    #[must_use]
    pub fn song_id(&self) -> &str {
        &self.song_id
    }

    /// The entry names, in archive order. A stem's `SoundEvent::index` is its position here.
    #[must_use]
    pub fn entries(&self) -> &[String] {
        &self.names
    }

    /// The position in [`Self::entries`] of the entry called exactly `name` (case-sensitive, as the game's file names are).
    #[must_use]
    pub fn entry_index(&self, name: &str) -> Option<u32> {
        self.by_name.get(name).copied()
    }

    /// The variations that have a chart file in the package (`"default"` first when present, then by name).
    #[must_use]
    pub fn variations(&self) -> Vec<String> {
        let mut found: Vec<String> = self
            .names
            .iter()
            .filter_map(|name| {
                let variation = is_vslice_chart_name(name)?;
                let wanted = if variation == "default" {
                    format!("{}-chart.json", self.song_id)
                } else {
                    format!("{}-chart-{variation}.json", self.song_id)
                };
                (*name == wanted).then(|| variation.to_string())
            })
            .collect();
        found.sort_by(|a, b| {
            (a != "default")
                .cmp(&(b != "default"))
                .then_with(|| a.cmp(b))
        });
        found
    }

    fn read_json_entry(&mut self, name: &str) -> RoxResult<Vec<u8>> {
        let index = self
            .entry_index(name)
            .ok_or_else(|| bad(format!("fnfc has no {name}")))?;
        read_limited(
            &mut self.archive,
            index as usize,
            MAX_JSON_BYTES as u64,
            name,
        )
    }

    /// `<songId>-chart[-<variation>].json`, unpacked.
    ///
    /// # Errors
    /// Returns an error if the file is missing or too large.
    pub fn chart(&mut self, variation: &str) -> RoxResult<Vec<u8>> {
        let name = format!("{}-chart{}.json", self.song_id, suffix(variation));
        self.read_json_entry(&name)
    }

    /// `<songId>-metadata[-<variation>].json`, unpacked.
    ///
    /// # Errors
    /// Returns an error if the file is missing or too large.
    pub fn metadata(&mut self, variation: &str) -> RoxResult<Vec<u8>> {
        let name = format!("{}-metadata{}.json", self.song_id, suffix(variation));
        self.read_json_entry(&name)
    }

    /// Entry `index` (a `SoundEvent::index` of the plan), unpacked, at most [`MAX_ENTRY_BYTES`].
    ///
    /// # Errors
    /// Returns an error if there is no such entry, it is a directory, or it is larger than the limit.
    pub fn read_entry(&mut self, index: usize) -> RoxResult<Vec<u8>> {
        let name = self
            .names
            .get(index)
            .ok_or_else(|| bad(format!("fnfc has no entry {index}")))?
            .clone();
        read_limited(&mut self.archive, index, MAX_ENTRY_BYTES, &name)
    }
}

/// `.fnfc` entry points: the same API as [`VSliceDecoder`], on the bytes of a package and a variation.
impl VSliceDecoder {
    /// The playable difficulties of `variation` (`"default"` or `""` for the default one) of a package.
    ///
    /// # Errors
    /// Returns an error if the package, or its chart or metadata of that variation, cannot be read.
    pub fn fnfc_variants(bytes: &[u8], variation: &str) -> RoxResult<Vec<Variant>> {
        let mut package = FnfcPackage::open(bytes)?;
        let (chart, metadata) = (package.chart(variation)?, package.metadata(variation)?);
        Self::variants(&chart, &metadata)
    }

    /// [`Self::decode_variant`] on a package.
    ///
    /// # Errors
    /// As [`Self::fnfc_variants`], or there is no such playable difficulty.
    pub fn decode_fnfc_variant(bytes: &[u8], variation: &str, index: usize) -> RoxResult<RoxChart> {
        Self::decode_fnfc_with_report(bytes, variation, index).map(|(chart, _)| chart)
    }

    /// [`Self::decode_with_report`] on a package.
    ///
    /// # Errors
    /// As [`Self::decode_fnfc_variant`].
    pub fn decode_fnfc_with_report(
        bytes: &[u8],
        variation: &str,
        index: usize,
    ) -> RoxResult<(RoxChart, VSliceReport)> {
        let mut package = FnfcPackage::open(bytes)?;
        let (chart, metadata) = (package.chart(variation)?, package.metadata(variation)?);
        Self::decode_with_report(&chart, &metadata, index)
    }

    /// [`Self::decode_with_audio_plan_in`] on a package, the files being its entries: every event is a
    /// `SoundEvent::in_container` on `package_file` (the package's own file name, relative and safe) with `index` the entry
    /// number in archive order ([`FnfcPackage::entries`], [`FnfcPackage::read_entry`]). The vocals are resolved as the game does.
    ///
    /// # Errors
    /// As [`Self::decode_fnfc_variant`]; also if `package_file` is unsafe or the package has no instrumental
    /// (`Inst.ogg` or `Inst-<id>.ogg`).
    pub fn decode_fnfc_with_audio_plan(
        bytes: &[u8],
        package_file: &str,
        variation: &str,
        index: usize,
    ) -> RoxResult<(RoxChart, VSliceReport, AudioPlan)> {
        let mut refused = 0;
        let Some(package_file) = safe_name(package_file, &mut refused) else {
            return Err(bad(format!("package file name {package_file:?} is unsafe")));
        };
        let mut package = FnfcPackage::open(bytes)?;
        let (chart, metadata) = (package.chart(variation)?, package.metadata(variation)?);
        let lookup = |name: &str| package.entry_index(name);
        plan_nth(
            &chart,
            &metadata,
            index,
            variation,
            Some(&Stems {
                lookup: &lookup,
                package: Some(&package_file),
            }),
        )
    }
}
