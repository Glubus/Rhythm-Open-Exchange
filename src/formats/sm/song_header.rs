#![warn(clippy::pedantic)]
//! The header of a `.sm`/`.ssc` song (its metadata and the timing a chart without its own falls back to), kept apart from
//! its charts: what a reader of charts stored outside the song (the `.edit` files) lays a chart over. A chart read with
//! [`SongHeader::chart_from_tags`] goes through the same pipeline as a chart of the song itself.
#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use rox::error::{RoxError, RoxResult};

use super::msd::{Tag, tokenize};
use super::ssc_parser::{SongTiming, chart, metadata, offset_us};
use super::types::{SmChart, SmFile, SmMetadata};

const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// The song-level tags of a `.sm` or `.ssc` file.
pub struct SongHeader {
    metadata: SmMetadata,
    offset_us: i64,
    timing: SongTiming,
    bpms: Vec<(i64, f32)>,
}

impl SongHeader {
    /// Reads the header of a song file; `extension` is `sm` or `ssc` (any case). In an `.ssc` it is every tag before the first
    /// `#NOTEDATA`, in an `.sm` every tag (a `#NOTES` is not a header tag). The charts are not read.
    ///
    /// # Errors
    /// Returns an error if the data exceeds 100 MB or the extension is neither `sm` nor `ssc`.
    pub fn parse(data: &[u8], extension: &str) -> RoxResult<Self> {
        let ssc = match extension.to_ascii_lowercase().as_str() {
            "sm" => false,
            "ssc" => true,
            other => return Err(RoxError::UnsupportedFormat(other.to_string())),
        };
        if data.len() > MAX_FILE_SIZE {
            return Err(RoxError::InvalidFormat(format!(
                "File too large: {} bytes (max 100MB)",
                data.len()
            )));
        }
        let text = String::from_utf8_lossy(data);
        let tags = tokenize(text.trim_start_matches('\u{feff}'));
        let header = if ssc {
            let end = tags
                .iter()
                .position(|t| t.name == "NOTEDATA")
                .unwrap_or(tags.len());
            &tags[..end]
        } else {
            &tags[..]
        };
        let timing = SongTiming::of(header);
        Ok(Self {
            metadata: metadata(header),
            offset_us: offset_us(header).unwrap_or(0),
            bpms: timing.bpm_points(),
            timing,
        })
    }

    /// The song's metadata (`music` is the `#MUSIC` value, relative to the song's folder).
    #[must_use]
    pub fn metadata(&self) -> &SmMetadata {
        &self.metadata
    }

    /// Replaces the `#MUSIC` value (a caller that looked for the file in the song's folder).
    pub fn set_music(&mut self, music: String) {
        self.metadata.music = music;
    }

    /// The chart a `.ssc`-style list of chart tags (`#STEPSTYPE`, `#DESCRIPTION`, `#DIFFICULTY`, `#METER`, `#CREDIT`,
    /// `#NOTES`, optionally its own `#BPMS`, `#STOPS`, `#DELAYS`, `#WARPS`, `#FAKES`, `#OFFSET`) makes with this song's timing
    /// under it.
    ///
    /// # Errors
    /// Returns why the chart can not be played: a step type this crate does not read, no `#NOTES`, rows of the wrong width, a
    /// timing value it can not honour (see [`super::ssc_parser`]).
    pub fn chart_from_tags(&self, tags: &[Tag]) -> Result<SmChart, String> {
        chart(tags, &self.timing)
    }

    /// A [`SmFile`] of the song with `charts` as its charts (the song's header, offset and BPM changes).
    #[must_use]
    pub fn file_with(&self, charts: Vec<SmChart>, skipped: Vec<String>) -> SmFile {
        SmFile {
            metadata: self.metadata.clone(),
            offset_us: self.offset_us,
            bpms: self.bpms.clone(),
            charts,
            skipped,
        }
    }
}

/// Reads the header of the song file `path` (`.sm` or `.ssc`). Like [`super::parse_file`], a `#MUSIC` that is empty or missing
/// from the folder becomes the first audio file of the folder, when there is one.
///
/// # Errors
/// Returns an error if the file can not be read, is not a `.sm`/`.ssc`, or is too large.
#[cfg(feature = "std")]
pub fn parse_header_file(path: &std::path::Path) -> RoxResult<SongHeader> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    let mut header = SongHeader::parse(&std::fs::read(path)?, extension)?;
    let folder = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let music = &header.metadata.music;
    if (music.is_empty() || !folder.join(music).is_file())
        && let Some(found) = super::first_audio_file(folder)
    {
        header.set_music(found);
    }
    Ok(header)
}
