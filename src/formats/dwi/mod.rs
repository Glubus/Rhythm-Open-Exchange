//! Dance With Intensity step files (`.dwi`). Decode only.
//!
//! One `.dwi` holds the charts of a whole song: `#SINGLE`, `#DOUBLE`, `#COUPLE` and `#SOLO` tags
//! with a difficulty (`BASIC`, `ANOTHER`, `MANIAC`, `SMANIAC`, and StepMania's aliases) each. Notes
//! of the format notes `vsrg-format-docs/vsrg/dwi/dwi`, read as StepMania's loader
//! (`NotesLoaderDWI.cpp`, MIT) does; the file is parsed into the types of the `.sm` reader
//! ([`crate::formats::sm::types::SmFile`]) and decoded by
//! [`crate::formats::sm::decoder::from_chart`], so BPM changes, freezes (stops) and the offset go
//! through the one pipeline of [`crate::formats::sm`].
//!
//! Decisions (the unit tests pin each of them):
//!
//! - **Step types**: `SINGLE` is `dance-single` (4 columns: left, down, up, right), `SOLO`
//!   `dance-solo` (6: left, up-left, down, up, up-right, right), `DOUBLE` `dance-double` (8: the
//!   first pad's four, then the second pad's four). `COUPLE` (two players with different steps)
//!   is not played: it is named in [`crate::formats::sm::types::SmFile::skipped`], like the `.sm`
//!   reader does for `dance-couple`. A second step string of `SINGLE`/`SOLO` is ignored.
//! - **Difficulty**: StepMania's `DwiCompatibleStringToDifficulty` (`BASIC`/`LIGHT` Easy,
//!   `ANOTHER`/`TRICK`/`STANDARD`/`DIFFICULT` Medium, `MANIAC`/`HEAVY`/`SSR` Hard,
//!   `SMANIAC`/`EXPERT`/`ONI` Challenge, plus `BEGINNER`, `EASY`, `MEDIUM`, `HARD`, `CHALLENGE`, `EDIT`);
//!   an unknown name is kept as written. The name as written is `description` and
//!   [`Variant::name`]. An empty rating is 1; the rating is read like `atoi` (`12.5` is 12).
//! - **Notes**: the cursor starts at beat 0 and advances after each slot; one character is 1/8 of a
//!   measure (a measure is 4 beats), `(..)` 1/16, `[..]` 1/24, `{..}` 1/64, `` `..' `` 1/192, and any
//!   closer resets to 1/8. `<..>` joins characters on one slot (a jump); a `<` followed by a `0`
//!   before the next `>` is the old 1/192 marker instead (StepMania's `Is192`). A row is the
//!   nearest 1/48 beat. `show!hold`: the arrow(s) shown are taps, the arrow(s) of the character after
//!   `!` are the hold head; the hold ends on the NEXT arrow of that column, whatever it is
//!   (the arrow is consumed), and is removed when there is none. A character that is no step character
//!   keeps its slot and is counted in [`DwiReport::warnings`]. Panels a step type lacks (`C`..`M` in
//!   `SINGLE`/`DOUBLE`) are ignored and counted. Each pad of a `DOUBLE` is timed from beat 0.
//!   The format has no mines, rolls, lifts or fakes. A chart whose step strings are both shorter
//!   than 2 characters has no step data (the loader's "didn't get enough data"): it is skipped.
//! - **Timing**: `#GAP` is an integer in ms (`atoi`), the time of beat 0 in the audio, later when
//!   positive (`#OFFSET` = -GAP/1000, then baked into the times like any `.sm`). `#BPM` (it must
//!   be positive) is the BPM at beat 0. `#CHANGEBPM`/`#BPMCHANGE` `BBB=nnn` and `#FREEZE` `BBB=sss`
//!   (milliseconds) put the change at DWI beat BBB, which the loader divides by 4 into `.sm`
//!   beats; a BPM of 0 or less in `#CHANGEBPM` is ignored; an entry that is not exactly `a=b` is ignored
//!   (counted). A freeze behaves as a `.sm` stop (the notes of its beat are hit before it). A file
//!   with no valid `#BPM` or a negative freeze has all its charts skipped.
//! - **Metadata**: the title is the whole `#TITLE` (StepMania splits `Title - subtitle`; this
//!   reader keeps it as written, like the `.sm` reader keeps `#TITLE`), colons included
//!   (the loader reads only up to the first `:`); same for `#ARTIST` and `#FILE`. `#FILE` is
//!   `metadata.audio_file` as written (it may be an absolute path from another machine; the
//!   caller looks for the file in the folder, the loader falls back to a `.wav`/`.mp3` there).
//!   `#SAMPLESTART`/`#SAMPLELENGTH` are ms, seconds (with a `.`) or `MM:SS.ss`; a length under 1
//!   is multiplied by 1000 (loader). The `+` prefix of `#SAMPLESTART` is not applied (the loader
//!   ignores it).
//! - **Text**: UTF-8 when valid, else Latin-1 (the loader's `utf-8,english`). The file is read
//!   without backslash unescaping.
//!
//! Not read: `#DISPLAYBPM`, `#DISPLAYTITLE`, `#DISPLAYARTIST`, `#GENRE`, `#CDTITLE`, `#MD5`,
//! `#STATUS`, `#RAND*`, `#BACKGROUND` scripts (StepMania does not read most of them either).
//!
//! What the reader dropped or repaired is in the [`DwiReport`].
pub mod decoder;
pub mod parser;

#[cfg(test)]
mod tests;

pub use decoder::{DwiDecoder, DwiReport, Variant};
