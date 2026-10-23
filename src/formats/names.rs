//! The second name of a map: its ROMANISED title and artist, when the file carries one that differs from the name the
//! decoders put in `Metadata::title` / `Metadata::artist` (the ORIGINAL one, as the author wrote it).
//!
//! | format          | original (`Metadata`)                | romanised                          |
//! |-----------------|--------------------------------------|------------------------------------|
//! | `.osu`          | `TitleUnicode` / `ArtistUnicode`, else `Title` / `Artist` | `Title` / `Artist` when `*Unicode` differs |
//! | `.sm` / `.ssc`  | `#TITLE` / `#ARTIST`                 | `#TITLETRANSLIT` / `#ARTISTTRANSLIT` when set and different |
//! | `.qua`, `.json` (FNF), `.rox` family | `Title` / `Artist` | none (one name only)               |
//!
//! Only the header of the file is read, never its notes, so a whole library can be looked at cheaply
//! (`romanised_names`). `Metadata` itself is not extended: it is archived by `rkyv` in the native `.rox` files.
use rox::error::{RoxError, RoxResult};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use crate::formats::sm::is_stepmania;
use crate::formats::sm::msd::tokenize;

/// How much of an `.sm`/`.ssc` is read to find its header tags: they come before the first chart.
const SM_HEADER_LIMIT: u64 = 256 * 1024;

/// The romanised variants of a map's title and artist; `None` where the file has no different one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RomanisedNames {
    pub title: Option<String>,
    pub artist: Option<String>,
}

/// `romanised` when it is a non-empty name other than `original`.
fn differing(romanised: &str, original: &str) -> Option<String> {
    let romanised = romanised.trim();
    (!romanised.is_empty() && romanised != original).then(|| romanised.to_string())
}

/// The romanised names of the map file at `path`, by its extension.
///
/// # Errors
/// Returns an error if the file can not be read.
pub fn romanised_names(path: &Path) -> RoxResult<RomanisedNames> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension == "osu" {
        return osu_names(std::fs::File::open(path)?);
    }
    if is_stepmania(path) {
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(SM_HEADER_LIMIT)
            .read_to_end(&mut bytes)?;
        return Ok(stepmania_names(&String::from_utf8_lossy(&bytes)));
    }
    Ok(RomanisedNames::default())
}

/// [`romanised_names`] for a file whose bytes the caller already read (`path` gives the extension): a scan reads each file
/// once, so the names come from the same bytes as the chart.
///
/// # Errors
/// Returns an error if an osu! header is not UTF-8.
pub fn romanised_names_from_bytes(path: &Path, data: &[u8]) -> RoxResult<RomanisedNames> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension == "osu" {
        return osu_names(data);
    }
    if is_stepmania(path) {
        let head = &data[..data
            .len()
            .min(usize::try_from(SM_HEADER_LIMIT).unwrap_or(usize::MAX))];
        return Ok(stepmania_names(&String::from_utf8_lossy(head)));
    }
    Ok(RomanisedNames::default())
}

/// `[Metadata]` of an osu! file, stopping at the first section that follows it.
fn osu_names(file: impl Read) -> RoxResult<RomanisedNames> {
    let (mut title, mut title_unicode, mut artist, mut artist_unicode) =
        (String::new(), None, String::new(), None);
    let mut in_metadata = false;
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|error| RoxError::InvalidFormat(error.to_string()))?;
        let line = line.trim_start_matches('\u{feff}').trim();
        if line.starts_with('[') {
            if in_metadata {
                break;
            }
            in_metadata = line == "[Metadata]";
            continue;
        }
        if !in_metadata {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Title" => title = value.to_string(),
            "TitleUnicode" => title_unicode = Some(value.to_string()),
            "Artist" => artist = value.to_string(),
            "ArtistUnicode" => artist_unicode = Some(value.to_string()),
            _ => {}
        }
    }
    Ok(RomanisedNames {
        title: differing(&title, title_unicode.as_deref().unwrap_or(&title)),
        artist: differing(&artist, artist_unicode.as_deref().unwrap_or(&artist)),
    })
}

