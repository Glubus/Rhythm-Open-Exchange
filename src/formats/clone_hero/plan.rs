#![warn(clippy::pedantic)]
//! The audio plan of a Clone Hero chart: the stems of the song, each a whole file at time 0.
//!
//! Without a listing of the files beside the chart, the plan names every reserved stem (`song.ogg`, `guitar.ogg`, ...)
//! as an OPTIONAL event plus the files the `[Song]` stream tags name; the consumer skips what it does not find. With a
//! listing (the names of the folder, or the entries of a `.sng`) the plan is exact: only files that exist, chosen with
//! the rules of the format notes.

use rox::audio_plan::SoundEvent;

use crate::formats::bms::bmson::safe_name;

/// Audio extensions of the format notes, in order of preference.
const EXTENSIONS: [&str; 4] = ["opus", "ogg", "mp3", "wav"];

/// A file that exists beside the chart, or inside the `.sng`.
#[derive(Debug, Clone)]
pub struct Listed {
    lower: String,
    name: String,
    /// Entry number inside a `.sng`; `None` for a plain file.
    index: Option<u32>,
}

/// The files that exist.
#[derive(Debug, Clone, Default)]
pub struct Listing {
    files: Vec<Listed>,
}

impl Listing {
    /// Plain files of a folder (names relative to the chart's folder).
    #[must_use]
    pub fn from_names<S: AsRef<str>>(names: &[S]) -> Self {
        let mut out = Self::default();
        for name in names {
            out.push(name.as_ref(), None);
        }
        out
    }

    pub(super) fn push(&mut self, name: &str, index: Option<u32>) {
        let name = name.replace('\\', "/");
        self.files.push(Listed {
            lower: name.to_lowercase(),
            name,
            index,
        });
    }

    /// First listed file named `name` (case-insensitive).
    fn find(&self, name: &str) -> Option<&Listed> {
        let lower = name.to_lowercase();
        self.files.iter().find(|f| f.lower == lower)
    }

    /// `base` with the first extension of [`EXTENSIONS`] that exists.
    fn find_stem(&self, base: &str) -> Option<&Listed> {
        EXTENSIONS
            .iter()
            .find_map(|ext| self.find(&format!("{base}.{ext}")))
    }
}

/// The reserved stems that are not split (`drums` and `vocals` are in groups below).
const PLAIN: [(&str, &str); 6] = [
    ("song", "MusicStream"),
    ("guitar", "GuitarStream"),
    ("rhythm", "RhythmStream"),
    ("bass", "BassStream"),
    ("keys", "KeysStream"),
    ("crowd", "CrowdStream"),
];
const DRUMS_NUMBERED: [(&str, &str); 4] = [
    ("drums_1", "DrumStream"),
    ("drums_2", "Drum2Stream"),
    ("drums_3", "Drum3Stream"),
    ("drums_4", "Drum4Stream"),
];
const VOCALS_NUMBERED: [&str; 2] = ["vocals_1", "vocals_2"];

/// Every reserved stem name, for the plan without a listing.
const ALL_RESERVED: [&str; 14] = [
    "song", "guitar", "rhythm", "bass", "keys", "drums", "drums_1", "drums_2", "drums_3",
    "drums_4", "vocals", "vocals_1", "vocals_2", "crowd",
];

/// What the plan needs of the chart: the `[Song]` stream tags.
pub struct Streams<'a> {
    pub tags: &'a [(&'static str, String)],
}

impl Streams<'_> {
    fn tag(&self, tag: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|(t, _)| *t == tag)
            .map(|(_, f)| f.as_str())
    }
}

/// The events of the plan.
///
/// `container` is the name of the `.sng` the entries live in (events then carry the entry number as `index`).
/// `unsafe_names` counts the stream tags that name an absolute path or climb out of the folder (they are refused).
pub fn stem_events(
    streams: &Streams<'_>,
    listing: Option<&Listing>,
    container: Option<&str>,
    unsafe_names: &mut usize,
) -> Vec<SoundEvent> {
    let mut events: Vec<SoundEvent> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let mut add = |events: &mut Vec<SoundEvent>, event: SoundEvent| {
        let key = format!("{}#{:?}", event.file.to_lowercase(), event.index);
        if !seen.contains(&key) {
            seen.push(key);
            events.push(event);
        }
    };
    match listing {
        None => {
            for base in ALL_RESERVED {
                add(
                    &mut events,
                    SoundEvent::optional_whole(0, format!("{base}.ogg")),
                );
            }
            for (_, file) in streams.tags {
                let Some(name) = safe_name(file, unsafe_names) else {
                    continue;
                };
                if !is_reserved_stem(&name) {
                    add(&mut events, SoundEvent::optional_whole(0, name));
                }
            }
        }
        Some(listing) => {
            for hit in resolve_all(streams, listing, unsafe_names) {
                let event = match (hit.index, container) {
                    (Some(index), Some(container)) => SoundEvent::in_container(0, container, index),
                    _ => SoundEvent::whole(0, hit.name.clone()),
                };
                add(&mut events, event);
            }
        }
    }
    events
}

/// `dir/Name.ext` is a reserved stem (`name` without directory and extension, case-insensitive).
fn is_reserved_stem(file: &str) -> bool {
    let leaf = file.rsplit('/').next().unwrap_or(file);
    let has_dir = leaf.len() != file.len();
    let stem = leaf
        .rsplit_once('.')
        .map_or(leaf, |(s, _)| s)
        .to_lowercase();
    !has_dir && ALL_RESERVED.contains(&stem.as_str())
}

/// The files of the listing the song plays: reserved names first, then the stream tags, with `drums` ignored when
/// numbered drums exist and `vocals` ignored when numbered vocals exist.
fn resolve_all<'a>(
    streams: &Streams<'_>,
    listing: &'a Listing,
    unsafe_names: &mut usize,
) -> Vec<&'a Listed> {
    let mut tag_hit = |tag: &str| -> Option<&'a Listed> {
        let name = safe_name(streams.tag(tag)?, unsafe_names)?;
        listing.find(&name).or_else(|| {
            let stem = name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s);
            listing.find_stem(stem)
        })
    };
    let mut hits: Vec<&Listed> = Vec::new();
    for (base, tag) in PLAIN {
        hits.extend(listing.find_stem(base).or_else(|| tag_hit(tag)));
    }
    // Drums: numbered stems, else the single `drums` stem, else the tags.
    let mut drums: Vec<&Listed> = DRUMS_NUMBERED
        .iter()
        .filter_map(|(base, _)| listing.find_stem(base))
        .collect();
    if drums.is_empty() {
        drums.extend(listing.find_stem("drums"));
    }
    if drums.is_empty() {
        drums = DRUMS_NUMBERED
            .iter()
            .filter_map(|(_, tag)| tag_hit(tag))
            .collect();
    }
    hits.extend(drums);
    // Vocals: same rule, the tag is `VocalStream`.
    let mut vocals: Vec<&Listed> = VOCALS_NUMBERED
        .iter()
        .filter_map(|base| listing.find_stem(base))
        .collect();
    if vocals.is_empty() {
        vocals.extend(
            listing
                .find_stem("vocals")
                .or_else(|| tag_hit("VocalStream")),
        );
    }
    hits.extend(vocals);
    hits
}
