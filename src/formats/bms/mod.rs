//! BMS (Be-Music Source) family: `.bms`, `.bme`, `.bml` ([`BmsDecoder`]) and `.pms` ([`PmsDecoder`]). Decode only.
//!
//! The syntax is one text format; the extension is a historical hint, except for `.pms`, which is the only way to know
//! a chart has nine buttons, hence a second decoder. Format notes: `vsrg-format-docs/vsrg/bms` in Prism's repository.
//!
//! Decisions (the format leaves them to the player; the unit tests pin each of them):
//!
//! - **Text**: byte order mark, else valid UTF-8, else `Shift_JIS`, else EUC-KR (`source.rs`).
//! - **`#RANDOM`**: reproducible. `#RANDOM n` always draws 1, `#SETRANDOM n` forces `n`, an `#IF` before any `#RANDOM`
//!   reads 1; `#ELSEIF`, `#ELSE`, `#ENDRANDOM` and nesting are supported (`parser.rs`).
//! - **Lines**: headers keep the last value; the lines of one channel of one measure are merged by position (a later
//!   object overrides an earlier one, `00` never does), except channel 01 which is only read to find the audio.
//! - **Time**: a measure is 4 beats times its channel-02 length, an object at slot `i` of `n` is at `i/n` of it; BPM from
//!   channel 03 (hexadecimal) and 08 (`#BPMxx`/`#EXBPMxx`); STOP from channel 09 (1/48 beat units, not scaled by the
//!   measure length, using the BPM that follows a BPM change at the same position; an object at the position of a STOP
//!   is at the start of the pause). A BPM that is not positive, and a STOP that is not positive, are ignored. STOPs are
//!   baked into the note times, as `.sm` does (`timing.rs`).
//! - **Layout** (see [`decoder`]): the lanes the chart uses, never `#PLAYER`. The scratch is its own column, left of the
//!   keys of the 1P side and right of the keys of the 2P side.
//! - **Long notes**: `#LNTYPE 1` channels (`5x`/`6x`) pair up head and tail; `#LNOBJ` closes the previous note of the lane;
//!   `#LNTYPE 2` runs end at the first empty slot. Invisible objects are ignored, landmines (`Dx`/`Ex`) are mines.
//! - **Sounds**: the sound of each playable note goes to the hitsound table. ROX has no background-music object, so
//!   channel 01 is not decoded, except that when the chart has exactly one defined background sound it is the chart's
//!   audio and the time origin (every time moves back to its position).
//!
//! `.bmson` (the JSON dialect, [`bmson`]) shares the timeline and the column order; its layout comes from `mode_hint`.
//!
//! What the decoder dropped or repaired is in the [`BmsReport`] of `decode_with_report`.

pub mod bmson;
pub mod decoder;
mod parser;
pub(crate) mod source;
pub(crate) mod timing;

pub(crate) use parser::Frac;

#[cfg(test)]
mod bmson_tests;
#[cfg(test)]
mod tests;

pub use bmson::{BmsonDecoder, BmsonReport};
pub use decoder::{BmsDecoder, BmsReport, PmsDecoder};
