#![warn(clippy::pedantic)]
//! What the first bytes of a file say about being an `.imd`: the extension is `.imd` only by convention (other programs use it), and the
//! format has no signature, so a caller that lists files (Prism's library scan) asks before it counts a file as a chart. See `mod.rs`.

use std::io::Read;
use std::path::Path;

use super::decoder::{BPM_ENTRY_BYTES, HEADER_BYTES, NOTE_BYTES, NOTES_HEADER_BYTES, SEPARATOR};
use crate::formats::sekai::SNIFF_BYTES;

fn u32_at(head: &[u8], at: usize) -> Option<u32> {
    head.get(at..at + 4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
}

/// Whether `head` (the first [`SNIFF_BYTES`] of a file, or all of it) of a file of `file_len` bytes looks like an `.imd`: at least one BPM
/// entry, a table that fits in the file, the separator `0x0303` after it and, when the note count is in `head` too, a file length of
/// exactly `8 + 12 x entries + 6 + 11 x notes` bytes. A table longer than `head` (over 5 000 entries) can only be checked as far as it
/// goes: the decoder checks the rest.
#[must_use]
pub fn looks_like_imd(head: &[u8], file_len: u64) -> bool {
    let Some(entries) = u32_at(head, 4) else {
        return false;
    };
    let table_end = HEADER_BYTES as u64 + BPM_ENTRY_BYTES as u64 * u64::from(entries);
    let notes_start = table_end + NOTES_HEADER_BYTES as u64;
    if entries == 0 || notes_start > file_len {
        return false;
    }
    let Ok(table_end) = usize::try_from(table_end) else {
        return false;
    };
    if head.len() < table_end + 2 {
        return true;
    }
    if u16::from_le_bytes([head[table_end], head[table_end + 1]]) != SEPARATOR {
        return false;
    }
    match u32_at(head, table_end + 2) {
        Some(notes) => notes_start + NOTE_BYTES as u64 * u64::from(notes) == file_len,
        None => true,
    }
}

/// Whether the file at `path` is an `.imd` chart: its extension is `.imd` (any case) and the file [`looks_like_imd`]. Reads the first bytes
/// of the file; `false` when it can not be read.
#[must_use]
pub fn is_imd_chart_file(path: &Path) -> bool {
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("imd"))
    {
        return false;
    }
    let mut head = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| {
            let len = file.metadata()?.len();
            file.take(SNIFF_BYTES as u64)
                .read_to_end(&mut head)
                .map(|_| len)
        })
        .is_ok_and(|len| looks_like_imd(&head, len))
}
