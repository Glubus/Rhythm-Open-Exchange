//! DDR `.ssq` step files ([`SsqDecoder`]). Decode only.
//!
//! One `.ssq` (usually `<song code>_all.ssq`) holds the tempo table and every chart of the song: Single (4 columns) and
//! Double (8 columns) in the slots Beginner, Basic, Difficult, Expert and Challenge. Each playable chart is a variant (see
//! [`SsqDecoder::variants`]). The format has no public specification; it is documented by reverse engineering. Format notes:
//! `vsrg-format-docs/vsrg/ddr/ssq` in Prism's repository, cross-checked against `SaxxonPike`'s `ssq.md`
//! (rhythm-game-formats) and skogaby's `ddr-chart-tools` (`docs/ssq_format.md`, `ssq_mine_chunk_format.md`; Apache-2.0), read
//! not copied.
//!
//! Decisions (the format leaves them to the reader; the unit tests pin each of them):
//!
//! - **Chunks**: a file is chunks of a 12-byte header (`u32` length with the header, `u16` type, three `u16` parameters),
//!   little-endian, ended by a zero length. Read: tempo (type 1, the first one), steps (type 3), mines (type 20). The events
//!   chunk (2) and the legacy effect chunks (4, 5, 17) do not change the chart and are not read; the events chunk is not
//!   required. Hudson chunks (types 9 and 16) are named in [`SsqReport::skipped`]. A chunk with a bad length (under 12, not a
//!   multiple of 4, past the end of the file) ends the walk: the chunks before it are kept. A chunk with `param2 == 0xFFFF`
//!   ends it too, as the game's lookup does.
//! - **Charts**: a step chunk is playable when its difficulty code is one of the ten the game accepts (`0114 0214 0314 0414 0614`,
//!   `0118 0218 0318 0418 0618`). Other codes (slot `0x05`, other styles) are skipped with a message. Of several chunks with the
//!   same code, the first is the chart (the game finds the first). A chart with no playable note is not a variant. Columns are the
//!   bits of the step byte: 0..=3 = left, down, up, right of player 1, 4..=7 = the same of player 2 (Double only).
//!   `metadata.difficulty_name` is the slot (`Beginner`, `Basic`, `Difficult`, `Expert`, `Challenge`), the same for Single and
//!   Double: [`Variant::name`] (`Single Expert`) tells them apart; `difficulty_value` is `None` (no level in the file).
//! - **Time**: a measure is 4096 ticks. The audio time of a tick is the linear interpolation between the two tempo entries around
//!   it (`tempo_data / ticks per second`), as the game does; before the first entry and after the last one the nearest segment with
//!   a BPM is extended. `tempo_data[0]` is the audio time of the first entry, nothing else is added (`#OFFSET` of a simfile is its
//!   opposite). Two entries with the same offset are a stop; a note on that tick is hit when the stop begins. BPM changes are one
//!   timing point per segment: `240 x TPS x delta ticks / (4096 x delta seconds-ticks)`, a BPM equal to the one before dropped
//!   (they are derived from integers, so only the notes' times are exact). A segment where the tick advances and the clock does
//!   not (old charts) has no BPM and takes no time.
//! - **Notes**: a step byte is a row of simultaneous taps. `0x00` ends freezes: it consumes the next entry of the freeze block, whose
//!   panels (kind `1`; other kinds are ignored) are the ones ending there; for each, the freeze head is the most recent earlier
//!   note on that panel, which becomes a hold lasting until the end tick. A freeze that never ends stays a tap; one that ends at
//!   its head's time stays a tap. The bytes `0x0F`, `0xF0` and `0xFF` are shock arrows: ROX has no shock note, each becomes a
//!   mine on every panel of the side(s) it names (`0x0F` the first four columns, `0xF0` the last four, `0xFF` all), the way
//!   StepMania's converters write them. Panels the style does not have (a fifth bit in a Single chart) are dropped. The mine chunk
//!   of the DDR World mod (type 20, `param2` = the chart's code) adds one mine per set panel; an entry with `flags` or the reserved
//!   word not 0, no panel, a shock pattern or a panel the style lacks is refused. A note that would start inside a freeze of its
//!   column is dropped (the model forbids it).
//! - **Metadata**: the file has no title, artist, audio or level. `title` is empty ([`SsqDecoder::decode_with_title`] and
//!   [`SsqDecoder::decode_file`] set it, `auto_decode*` give the file stem), `artist` and `creator` are empty and `audio_file`
//!   is EMPTY: the audio is a separate bank (`.xwb`) named after the song code, found by the caller.
//!
//! Not handled: the Hudson Wii charts (type 9 with a difficulty code) and lane charts (type 16), the effect chunks, the events and
//! section chunks, the `param4` of a header. Every file in the tests is built in memory; no real `.ssq` is in the repository
//! (Konami's data has no licence).
//!
//! What the decoder dropped or repaired is in the [`SsqReport`] of `decode_with_report`.

mod chunks;
pub mod decoder;
mod tempo;

#[cfg(test)]
mod tests;

pub use decoder::{Difficulty, SsqDecoder, SsqReport, Style, Variant};
