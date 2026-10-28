#![warn(clippy::pedantic)]
//! `.edit` reader: the chart of the file laid over a song's header, through the `sm` pipeline.

use std::path::Path;

use rox::error::{RoxError, RoxResult};
use rox::model::RoxChart;

use crate::formats::sm::msd::{Tag, tokenize};
use crate::formats::sm::parser::chart_tags;
use crate::formats::sm::song_header::{SongHeader, parse_header_file};
use crate::formats::sm::ssc_decoder::first_chart;
use crate::formats::sm::types::SmFile;

/// Largest `.edit` read (StepMania's `MAX_EDIT_STEPS_SIZE_BYTES`: larger edits are refused as "unreasonably large").
pub const MAX_EDIT_SIZE: usize = 60 * 1024;

/// Reader of `.edit` files: the song they name and the chart they hold laid over that song.
pub struct EditDecoder;
crate::impl_format!(EditDecoder, ["edit"]);

impl EditDecoder {
    /// The song an edit names with `#SONG`: `Group/Song folder` or just `Song folder`, backslashes converted to `/`, safe to
    /// join to a folder (see the module docs). The first `#SONG` of the file counts.
    ///
    /// # Errors
    /// Returns an error if the file is too large, has no `#SONG`, or its value is empty, absolute, has a `.`/`..` part or a
    /// control character, or has more than two parts.
    pub fn song_path(edit: &[u8]) -> RoxResult<String> {
        let tags = read_tags(edit)?;
        let song = tags
            .iter()
            .find(|tag| tag.name == "SONG")
            .ok_or_else(|| RoxError::InvalidFormat("edit has no #SONG".to_string()))?;
        normalise_song(song.first())
            .map_err(|reason| RoxError::InvalidFormat(format!("edit #SONG: {reason}")))
    }

    /// The chart of `edit` laid over the song header, as a [`SmFile`] (the song's header with the one chart in `charts`, or
    /// none and why in `skipped`).
    ///
    /// # Errors
    /// Returns an error if the file is too large or holds no chart at all (no `#NOTES`).
    pub fn parse_for_song(edit: &[u8], song: &SongHeader) -> RoxResult<SmFile> {
        let tags = read_tags(edit)?;
        let chart_tags = select_chart(&tags)
            .ok_or_else(|| RoxError::InvalidFormat("edit has no #NOTES chart".to_string()))?;
        Ok(match song.chart_from_tags(&chart_tags) {
            Ok(mut chart) => {
                // The loader forces the difficulty to Edit whatever the file says.
                chart.difficulty = "Edit".to_string();
                song.file_with(vec![chart], Vec::new())
            }
            Err(reason) => song.file_with(Vec::new(), vec![format!("edit chart: {reason}")]),
        })
    }

    /// [`Self::decode_for_song`] for a song header read once (with [`SongHeader::parse`] or [`parse_header_file`]).
    ///
    /// # Errors
    /// Returns an error if the file is too large, has no chart, or its chart can not be played (the reason is in the error).
    pub fn decode_for_header(edit: &[u8], song: &SongHeader) -> RoxResult<RoxChart> {
        let file = Self::parse_for_song(edit, song)?;
        let chart = first_chart(&file)?;
        chart.validate()?;
        Ok(chart)
    }

    /// The edit's chart on the song whose `.sm`/`.ssc` bytes are `song` (`song_extension` is `sm` or `ssc`). `audio_file` is the
    /// song's `#MUSIC`, relative to the song's folder.
    ///
    /// # Errors
    /// Returns an error if the song is not a `.sm`/`.ssc` or too large, or see [`Self::decode_for_header`].
    pub fn decode_for_song(edit: &[u8], song: &[u8], song_extension: &str) -> RoxResult<RoxChart> {
        Self::decode_for_header(edit, &SongHeader::parse(song, song_extension)?)
    }

    /// The edit's chart on the song file `song_path`, read from disk. Like `sm::parse_file`, an empty `#MUSIC` or one missing
    /// from the song's folder becomes the first audio file of the folder (when there is one); `audio_file` stays relative to
    /// that folder.
    ///
    /// # Errors
    /// Returns an error if the song file can not be read or is not a `.sm`/`.ssc`, or see [`Self::decode_for_header`].
    pub fn decode_for_song_file(edit: &[u8], song_path: &Path) -> RoxResult<RoxChart> {
        Self::decode_for_header(edit, &parse_header_file(song_path)?)
    }
}

/// Tokenizes an edit (UTF-8, BOM and undecodable bytes tolerated like `.ssc`), refusing the files the game refuses.
fn read_tags(edit: &[u8]) -> RoxResult<Vec<Tag>> {
    if edit.len() > MAX_EDIT_SIZE {
        return Err(RoxError::InvalidFormat(format!(
            "edit is unreasonably large: {} bytes (max {MAX_EDIT_SIZE})",
            edit.len()
        )));
    }
    let text = String::from_utf8_lossy(edit);
    Ok(tokenize(text.trim_start_matches('\u{feff}')))
}

/// The tags of the chart the loader keeps: the `.ssc`-style block of the first `#NOTES` that follows a `#NOTEDATA`, or the
/// first `#NOTES` with the six `.sm` parameters (one with fewer is skipped, as the loader does).
fn select_chart(tags: &[Tag]) -> Option<Vec<Tag>> {
    let mut block_start = None;
    for (index, tag) in tags.iter().enumerate() {
        match tag.name.as_str() {
            "NOTEDATA" => block_start = Some(index),
            "NOTES" => {
                if let Some(start) = block_start {
                    return Some(tags[start + 1..=index].to_vec());
                }
                if tag.params.len() >= 6 {
                    return chart_tags(tag);
                }
            }
            _ => {}
        }
    }
    None
}

/// `#SONG` as `SongManager::FindSong` reads it: one or two parts after `\` to `/` and dropping empty ones.
fn normalise_song(value: &str) -> Result<String, String> {
    let value = value.trim().replace('\\', "/");
    if value.starts_with('/') {
        return Err(format!("`{value}` is absolute"));
    }
    let bytes = value.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(format!("`{value}` has a drive"));
    }
    let parts: Vec<&str> = value.split('/').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() {
        return Err("empty".to_string());
    }
    if parts.len() > 2 {
        return Err(format!(
            "`{value}` has {} parts (a song is `Group/Song folder`)",
            parts.len()
        ));
    }
    if let Some(part) = parts
        .iter()
        .find(|part| matches!(**part, "." | "..") || part.chars().any(char::is_control))
    {
        return Err(format!(
            "part `{}` is not a folder name",
            part.escape_debug()
        ));
    }
    Ok(parts.join("/"))
}
