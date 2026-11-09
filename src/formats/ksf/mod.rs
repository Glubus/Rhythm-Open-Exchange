//! Kick It Up / Direct Move step files (`.ksf`). Decode only.
//!
//! One `.ksf` is ONE chart; a song is a folder of several (one per difficulty and play style).
//! Notes of the format notes `vsrg-format-docs/vsrg/kick-it-up/ksf`, read as StepMania's loader
//! (`NotesLoaderKSF.cpp`, MIT) does; the file is parsed into the types of the `.sm` reader and
//! decoded by [`crate::formats::sm::decoder::from_chart`], so delays, BPM changes and the offset go
//! through the one pipeline of [`crate::formats::sm`].
//!
//! Decisions (the unit tests pin each of them):
//!
//! - **Step type and difficulty** come from the FILE NAME (lower-cased, extension removed), by the
//!   loader's substring rules (`another` Edit, `wild`/`wd`/`crazy+`/`cz+`/`hardcore` Challenge,
//!   `crazy`/`cz`/`nightmare`/`nm` Hard, `hard`/`hd`/`freestyle`/`fs`/`double` Medium,
//!   `easy`/`ez`/`normal` Easy, `beginner`/`practice`/`pr` Beginner, else Hard; default meters 25,
//!   20, 14, 8, 4, 4, 10 when `#DIFFICULTY` is absent) and `halfdouble` (`pump-halfdouble`, the
//!   middle six of the ten columns) / `double`, `nightmare`, `freestyle`, `db`, `nm`, `fs` or
//!   `#PLAYER` containing `double` (`pump-double`) / `_1` / `_2` (`pump-couple`) / else
//!   `pump-single`. `pump-single` (5 columns: down-left, up-left, center, up-right,
//!   down-right), `pump-halfdouble` (6) and `pump-double` (10, the two pads) are played, as in the `.sm` reader;
//!   `pump-couple` (10 columns, two players) is named in [`crate::formats::sm::types::SmFile::skipped`]. The file name
//!   is the chart's `description` (an `Edit` is listed by it).
//! - **Rows**: `#STEP` rows of 13 characters, one per `1/#TICKCOUNT` beat (4 when absent; 0 or less
//!   refuses the chart), up to the row of thirteen `2`s; the rows after it are not read. `|` rows do not take a
//!   slot. `0` empty, `1` tap, `4` hold (the head on the first `4`, the tail on the LAST `4` row;
//!   one row is a tap; a hold still open at the end row is closed there, one with no end row is
//!   removed), `M` mine, `L` lift (a tap, as in `.sm`), `F` fake (not played). Any other character
//!   is counted and ignored. A row shorter than the step type needs refuses the chart.
//! - **Timing**: `#STARTTIME` is centiseconds, the time of beat 0 in the audio (positive = later).
//!   `#BPM` is the BPM at beat 0 (it must be positive). When the file is "KIU compliant" (it has
//!   `#BPM2`/`#BPM3`/`#BUNKI`/`#BUNKI2`/`#STARTTIME2`, or reaches its end row with no `|` row; judged
//!   per file, the loader judges the last file of the folder), `#BPM2` takes over at `#BUNKI`
//!   centiseconds and `#BPM3` at `#BUNKI2`, converted to beats the way the loader's `HandleBunki` does
//!   (`(time + gap) * early BPM / 60`, with `#STARTTIME2` as the second gap; the loader's
//!   approximation, kept). The Direct Move rows `|T<n>|` (tick count), `|B<n>|` (BPM at the current
//!   beat), `|E<n>|` (a delay of `n` ticks at the current BPM) and `|D<ms>|` (a delay of `ms`) are read; `|M` `|C` `|S`
//!   `|F` `|X` (combo, speed, scroll: no effect on time) are ignored and counted. A delay freezes the
//!   song before the notes of its beat, like a `.sm` `#DELAYS`. The loader's per-beat float
//!   accumulation is done in `f64`; rows are 1/48 beat, rounded like the loader's `BeatToNoteRow`.
//!   `#STARTTIME2` is read only as that second gap, `#STARTTIME3` only marks the file as compliant: the
//!   format's mid-song offset jumps are not applied (the notes say StepMania does not implement them).
//! - **Metadata**: `#TITLE` is `Title`, `Artist - Title` or `Artist - Title - Difficulty`
//!   (difficulty names `double`, `easy`, `normal`, `hard`, `crazy`, `nightmare` dropped); what
//!   it does not give comes from the folder name given in the [`KsfContext`] by the same rule.
//!   `#SONGFILE` is `metadata.audio_file` as written; empty when absent (the loader plays
//!   `song.mp3|oga|ogg|wav` of the folder first: the caller's job), `#TITLEFILE` the background, `#DISCFILE` the banner;
//!   `#MUSICINTRO`/`#INTRO` the preview start (`SS`, `MM:SS` or `HH:MM:SS`), the preview lasts
//!   7 s (the loader's constant).
//! - **Text**: UTF-8 when valid, else EUC-KR/CP949 (Korean charts). Read without unescaping.
//!
//! What the decoder dropped or repaired is in the [`KsfReport`].
pub mod decoder;
pub mod parser;

#[cfg(test)]
mod tests;

pub use decoder::{KsfDecoder, KsfReport};
pub use parser::KsfContext;
