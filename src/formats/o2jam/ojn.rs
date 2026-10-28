#![warn(clippy::pedantic)]
//! The bytes of an `.ojn`: the 300-byte header and the packages of a note section.
//!
//! Offsets are the ones of `vsrg-format-docs/vsrg/o2jam/ojn` (checked against the real files there). This file only
//! reads; the meaning of the events (time, holds, sounds) is in `decoder.rs`.

use std::borrow::Cow;

use encoding_rs::EUC_KR;
use rox::error::{RoxError, RoxResult};

pub(super) const HEADER_LEN: usize = 300;

/// The header of an `.ojn`: the fields Prism reads. The legacy fields (`old_*`, `bmp_size`, `file_version`) are not kept.
#[derive(Debug, Clone, PartialEq)]
pub struct OjnHeader {
    pub song_id: i32,
    /// `encode_version` (2.9 in every known file).
    pub encode_version: f32,
    pub genre: i32,
    /// Initial BPM of the header.
    pub bpm: f32,
    /// Level of Easy, Normal, Hard.
    pub level: [i16; 3],
    pub event_count: [i32; 3],
    /// Playable notes per difficulty as the header counts them: every event on a note lane, so a long note counts twice.
    pub note_count: [i32; 3],
    pub measure_count: [i32; 3],
    pub package_count: [i32; 3],
    pub title: String,
    pub artist: String,
    pub noter: String,
    /// Name of the sample container, as written in the file (not sanitised).
    pub ojm_file: String,
    pub cover_size: i32,
    /// Duration in seconds per difficulty, display data of the header.
    pub duration_s: [i32; 3],
    pub note_offset: [i32; 3],
    pub cover_offset: i32,
}

fn le_i32(data: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

fn le_i16(data: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([data[at], data[at + 1]])
}

fn le_f32(data: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

fn three(data: &[u8], at: usize) -> [i32; 3] {
    [le_i32(data, at), le_i32(data, at + 4), le_i32(data, at + 8)]
}

/// A fixed-size NUL-padded string: cut at the first NUL, UTF-8 if it is valid, else CP949 (EUC-KR, what the Korean
/// clients write), else with the bytes that cannot be read replaced.
fn text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let bytes = &bytes[..end];
    let text: Cow<'_, str> = match std::str::from_utf8(bytes) {
        Ok(text) => Cow::Borrowed(text),
        Err(_) => EUC_KR.decode_without_bom_handling(bytes).0,
    };
    text.trim().to_string()
}

impl OjnHeader {
    /// Reads the header.
    ///
    /// # Errors
    /// Returns an error if the file is shorter than 300 bytes, is an encrypted OJN (header `new`, not supported) or
    /// has no `ojn` signature.
    pub fn parse(data: &[u8]) -> RoxResult<Self> {
        if data.starts_with(b"new") {
            return Err(RoxError::InvalidFormat(
                "ojn: encrypted file (`new` header) is not supported".into(),
            ));
        }
        if data.len() < HEADER_LEN {
            return Err(RoxError::InvalidFormat(format!(
                "ojn: {} bytes, the header alone is {HEADER_LEN}",
                data.len()
            )));
        }
        if &data[4..8] != b"ojn\0" {
            return Err(RoxError::InvalidFormat("ojn: no `ojn` signature".into()));
        }
        Ok(Self {
            song_id: le_i32(data, 0),
            encode_version: le_f32(data, 8),
            genre: le_i32(data, 12),
            bpm: le_f32(data, 16),
            level: [le_i16(data, 20), le_i16(data, 22), le_i16(data, 24)],
            event_count: three(data, 28),
            note_count: three(data, 40),
            measure_count: three(data, 52),
            package_count: three(data, 64),
            title: text(&data[108..172]),
            artist: text(&data[172..204]),
            noter: text(&data[204..236]),
            ojm_file: text(&data[236..268]),
            cover_size: le_i32(data, 268),
            duration_s: three(data, 272),
            note_offset: three(data, 284),
            cover_offset: le_i32(data, 296),
        })
    }

