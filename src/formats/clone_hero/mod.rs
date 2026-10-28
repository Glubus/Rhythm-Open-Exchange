//! Clone Hero: `.chart` files ([`ChartDecoder`]), the `song.ini` beside them ([`SongIni`]) and `.sng` packages
//! ([`SngPackage`]). Decode only.
//!
//! One `.chart` holds every instrument and difficulty of a song; a track is a section `[<Difficulty><Instrument>]`.
//! Format notes: `vsrg-format-docs/vsrg/clone-hero/{chart,ini,sng}` in Prism's repository, cross-checked against the
//! `GuitarGame_ChartFormats` docs (CC0), the `.chart` reader of Moonscraper (as kept in `YARG.Core`) and the `.sng`
//! reader of `YARG.Core` (read, not copied).
//!
//! Decisions (the format leaves them to the player; the unit tests pin each of them):
//!
//! - **Variants**: the 5-lane tracks `Single` (guitar), `DoubleGuitar`, `DoubleBass`, `DoubleRhythm` and `Keyboard`
//!   (the format notes give them all the same five-fret notes), at `Expert`, `Hard`, `Medium`, `Easy`, ordered by
//!   instrument in that order then by difficulty in that order. A section with no note on the five frets is not a
//!   variant. Drums, the GHL (6-fret) tracks, `SingleBass` (a legacy track the notes call unclear) and every other
//!   section are not read: the names of the sections that look like tracks are in
//!   [`ChartReport::unsupported_tracks`].
//! - **Notes**: `N 0..=4` are the columns 0..=4 (five columns). `N 5` (strum/HOPO flip) and `N 6` (tap) are modifiers
//!   that do not change timing: not kept, counted. `N 7` (open) has no column: dropped and counted, so a chart that
//!   uses open notes loses them (lossy; five columns only). Other types are counted and dropped. Chords are plain
//!   simultaneous notes. Two notes on the same tick and fret: the first stays. `S` (star power, phrases), `E` (events,
//!   solos) and `H` are ignored.
//! - **Sustains**: a length above 0 is a hold, a length of 0 a tap. A `.chart` does NOT cut short sustains by itself
//!   (the notes say so, unlike `.mid`; the reader in `YARG.Core` has a threshold of 0 for `.chart`). The `song.ini` tag
//!   `sustain_cutoff_threshold` (ticks) changes that when it is given: a shorter sustain becomes a tap. A hold never
//!   goes past the next note of its fret (it ends where that starts), as the model forbids overlaps.
//! - **Time**: ticks to µs with the tempo map: `B` events in thousandths of a BPM, 120 BPM until the first `B`, the
//!   last `B` of a tick wins; `A` anchors are ignored; time signatures do not enter the conversion (a `TS` only sets
//!   `signature` of the BPM points: the numerator in quarter notes, `numerator x 4 / 2^exponent` rounded, default
//!   exponent 2). `Resolution` defaults to 192 when absent (a present but invalid one is an error). Events are sorted
//!   by tick (stable); a line that is not `<tick> = ...` is skipped and counted.
//! - **Audio shift**: times of a ROX chart count from the start of the audio. `Offset` of `[Song]` (seconds: audio
//!   position = chart time + `Offset`, so a higher value makes the audio start sooner; Moonscraper) and `delay` of the
//!   `song.ini` (ms: delays the chart relative to the audio, higher = later notes) have the SAME sign: every note and
//!   tempo is moved by `+Offset` or `+delay` (the way the `.sm` decoder moves by its offset), and the stems start at
//!   0. If both are present the `song.ini` `delay` wins when it is not 0 (the notes give the `song.ini` priority;
//!   the reader in `YARG.Core` only reads `delay` and never `[Song]` `Offset`), else `Offset` applies; they are never
//!   added. A shift beyond 10^7 s is not applied ([`ChartReport::shift_out_of_range`]).
//! - **Text**: UTF-8 when valid, else Latin-1 (the choice of `YARG.Core`; the notes say the encoding is not
//!   specified); a byte-order mark is removed. Quoted values lose ONE outer pair of quotes. Keys of `[Song]` and the
//!   section names are matched exactly as written. Sections may come in any order and may be repeated (merged); a body
//!   that is never closed runs to the next section. `[Song]` is required, `[SyncTrack]` is not (120 BPM).
//! - **Metadata**: title, artist and creator come from the `song.ini` (`name`, `artist`, `charter`) when given, else
//!   from `[Song]` (`Name`, `Artist`, `Charter`); the difficulty name is `<Instrument> <Difficulty>` (`Guitar Expert`,
//!   `Guitar Co-op Hard`, `Bass Easy`, `Rhythm Medium`, `Keys Expert`); the difficulty value is the `song.ini` `diff_*`
//!   tag of the instrument when it is not negative; genre, background, preview start and length come from the
//!   `song.ini`, else `[Song]`. `audio_file` is empty and `hitsounds` is empty: the sound is the audio plan.
//! - **Audio plan**: the song plays EVERY stem at once, whichever the chosen track: one event per stem, a whole file at
//!   time 0. The stems are the reserved names `song`, `guitar`, `rhythm`, `bass`, `keys`, `drums`, `drums_1..4`,
//!   `vocals`, `vocals_1`, `vocals_2`, `crowd` (not `preview`, not `vocals_explicit*`) with `.opus`, `.ogg`, `.mp3` or
//!   `.wav`, and the files of the `[Song]` tags `MusicStream`, `GuitarStream`, `RhythmStream`, `BassStream`,
//!   `KeysStream`, `DrumStream`..`Drum4Stream`, `VocalStream`, `CrowdStream`; the reserved names are checked first and
//!   the tag only when its stem has no reserved file, `drums` is ignored when `drums_N` exists and `vocals` when
//!   `vocals_N` exists. Without a file listing every event is `optional` (reserved stems named `<name>.ogg`, tag files
//!   as written unless they are a reserved stem) and the CONSUMER must apply the two "ignored when" rules and find
//!   the other extensions; with a listing (see [`ChartContext`]) only the files that exist are in the plan (not
//!   optional) and the rules are applied. The game also mutes a stem when notes of its instrument are missed: not
//!   reproduced, the plan plays every stem always.
//! - **`.sng`**: [`SngPackage::open`] reads header, metadata, file index and the masked data; the chart is
//!   `notes.chart` (`notes.mid` is an error: `.mid` is not read), the metadata pairs are the `song.ini` tags. The
//!   plan's events have `file` = the name of the `.sng` and `index` = the entry number (`SngPackage::read`).
//!
//! Not handled: `.mid`, drums, GHL, open notes, forced/tap flags (no ROX counterpart), star power, solos, lyrics,
//! `hopo_frequency`, the `[Events]` section (also `end`), `.sng` versions other than 1.
//!
//! What the decoder dropped or repaired is in the [`ChartReport`].

pub mod chart;
pub mod decoder;
pub mod ini;
pub mod plan;
pub mod sng;

#[cfg(test)]
mod tests;

pub use decoder::{
    ChartContext, ChartDecoder, ChartReport, Difficulty, Instrument, ShiftSource, Variant,
};
pub use ini::SongIni;
pub use plan::Listing;
pub use sng::{SngEntry, SngPackage};
