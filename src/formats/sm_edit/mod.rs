//! StepMania `.edit` files ([`EditDecoder`]). Decode only; read with the song they belong to.
//!
//! An `.edit` is a `.sm`/`.ssc` fragment: a `#SONG:Group/Song folder;` tag naming the song and ONE chart, in the `.ssc` layout
//! (`#NOTEDATA:;` then `#STEPSTYPE`, `#DESCRIPTION`, `#DIFFICULTY`, `#METER`, `#CREDIT`, optionally its own timing tags, then
//! `#NOTES`) or the `.sm` layout (one `#NOTES:` of six parameters). It has no title, artist, audio or timing of its own: the
//! song's. ROX does not find songs, so the caller reads the edit's [`EditDecoder::song_path`], finds the song's `.sm`/`.ssc`
//! itself and hands the song over: [`EditDecoder::decode_for_song`] (bytes and extension),
//! [`EditDecoder::decode_for_header`] (a [`SongHeader`] read once for several edits) or [`EditDecoder::decode_for_song_file`]
//! (the song file's path). `auto_decode` of an `.edit` path fails with `InvalidFormat("edit needs its song")`: a path alone can
//! not resolve the song.
//!
//! Format notes: `vsrg-format-docs/vsrg/stepmania/edit` in Prism's repository, checked against StepMania's loaders
//! (`NotesLoaderSM.cpp`/`NotesLoaderSSC.cpp` `LoadEditFromMsd`, `SongManager::FindSong`, `Song::Matches`; MIT), read not copied.
//!
//! Decisions (the unit tests pin each of them):
//!
//! - **Song**: `#SONG` is read as the loader does: the first one counts, backslashes become `/`, empty parts are dropped, and the
//!   result is one part (a folder or title) or two (`Group/Song folder`): StepMania matches the group by name and the song by
//!   folder name or title, ignoring case. [`EditDecoder::song_path`] returns that normalised path and refuses what is not safe to
//!   join to a folder: a leading separator (absolute), a drive, `.` or `..` parts, control characters, more than two parts, an
//!   empty value. An edit without `#SONG` has no song path (error): StepMania infers the song from the folder
//!   `Edits/<Group>/<Song folder>/` or from the song folder the edit sits in; the caller does the same.
//! - **Chart**: only the first chart of the file is used, as the game does ("one Steps per edit file"). The layout is told by the
//!   tags: a `#NOTEDATA` before the first `#NOTES` is `.ssc` style (the chart is its tags up to that `#NOTES`), otherwise the first
//!   `#NOTES` with the six `.sm` parameters. Whatever difficulty the file says, the chart is `Edit`, and
//!   `metadata.difficulty_name` is its `#DESCRIPTION` (`Edit` when empty). The chart is read by the same code as a chart of a
//!   `.sm`/`.ssc` (taps, holds, rolls, mines, lifts, fakes, warps), restricted to the same step types: `dance-single` 4,
//!   `dance-solo` 6, `dance-double` 8, `pump-single` 5, `pump-halfdouble` 6, `kb7-single` 7; another step type, bad rows or timing
//!   it can not honour make the decode fail with the reason ([`EditDecoder::parse_for_song`] returns it in `SmFile::skipped`).
//! - **Timing**: the edit uses the song's `#OFFSET`, `#BPMS`, `#STOPS`, `#DELAYS`, `#WARPS` and `#FAKES`; a `.ssc`-style edit with
//!   its own `#BPMS` has its own split timing (stops, delays, warps, fakes too) and may carry its own `#OFFSET`, which the
//!   loader honours whatever the `#VERSION`. Times include the offset, as everywhere in the `sm` module.
//! - **Metadata**: title, artist, `#CREDIT` of the song (the edit's own `#CREDIT` wins), background, preview and `audio_file` are the
//!   song's. `audio_file` is the song's `#MUSIC` as written: RELATIVE to the song's folder (the caller makes it absolute);
//!   `decode_for_song_file` also applies the `.sm` rule that an empty or missing `#MUSIC` plays the first audio file of that
//!   folder. The edit's `#SONG` and the order of the tags do not matter beyond the above.
//! - **Size**: a file over 60 KiB (StepMania's `MAX_EDIT_STEPS_SIZE_BYTES`) is refused ([`MAX_EDIT_SIZE`]), as the game does.
//!
//! Not handled: the duplicate-edit and per-profile limits of the game, `.ssc` tags other than the ones above (as in `sm`).

mod decoder;

#[cfg(test)]
mod tests;

pub use decoder::{EditDecoder, MAX_EDIT_SIZE};