/// The header tags of a `.sm`/`.ssc` text (the tokenizer is the one the chart parsers use, so escapes and comments agree).
#[must_use]
pub fn stepmania_names(content: &str) -> RomanisedNames {
    let header_end = ["#NOTEDATA", "#NOTES"]
        .iter()
        .filter_map(|marker| content.find(marker))
        .min()
        .unwrap_or(content.len());
    let tags = tokenize(&content[..header_end]);
    let text = |name: &str| {
        tags.iter()
            .rev()
            .find(|tag| tag.name == name)
            .map(super::sm::msd::Tag::text)
            .unwrap_or_default()
    };
    RomanisedNames {
        title: differing(&text("TITLETRANSLIT"), &text("TITLE")),
        artist: differing(&text("ARTISTTRANSLIT"), &text("ARTIST")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn osu(metadata: &str) -> RomanisedNames {
        osu_names(format!("osu file format v14\n\n[General]\nAudioFilename: a.mp3\n\n[Metadata]\n{metadata}\n[Difficulty]\nTitle:not me\n").as_bytes()).unwrap()
    }

    #[test]
    fn osu_keeps_the_ascii_title_as_the_romanised_one_when_a_unicode_one_exists() {
        let names = osu(
            "Title:Rokuchounen to Ichiya Monogatari\nTitleUnicode:六兆年と一夜物語\nArtist:kemu\nArtistUnicode:ケム\nVersion:4K\n",
        );
        assert_eq!(
            names,
            RomanisedNames {
                title: Some("Rokuchounen to Ichiya Monogatari".into()),
                artist: Some("kemu".into())
            }
        );
    }

    #[test]
    fn osu_without_a_second_or_with_an_identical_one_has_no_romanised_name() {
        assert_eq!(
            osu("Title:Plain\nArtist:Someone\n"),
            RomanisedNames::default()
        );
        assert_eq!(
            osu("Title:Same\nTitleUnicode:Same\nArtist:A\nArtistUnicode:A\n"),
            RomanisedNames::default()
        );
    }

    #[test]
    fn osu_titles_and_artists_are_independent_and_an_empty_unicode_one_does_not_hide_the_ascii_one()
    {
        let names = osu("Title:Only title\nTitleUnicode:\nArtist:Same\nArtistUnicode:Same\n");
        assert_eq!(
            names,
            RomanisedNames {
                title: Some("Only title".into()),
                artist: None
            }
        );
    }

    #[test]
    fn osu_ignores_keys_outside_metadata_and_a_byte_order_mark() {
        let text = "\u{feff}osu file format v14\n[Metadata]\nTitle:Latin\nTitleUnicode:ラテン\n";
        assert_eq!(
            osu_names(text.as_bytes()).unwrap().title.as_deref(),
            Some("Latin")
        );
    }

    #[test]
    fn stepmania_uses_the_translit_tags_and_stops_at_the_first_chart() {
        let sm = "#TITLE:千本桜;\n#TITLETRANSLIT:Senbonzakura;\n#ARTIST:黒うさP;\n#ARTISTTRANSLIT:;\n#NOTES:\n dance-single:\n :\n Easy:\n 1:\n 0,0,0,0,0:\n0000\n;\n#TITLETRANSLIT:late;\n";
        assert_eq!(
            stepmania_names(sm),
            RomanisedNames {
                title: Some("Senbonzakura".into()),
                artist: None
            }
        );
        let ssc = "#VERSION:0.83;\n#TITLE:Song;\n#TITLETRANSLIT:Song;\n#ARTIST:A;\n#ARTISTTRANSLIT:Aa;\n#NOTEDATA:;\n#STEPSTYPE:dance-single;\n";
        assert_eq!(
            stepmania_names(ssc),
            RomanisedNames {
                title: None,
                artist: Some("Aa".into())
            }
        );
    }

    #[test]
    fn single_name_formats_and_unknown_files_have_no_romanised_name() {
        let folder = std::env::temp_dir().join(format!("rox-names-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let qua = folder.join("song.qua");
        std::fs::write(&qua, "Title: 曲\nArtist: 人\n").unwrap();
        assert_eq!(romanised_names(&qua).unwrap(), RomanisedNames::default());
        let osu_file = folder.join("song.osu");
        std::fs::write(
            &osu_file,
            "[Metadata]\nTitle:A\nTitleUnicode:あ\nArtist:B\n",
        )
        .unwrap();
        assert_eq!(
            romanised_names(&osu_file).unwrap().title.as_deref(),
            Some("A")
        );
        let sm_file = folder.join("song.SM");
        std::fs::write(&sm_file, "#TITLE:あ;#TITLETRANSLIT:A;").unwrap();
        assert_eq!(
            romanised_names(&sm_file).unwrap().title.as_deref(),
            Some("A")
        );
        assert!(romanised_names(&folder.join("missing.osu")).is_err());
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn names_from_bytes_are_the_names_from_the_file() {
        let osu_text = "osu file format v14\n[Metadata]\nTitle:A\nTitleUnicode:あ\nArtist:B\nArtistUnicode:B\n[Events]\n";
        assert_eq!(
            romanised_names_from_bytes(Path::new("x/song.OSU"), osu_text.as_bytes()).unwrap(),
            RomanisedNames {
                title: Some("A".into()),
                artist: None
            }
        );
        let sm_text = "#TITLE:あ;#TITLETRANSLIT:A;";
        assert_eq!(
            romanised_names_from_bytes(Path::new("song.ssc"), sm_text.as_bytes())
                .unwrap()
                .title
                .as_deref(),
            Some("A")
        );
        assert_eq!(
            romanised_names_from_bytes(Path::new("song.qua"), b"Title: x").unwrap(),
            RomanisedNames::default()
        );
    }
}
