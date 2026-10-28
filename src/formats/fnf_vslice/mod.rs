//! FNF "V-Slice" (Friday Night Funkin' 0.3+): `<song>-chart.json` with its `<song>-metadata.json` ([`VSliceDecoder`]) and the
//! `.fnfc` package that bundles them with the stems ([`FnfcPackage`]). Decode only. The legacy `{ "song": ... }` charts stay in
//! [`crate::formats::fnf`]. Format notes: `vsrg-format-docs/vsrg/friday-night-funkin/{vslice-chart-json,vslice-metadata-json,fnfc}`
//! in Prism's repository, checked against the game's source (FunkinCrew/Funkin, Apache-2.0, commit `141b440d`, read, not copied:
//! `SongData.hx`, `SongRegistry.hx`, `Conductor.hx`, `play/song/Song.hx`, `audio/VoicesGroup.hx`, `play/PlayState.hx`, `Paths.hx`).
//!
//! Decisions (the format leaves them to the player; the unit tests pin each of them):
//!
//! - **Files**: the chart gives notes, the metadata gives tempo, names and offsets; both are needed (`variants`, `decode_*` take
//!   both as bytes). Versions are the game's: chart `2.0.x`, metadata `2.2.x` (`SongRegistry`), another one is an error. Only
//!   `timeFormat` `ms` (the default) is read; `ticks` and `float` are an error naming the format.
//! - **Variants**: one per difficulty of THIS chart file's `notes` map (a variation has its own file, so its own variants), in
//!   a stable order: the base game's `easy`, `normal`, `hard`, `erect`, `nightmare` ([`DIFFICULTY_ORDER`]), then every other id
//!   sorted by name. A difficulty with no player note is not a variant. `difficulty_name` is the id in Title Case.
//! - **Notes**: the player strumline only: `d` 0..=3 are the columns 0..=3 of a 4-key chart; `d` 4 and above (the opponent and
//!   further strumlines) are not in the chart. `t` ms becomes µs, `l` > 0 is a hold of `l` ms. The note kind `k` is ignored (the
//!   note is a normal one) and counted by name in the [`VSliceReport`]; so are the `events` (not representable). Notes before 0 or
//!   out of range are skipped, a note that starts inside a hold of its column or at the time of the previous one is dropped
//!   (all counted).
//! - **Tempo**: `timeChanges` become BPM points (`t` ms to µs, `bpm`, numerator `n` as the signature; an absent `n` is 4, an
//!   absent `bpm` keeps the previous one). Like the game they are sorted by time and a negative time moves to 0; the first
//!   entry must be at 0 with a `bpm`. The denominator `d` and `bt` have no ROX equivalent (non-4 denominators are counted).
//! - **Scroll speed**: NOT mapped. `scrollSpeed` is one constant per difficulty (a display speed, `ScrollSpeed` has no time), and
//!   ROX's `Sv` point is a multiplier that changes at a time; a constant has no timing change to express (an `Sv` at 0 would
//!   only rescale a chart that has no other `Sv`). The value is in [`VSliceReport::scroll_speed`] (the difficulty's, else
//!   `default`'s). Song events such as scroll-speed changes are not read.
//! - **Metadata**: `songName` (else `Unknown`) is the title, `artist`, `charter` the creator, `playData.ratings[id]` the
//!   `difficulty_value`, `previewStart`/`previewEnd` the preview. `audio_file` is EMPTY (the audio is several stems, see the
//!   plan) and `audio_offset_us` is 0.
//! - **Audio plan**: whole-stem events, `from_us` 0 and no length. The instrumental is `Inst.ogg`, or `Inst-<id>.ogg` for
//!   `playData.characters.instrumental` (`Song.getInstPath`), NOT optional. The vocals are `Voices-<character>.ogg`, or
//!   `Voices-<character>-<variation>.ogg` for a variation other than `default` (`Song.buildPlayerVoiceList`), for the lists
//!   `playerVocals` / `opponentVocals`, else for the characters `player` / `opponent`; they are `optional` (the game ignores
//!   missing vocals). With a way to ask which files exist (`decode_with_audio_plan_in`, and always in a package) the game's
//!   search is reproduced: the list when all its files exist, else the id shortened at its last `-`, then the plain
//!   `Voices[-<variation>].ogg`. Without it the names are taken as written.
//! - **Offsets** (sign): `offsets.*` are milliseconds and a POSITIVE value DELAYS the stem. `SongOffsets` says so ("`5000.0`
//!   means there will be 5 seconds of silence before the song starts", `SongData.hx` 237-243); `Conductor.update` adds
//!   `instrumentalOffset` to the audio position (`Conductor.hx` 433, 294-298) and the game seeks the vocals to
//!   `position - instrumentalOffset` (`PlayState.hx` 1090) then `time -= voicesOffset` (`VoicesGroup.hx` 74-78). So the
//!   instrumental event is at `time_us = instrumental offset` and a vocal event at `instrumental offset + vocal offset`, the
//!   chart's own times unchanged. The offset read is `SongOffsets.getInstrumentalOffset(id)` / `getVocalOffset(character, id)`
//!   with `id` = `characters.instrumental`: an empty id reads `instrumental` / `vocals`, a non-empty one `altInstrumentals[id]`
//!   (else `instrumental`) / `altVocals[id][character]` (the game's `buildVocals` passes `''`, which reads `altVocals['']`
//!   and finds nothing; this decoder follows the documented intent). A listed vocal file uses its side's character offset.
//! - **`.fnfc`**: a ZIP at most [`fnfc::MAX_ENTRIES`] entries and [`fnfc::MAX_TOTAL_BYTES`] declared bytes, entry names that are
//!   absolute or climb out (`..`) refuse the package, JSON entries are read up to 64 MiB and stems on demand up to
//!   [`fnfc::MAX_ENTRY_BYTES`], never trusting the declared size. File names are looked up in the root with the exact case, from
//!   the `songId` of `manifest.json`. The plan's events are `SoundEvent::in_container` events: `file` is the package file name
//!   given by the caller, `index` the entry number in archive order, as for an `.ojm`.
//!
//! What the decoder dropped or repaired is in the [`VSliceReport`].

mod audio;
mod data;
pub mod decoder;
pub mod files;
pub mod fnfc;

#[cfg(test)]
mod tests;

pub use decoder::{DIFFICULTY_ORDER, VSliceDecoder, VSliceReport, Variant};
pub use files::{is_vslice_chart_name, vslice_metadata_name};
pub use fnfc::FnfcPackage;
