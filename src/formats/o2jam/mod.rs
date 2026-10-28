//! `O2Jam`: `.ojn` charts ([`OjnDecoder`]) and the `.ojm` sample containers they play ([`OjmContainer`]). Decode only.
//!
//! One `.ojn` holds three charts (Easy `EX`, Normal `NX`, Hard `HX`); a difficulty with no playable note is not a
//! variant (see [`OjnDecoder::variants`]). Every chart has seven columns: the note lanes 1..=7 are the columns 0..=6.
//! Format notes: `vsrg-format-docs/vsrg/o2jam/ojn` and `.../ojm` in Prism's repository, cross-checked against the
//! `Open2Jam` parsers and renderer and Raindrop's `NoteLoaderOJN` (read, not copied).
//!
//! Decisions (the format leaves them to the player; the unit tests pin each of them):
//!
//! - **Time**: a measure is 4 beats, a slot `i` of `n` is at `i / n` of it. The fraction of channel 0 shortens the
//!   measure (the next one starts after `4 x fraction` beats) but does not compress its notes: they stay spread on four
//!   beats, as `Open2Jam` and Raindrop place them. BPM events (channel 1) set the tempo from their beat; the header BPM
//!   is the tempo at beat 0 (an event at beat 0 replaces it, a BPM equal to the one before is dropped). A BPM or a
//!   fraction that is not positive is ignored.
//! - **Order**: packages are read to the end of their section (not by the counts of the header), events are sorted by
//!   position, events at the same position keep the order of the file.
//! - **Notes**: types 0 and 1 are taps (type 1 is "unused" in the notes of the format; `Open2Jam` plays it as a normal
//!   note, so does this decoder, and counts it in the report), 2 starts a long note, 3 ends the one open on its lane.
//!   Broken long notes follow the `O2Jam` policy of `Open2Jam`: a tap or a head inside a long note of the lane goes to the
//!   background sounds, a tail with no head is removed, a head that is never closed (or whose tail is not after it) is a
//!   tap.
//! - **Sounds**: a note's sample id is `value - 1`, plus 1000 when bit 2 of the type is set. Every note's sample, also
//!   the ones the chart drops, and every background event (channel 9 and above; a tail there is removed) is in the
//!   [`AudioPlan`](rox::audio_plan::AudioPlan) as an event on the `.ojm` the header names (`SoundEvent::index` is the sample id).
//!   The volume nibble (1..=15, 0 = maximum) becomes the event volume; the pan nibble is dropped; a long-note tail
//!   makes no sound. The chart's `audio_file` is empty and its `hitsounds` table is empty.
//! - **Text**: strings are cut at NUL, UTF-8 when valid, else CP949.
//!
//! Not handled: the encrypted `.ojn` (header `new`: an error), M30 encryption flags other than 0, 16 and 32 (an error),
//! the cover images, the pan nibble, and everything the format notes mark unknown (`file_version`, the legacy header
//! fields, the header duration and BPM beyond the initial tempo).
//!
//! What the decoder dropped or repaired is in the [`OjnReport`] of `decode_with_report`.

pub mod decoder;
pub mod ojm;
pub mod ojn;

#[cfg(test)]
mod tests;

pub use decoder::{Difficulty, OjnDecoder, OjnReport, Variant};
pub use ojm::{OjmContainer, OjmKind, OjmReport, OjmSample, SampleFormat};
pub use ojn::OjnHeader;
