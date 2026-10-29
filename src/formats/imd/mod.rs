#![warn(clippy::pedantic)]
//! Rhythm Master classic charts, `.imd` ([`ImdDecoder`]). Decode only.
//!
//! One `.imd` is ONE difficulty of a song: a header, the BPM table and the notes, little-endian, no compression (the encrypted charts of the newer
//! client are not covered). Format notes: `vsrg-format-docs/vsrg/rhythm-master/imd` in Prism's repository, read from `RMEssentials`
//! (`rmechart.cpp`, `RmeChart::fromImd`/`fromImdNote`; AGPL-3.0, so READ for facts only, nothing copied: the layout, the nine note types and
//! the rule that notes outside tracks 3..8 of its own numbering are ignored) and `RM2Malody` (`imd_helper.py`; no licence file, facts only),
//! and checked on real files that were downloaded for local use and deleted. Every decision below is pinned by `tests.rs`, whose files are
//! all built by hand (no real chart is in the repository: the official and fan charts have no licence to redistribute them).
//!
//! ## File
//!
//! | Offset | Type | Field |
//! |---|---|---|
//! | 0 | `u32` | total time of the song, ms (reported, not used) |
//! | 4 | `u32` | BPM entry count `n` |
//! | 8 | `n` x 12 bytes | `{u32 time_ms; f64 bpm}` |
//! | 8+12n | `u16` | separator `0x0303` |
//! | 10+12n | `u32` | note count `m` |
//! | 14+12n | `m` x 11 bytes | `{u16 type; u32 time_ms; u8 track; i32 action}` |
//!
//! The file is EXACTLY `8 + 12n + 6 + 11m` bytes (the notes and `RMEssentials` both require it): any other size, a separator that is not
//! `0x0303`, no BPM entry, or a table or a note list that runs past the end is an error (checked before anything is allocated, with
//! 64-bit arithmetic; files above 64 MiB are refused). Records are NOT sorted in real files: the notes are sorted by time here.
//!
//! ## Columns
//!
//! - **Count**: the file does not store it. The file name does: `<song>_<4|5|6>k_<ez|nm|hd>.imd` (the game's and the community's naming
//!   rule, see [`parse_name`]); when it ends like that, THE NAME WINS and `key_count` is 4, 5 or 6. Otherwise the count is read from the notes
//!   the way `RMEssentials`' chart viewer does: the highest track used plus one, at least 4, at most 6. A note on a lane the chart has not (a
//!   track at or above the count, which also catches 6 and above) is dropped and counted ([`ImdReport::notes_outside_lanes`]).
//! - **Column**: `track` is the 0-based lane, left to right: the column. (`RMEssentials` adds 3 to put it in `imdjson`'s numbering.)
//!
//! ## Notes
//!
//! The nine types of the format (anything else is dropped and counted in [`ImdReport::unknown_types`]; `RMEssentials` refuses the whole
//! file) and what each becomes:
//!
//! | type | the format's meaning | ROX |
//! |---|---|---|
//! | `0x00` | tap | tap |
//! | `0x01` | single slide: one note that flicks to lane `track + action` | tap on `track` ([`ImdReport::single_slides_as_taps`]); the target is lost |
//! | `0x02` | hold, `action` = length in ms | hold |
//! | `0x22`, `0x62`, `0xA2` | a hold segment of a chain (not-end / start / end), `action` = length in ms | hold on its own lane ([`ImdReport::chain_holds`]) |
//! | `0x21`, `0x61`, `0xA1` | a slide of a chain (joint / start / end), `action` = signed lane delta | dropped when it sits beside a hold segment, else a tap |
//!
//! - **Chains** ("worms"): the format builds a hold-and-slide chain of hold segments joined by slides: the finger holds a lane, slides to the
//!   next one at the instant the segment ends, and holds on. ROX has a hold per lane, so EACH SEGMENT IS A HOLD on its own lane, from its time
//!   for its length, and the slides that join them are dropped: nothing is pressed again at a joint (the segment that follows starts on the
//!   same instant, on the lane `track + action`, and is the press). This keeps where the finger is and when. A slide counts as "beside a
//!   hold segment" when a segment ENDS on its time on its own lane (a joint or an end slide) or STARTS on its time on the lane `track +
//!   action` (a start slide or a joint), within 2 ms (real files round a segment's length by one ms or two: 108 of the 1 791 end slides of
//!   the sample were off by 1, one by 2; the tolerance is this reader's, not the format's). Such a slide is counted in
//!   [`ImdReport::chain_joints_dropped`]. A chain slide with no segment beside it is not part of a chain this reader can see: it stays a tap
//!   on its lane instead of being lost ([`ImdReport::lone_chain_slides_as_taps`]). The chain itself is not rebuilt and the order of the
//!   notes is not used, so two chains at once can not be mixed up.
//! - **Holds**: `action` ms from `time`. A length of 0 or less is a tap ([`ImdReport::empty_holds_as_taps`]).
//! - **Inside a hold**: a note on the lane of a hold, between its start and its end, is dropped (the model forbids it,
//!   [`ImdReport::overlapping_notes_dropped`]); two taps on the same lane and time keep the first ([`ImdReport::duplicate_taps`]). A note that
//!   starts exactly where a hold of its lane ends is kept.
//!
//! ## Time
//!
//! Times are milliseconds from the start of the audio already: the note's `time_ms` times 1000 is its time in microseconds, no offset, no tempo
//! conversion (the beat grid only matters to editors: `RMEssentials`' `tick = round(ms x bpm / 1250)`). A hold's length is its `action` in ms.
//!
//! The BPM table is read as timing points so ROX has a BPM: the entries with a BPM that is a finite number in `(0, 1e7]` (others are
//! ignored and counted, [`ImdReport::invalid_bpm_entries`]), sorted by time (of two at the same time the last wins), one point wherever the BPM
//! differs from the point before (official files have one entry per BEAT at one tempo, hundreds of entries: that is ONE point). The signature
//! is 4. The first point is moved to the first note's time when it came later ([`ImdReport::first_bpm_moved`]): the model needs a BPM by
//! the first note. `RMEssentials` and `RM2Malody` only read the first entry; whether the game changes tempo with the table is not verified, and the notes'
//! times do not depend on it anyway.
//!
//! ## Metadata
//!
//! The file has no title, artist, audio or level. `title` is the file name without its extension and without the `_<n>k_<xx>` suffix
//! (`keluodiya_4k_hd.imd` is "keluodiya": the one name of the nine difficulty files of a song; a name with no suffix is its whole stem), or the
//! title given to [`ImdDecoder::decode_with_title`]; `difficulty_name` is `Easy`, `Normal` or `Hard` from the suffix (`_ez`, `_nm`, `_hd`) and EMPTY
//! without one; `difficulty_value` is `None`; `artist` and `creator` are empty; `audio_file` is EMPTY: the song is a separate `<song>.mp3` found
//! by the caller (the community's import rule: song `xxx.mp3`, chart `xxx_4(5,6)k_ez(nm,hd).imd`; `xxx` is [`ImdName::song`]).
//!
//! ## Telling an `.imd` of Rhythm Master from another file
//!
//! `.imd` is an extension other programs use and the format has no signature. [`looks_like_imd`] (the first [`crate::formats::SNIFF_BYTES`] of the file
//! and its length) says yes when the table fits, the separator is where the BPM count puts it and the length is exactly the one the note count
//! gives; Prism's scan lists an `.imd` only then, so an unrelated file is no map and no failure.
//!
//! Not handled: the star-motion `.mde` charts, the `.json` `imdjson` dialect (and the Rhythm Master `WeChat` mini program charts), the encrypted
//! charts of the newer client, the `.imd` files of other programs and games, the `.rmp` archives, the song list XML (titles and artists).
//!
//! What the decoder dropped or simplified is in the [`ImdReport`] of `decode_with_report`.

mod decoder;
mod sniff;

#[cfg(test)]
mod tests;

pub use decoder::{
    ImdDecoder, ImdDifficulty, ImdName, ImdReport, MAX_LANES, MIN_LANES, parse_name,
};
pub use sniff::{is_imd_chart_file, looks_like_imd};
