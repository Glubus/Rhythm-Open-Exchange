#![warn(clippy::pedantic)]
//! `song.ini`: the metadata file beside a `.chart`, and the same tags inside a `.sng`.

use std::collections::BTreeMap;

use super::chart::{MAX_TEXT_BYTES, decode_text};
use rox::error::{RoxError, RoxResult};

/// Formatting tags (Unity / `TextMeshPro` rich text) that are removed from text values.
const RICH_TAGS: &[&str] = &[
    "align",
    "allcaps",
    "alpha",
    "b",
    "br",
    "color",
    "cspace",
    "font",
    "font-weight",
    "gradient",
    "i",
    "indent",
    "line-height",
    "line-indent",
    "link",
    "lowercase",
    "margin",
    "mark",
    "mspace",
    "noparse",
    "nobr",
    "page",
    "pos",
    "rotate",
    "size",
    "smallcaps",
    "space",
    "sprite",
    "s",
    "style",
    "sub",
    "sup",
    "u",
    "uppercase",
    "voffset",
    "width",
];

/// The tags of a `song.ini` that ROX reads. Every value is `None` when the tag is absent or empty.
///
/// Keys are matched without regard to case; the value is what follows the first `=`, trimmed. Text values lose their
/// formatting tags (`<color=#00FF00>`). A number that does not parse is left out.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SongIni {
    pub name: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<String>,
    /// `charter` (`frets` is a duplicate of it and is ignored).
    pub charter: Option<String>,
    /// `delay`, ms: delays the chart relative to the audio (higher = later notes), may be negative.
    pub delay_ms: Option<i64>,
    /// `song_length`, ms (metadata only).
    pub song_length_ms: Option<i64>,
    pub preview_start_time_ms: Option<i64>,
    pub preview_end_time_ms: Option<i64>,
    /// `sustain_cutoff_threshold`, ticks.
    pub sustain_cutoff_threshold: Option<i64>,
    /// `background`: a file name.
    pub background: Option<String>,
    /// Every `diff_*` tag as written (lowercase key), `-1` meaning "no value" is kept; use [`SongIni::difficulty`].
    pub diffs: BTreeMap<String, i32>,
}

impl SongIni {
    /// Parses the bytes of a `song.ini`: UTF-8 when valid, else Latin-1.
    ///
    /// Only the `[song]` section is read (the header is matched without regard to case); lines before any header are
    /// read as if they were in it. A line starting with `;` or `#` is a comment (only whole-line comments exist).
    /// When a tag appears twice the last one wins.
    ///
    /// # Errors
    /// Returns an error if the file is larger than 64 MiB.
    pub fn parse(bytes: &[u8]) -> RoxResult<Self> {
        if bytes.len() > MAX_TEXT_BYTES {
            return Err(RoxError::InvalidFormat(format!(
                "song.ini: {} bytes, more than {MAX_TEXT_BYTES}",
                bytes.len()
            )));
        }
        let text = decode_text(bytes);
        let mut pairs: Vec<(String, String)> = Vec::new();
        let mut in_song = true;
        for raw in text.split(['\r', '\n']) {
            let line = raw.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                in_song = line[1..line.len() - 1].trim().eq_ignore_ascii_case("song");
            } else if in_song && let Some((key, value)) = line.split_once('=') {
                pairs.push((key.trim().to_owned(), value.trim().to_owned()));
            }
        }
        Ok(Self::from_pairs(
            pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())),
        ))
    }

    /// The tags of key/value pairs (the metadata of a `.sng` holds the same tags). Same rules as [`SongIni::parse`].
    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut ini = Self::default();
        for (key, value) in pairs {
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match key.as_str() {
                "name" => ini.name = text(value),
                "artist" => ini.artist = text(value),
                "album" => ini.album = text(value),
                "genre" => ini.genre = text(value),
                "year" => ini.year = text(value),
                "charter" => ini.charter = text(value),
                "background" => ini.background = text(value),
                "delay" => ini.delay_ms = value.parse().ok(),
                "song_length" => ini.song_length_ms = value.parse().ok(),
                "preview_start_time" => ini.preview_start_time_ms = value.parse().ok(),
                "preview_end_time" => ini.preview_end_time_ms = value.parse().ok(),
                "sustain_cutoff_threshold" => ini.sustain_cutoff_threshold = value.parse().ok(),
                k if k.starts_with("diff_") => {
                    if let Ok(n) = value.parse::<i32>() {
                        ini.diffs.insert(key, n);
                    }
                }
                _ => {}
            }
        }
        ini
    }

    /// The `diff_*` tag `tag` (for example `diff_guitar`), `None` when absent or `-1` (unset).
    #[must_use]
    pub fn difficulty(&self, tag: &str) -> Option<i32> {
        self.diffs.get(tag).copied().filter(|&n| n >= 0)
    }
}

/// A text value without formatting tags, `None` when nothing is left.
fn text(value: &str) -> Option<String> {
    let stripped = strip_rich_tags(value);
    let stripped = stripped.trim();
    (!stripped.is_empty()).then(|| stripped.to_owned())
}

/// `value` without `<tag>`, `<tag=...>` and `</tag>` for the tags of [`RICH_TAGS`]; any other `<` stays.
fn strip_rich_tags(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('>') else {
            rest = &rest[open..];
            break;
        };
        let inner = after[..close].trim_start_matches('/');
        let name = inner
            .split(|c: char| c == '=' || c.is_whitespace())
            .next()
            .unwrap_or("");
        if RICH_TAGS.iter().any(|t| t.eq_ignore_ascii_case(name)) {
            rest = &after[close + 1..];
        } else {
            out.push('<');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}
