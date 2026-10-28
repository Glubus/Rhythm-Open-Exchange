//! Clone Hero: `.chart` files ([`ChartDecoder`]), `notes.mid` files ([`MidDecoder`]), the `song.ini` beside them
//! ([`SongIni`]) and `.sng` packages ([`SngPackage`]). Decode only.
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
//!   `notes.mid` when the package holds one, else `notes.chart` (the order of `YARG.Core`, which tries `notes.mid`,
//!   `notes.midi`, `notes.chart` and stops at the first it finds; `notes.midi` is not read), the metadata pairs are the
//!   `song.ini` tags. The plan's events have `file` = the name of the `.sng` and `index` = the entry number
//!   (`SngPackage::read`).
//!
//! ## `.mid` (`notes.mid`, [`MidDecoder`])
//!
//! Sources: `vsrg-format-docs/vsrg/clone-hero/mid`, `GuitarGame_ChartFormats` `mid-format` (CC0), the `.mid` reader of
//! Moonscraper as kept in `YARG.Core` (read, not copied). The `.mid` shares the sustain, shift and audio-plan rules of the
//! `.chart` above; what differs is pinned by `mid_tests.rs` (every file there is built byte by byte; no real chart is
//! used, community charts carry no licence to redistribute them).
//!
//! - **Detection**: `.mid` is the extension of every MIDI file, so only a file NAMED `notes.mid` (any case) is a chart
//!   ([`is_mid_chart_name`], `DetectedFormat::CloneHeroMid`): any other `.mid` stays an unsupported format for
//!   `auto_decode*`. The same name rule is used by Prism's scan, downloader, optimizer and audio code. A folder that holds
//!   a `notes.mid` and a `notes.chart` is read through the `.mid` by Clone Hero's readers (`YARG.Core`).
//! - **File**: Standard MIDI File format 0 or 1 in ticks per quarter note (format 2, SMPTE division and a division of 0
//!   are errors, as the notes say they are not supported); `smf.rs` is the reader (variable-length numbers of at most 4
//!   bytes, running status NOT reset by SysEx or meta events since charts exist that do not reset it, SysEx of any
//!   content, unknown chunks skipped, the track count of the header ignored, events after `FF 2F` not read). A cut or
//!   malformed file is an error. Hard limits: 16 MiB, 1024 chunks, 4 million events per track, 1 million notes and
//!   markers per instrument track.
//! - **Tracks**: in a format 1 file the FIRST track is the tempo track only (as `YARG.Core`: it reads the tempo map from
//!   the first chunk and the instruments from the others); in a format 0 file the only track is both (an extension: the
//!   reader of `YARG.Core` skips instruments in a format 0 file). A track is named by its track-name event at tick 0
//!   (the first one naming a 5-lane instrument, else the first), the name matched exactly: `PART GUITAR`, `PART GUITAR
//!   COOP`, `PART BASS`, `PART RHYTHM`, `PART KEYS`. A second track of the same name is ignored, except that a second
//!   `PART GUITAR` replaces the first (the reader of `YARG.Core` does that for the guitar alone). Every other track
//!   (drums, vocals, harmonies, the GHL tracks, the `PART REAL_*` pro tracks, `PART DANCE`, and `T1 GEMS`, which the
//!   notes say can be read like the five, but is not one of them here) is not read: the names that look like
//!   instruments (`PART ...`, `HARM...`, `T1 GEMS`) are in
//!   [`ChartReport::unsupported_tracks`].
//! - **Variants**: the five instruments at `Expert`, `Hard`, `Medium`, `Easy`, ordered like the `.chart`'s (instrument,
//!   then difficulty); a difficulty without a note on the five frets is not a variant. The variant's `section` (and the
//!   report's) is `<track name> <difficulty>`, e.g. `PART GUITAR Expert`; the difficulty name is `Guitar Expert`...
//! - **Notes**: a note is a note on / note off pair of one channel and key (a note on of velocity 0 is a note off); a key
//!   sounding on one channel does not pair with another channel; a second note on of a sounding key, a note off with no
//!   note on and notes still sounding at the end are ignored and counted ([`ChartReport::unpaired_midi_events`]).
//!   Difficulties are octaves: green..orange are the keys 96..=100 (Expert), 84..=88 (Hard), 72..=76 (Medium), 60..=64
//!   (Easy), columns 0..=4. Forced HOPO / strum keys (101/102, 89/90, 77/78, 65/66) and the note-based tap marker 104 are
//!   counted ([`ChartReport::forced_flags`], [`ChartReport::tap_flags`]) and do not change timing; so is the Phase Shift
//!   tap SysEx (`F0 08 'P' 'S' 00 00 <difficulty> 04 <1 start | 0 end> F7`). Solo (103), star power (116), versus,
//!   Big Rock Ending, tremolo/trill markers and the animation keys are ignored. The HOPO threshold is not used and the
//!   10-tick chord snapping of the older games is not applied (the notes call it optional).
//! - **Open notes** (no column, five columns only): the key below each difficulty's green (95, 83, 71, 59) is an open
//!   note only after an `[ENHANCED_OPENS]` text event (with or without brackets; from the event on, as in `YARG.Core`;
//!   without it those keys are animation data and ignored); the Phase Shift open SysEx (modifier `01`, difficulty 0
//!   easy..3 expert or `FF` all) turns every note of its difficulty from its start tick to the tick before its end (a
//!   phrase of no length holds its start tick) into an open note. Every open note is dropped and counted
//!   ([`ChartReport::open_notes_dropped`]), so a chart that uses them loses them (lossy).
//! - **Sustains**: a note longer than 0 ticks is a hold, unless it is shorter than the cutoff: then it is a tap
//!   ([`ChartReport::sustains_cut_to_taps`]). The cutoff is a third of the resolution (integer division; the notes: "a
//!   sustain shorter than a 1/12th step is a normal note"; `YARG.Core`: `resolution / 3`), or the `song.ini`
//!   `sustain_cutoff_threshold` when it is given and not negative (a 0 is read as 1 tick: only length-0 notes are taps).
//!   A hold never goes past the next note of its fret (only reachable with the same key on two channels). Chords may
//!   have a different length per note.
//! - **Time**: ticks to µs with the tempo map of the first track, exact in µs per quarter note (a `FF 51` of 0 or of a
//!   length other than 3, and a `FF 58` of fewer than 2 bytes, a numerator of 0 or an exponent above 31 are ignored and
//!   counted; the last event of a tick wins; 120 BPM until the first tempo); `signature` is the time signature in quarter
//!   notes (`numerator x 4 / 2^exponent` rounded) in force at each tempo. Tempo and time signature events of the
//!   instrument tracks are ignored and counted ([`ChartReport::sync_events_off_first_track`]). A `.mid` has no `Offset`:
//!   the `song.ini` `delay` shifts every time (same sign as the `.chart`) and the stems start at 0. A note beyond 10^7
//!   seconds is dropped.
//! - **Metadata**: none in the file: title, artist, creator, genre, background, preview and `diff_*` come from the
//!   `song.ini` (empty without one); `audio_file` is empty; the audio plan is the `.chart`'s without stream tags (the
//!   reserved stem names only).
//!
//! Not handled: drums, GHL, forced/tap/open flags (no ROX counterpart), star power, solos, lyrics, `hopo_frequency`,
//! `notes.midi`, a `.mid` of another name, `.sng` versions other than 1.
//!
//! What the decoder dropped or repaired is in the [`ChartReport`].

pub mod chart;
pub mod decoder;
pub mod ini;
pub mod mid;
pub mod plan;
pub mod smf;
pub mod sng;

#[cfg(test)]
mod mid_tests;
#[cfg(test)]
mod tests;

pub use decoder::{
    ChartContext, ChartDecoder, ChartReport, Difficulty, Instrument, ShiftSource, Variant,
};
pub use ini::SongIni;
pub use mid::{MidDecoder, is_mid_chart_name};
pub use plan::Listing;
pub use sng::{SngEntry, SngPackage};