    /// Name of the genre, as the format notes list them (any other number is `Etc`).
    #[must_use]
    pub fn genre_name(&self) -> &'static str {
        match self.genre {
            0 => "Ballad",
            1 => "Rock",
            2 => "Dance",
            3 => "Techno",
            4 => "Hip-hop",
            5 => "Soul/R&B",
            6 => "Jazz",
            7 => "Funk",
            8 => "Classical",
            9 => "Traditional",
            _ => "Etc",
        }
    }

    /// The bytes of the section of difficulty `index` (0 Easy, 1 Normal, 2 Hard): from its `note_offset` to the next
    /// one (to `cover_offset` for the last). A section whose offsets are outside the file or reversed is empty.
    pub(super) fn section<'a>(&self, data: &'a [u8], index: usize) -> &'a [u8] {
        let offset = |value: i32| {
            usize::try_from(value)
                .ok()
                .filter(|&o| (HEADER_LEN..=data.len()).contains(&o))
        };
        let Some(start) = offset(self.note_offset[index]) else {
            return &[];
        };
        let end = if index < 2 {
            offset(self.note_offset[index + 1])
        } else {
            // A cover offset that is not a place of the file leaves the last section running to the end.
            offset(self.cover_offset).or(Some(data.len()))
        };
        match end {
            Some(end) if end >= start => &data[start..end],
            _ => &[],
        }
    }
}

/// What a package slot holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Slot {
    /// Channel 0: the measure is this many times a 4-beat measure.
    MeasureFraction(f32),
    /// Channel 1: new BPM.
    Bpm(f32),
    /// Channels 2..=8 (lane 1..=7) and every other channel (background sound): the 4 bytes of a note event.
    Note {
        /// `0` for background channels, else the lane `1..=7`.
        lane: u8,
        /// The signed `value` field (never 0 here).
        value: i16,
        volume_pan: u8,
        note_type: u8,
    },
}

/// One non-empty slot of a package, with where it sits in its measure.
#[derive(Debug, Clone, Copy)]
pub(super) struct RawEvent {
    pub measure: u32,
    /// Slot `index` of `count` slots: the event is at `index / count` of the measure.
    pub index: u16,
    pub count: u16,
    pub slot: Slot,
}

/// How many bytes of a section were not packages.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SectionScan {
    /// The section ended inside a package header or inside the slots of a package.
    pub truncated: bool,
    /// Packages with a negative measure number: skipped.
    pub negative_measures: usize,
}

/// Walks the packages of a section (until its bytes end, not by `package_count`, as `Open2Jam` does). Empty slots
/// (note value 0, BPM or fraction 0) are not returned but their place in the grid is kept by `index`/`count`.
pub(super) fn scan_section(section: &[u8]) -> (Vec<RawEvent>, SectionScan) {
    let mut events = Vec::new();
    let mut scan = SectionScan::default();
    let mut at = 0usize;
    while at < section.len() {
        if section.len() - at < 8 {
            scan.truncated = true;
            break;
        }
        let measure = le_i32(section, at);
        let channel = le_i16(section, at + 4);
        let count = u16::try_from(le_i16(section, at + 6)).unwrap_or(0);
        at += 8;
        let body = usize::from(count) * 4;
        if section.len() - at < body {
            scan.truncated = true;
            break;
        }
        let Ok(measure) = u32::try_from(measure) else {
            scan.negative_measures += 1;
            at += body;
            continue;
        };
        for index in 0..count {
            let slot = &section[at..at + 4];
            at += 4;
            let slot = match channel {
                0 | 1 => {
                    let value = f32::from_le_bytes([slot[0], slot[1], slot[2], slot[3]]);
                    // `value == 0` is the only empty slot; NaN is kept so the decoder can count it.
                    if value == 0.0 {
                        continue;
                    }
                    if channel == 0 {
                        Slot::MeasureFraction(value)
                    } else {
                        Slot::Bpm(value)
                    }
                }
                _ => {
                    let value = i16::from_le_bytes([slot[0], slot[1]]);
                    if value == 0 {
                        continue;
                    }
                    let lane = match channel {
                        2..=8 => u8::try_from(channel - 1).unwrap_or(0),
                        _ => 0,
                    };
                    Slot::Note {
                        lane,
                        value,
                        volume_pan: slot[2],
                        note_type: slot[3],
                    }
                }
            };
            events.push(RawEvent {
                measure,
                index,
                count,
                slot,
            });
        }
    }
    (events, scan)
}
