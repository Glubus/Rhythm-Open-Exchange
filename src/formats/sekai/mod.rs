#![warn(clippy::pedantic)]
//! Project SEKAI lane charts: `.sus` ([`SusDecoder`]) and `.usc` ([`UscDecoder`]), and the BDON chart of BanG Dream! Our Notes
//! ([`BdonDecoder`], same 12-lane model, see `bdon.rs`). Decode only.
//!
//! Both formats describe one chart per file on the 12 fixed lanes of Project SEKAI. Format notes:
//! `vsrg-format-docs/vsrg/project-sekai/{sus,usc}` in Prism's repository, cross-checked against
//!
//! - the SUS 2.7 specification (kb10uy's gist, `#WAVE`, `#WAVEOFFSET`, `#MEASUREBS`, `#mmm02`, `#BPMzz` / `#mmm08`) and
//!   `YumYummity`'s "SUS Format Specification for PJSK" gist (the Project SEKAI dialect: note kinds, overlaps, lanes);
//! - `sonolus-pjsekai-engine` (`NonSpicyBurrito`; MIT; commit 0f6f37f): `lib/src/sus/analyze.ts` and `convert.ts` (`susToUSC`,
//!   what a SUS measure, a lane, a width, an overlap and the wave offset become), `lib/src/usc/index.ts` and `convert.ts`
//!   (the USC types and what each connection is);
//! - `sus-js` (mkpoli; MIT; commit e1d9fe9): `sus-parse` (the defaults: 4/4 without `#mmm02`, 480 ticks per beat without
//!   `#REQUEST`);
//! - `rg_converters` (`UntitledCharts`; GPL-3.0, so READ for facts only, nothing copied): the USC wrapper
//!   `{"usc": {"offset", "objects"}, "version": 1|2}`, the object types of version 2 (`damage`, `guide`, `skill`, `feverChance`,
//!   `feverStart`, `timeScaleGroup`), and that the USC `offset` is the opposite of the SUS `#WAVEOFFSET`.
//!
//! Every decision below is pinned by `tests.rs`, whose files are all built by hand (no real chart is in the repository:
//! community charts have no licence to redistribute them).
//!
//! ## Lanes and columns
//!
//! - **Columns**: ALWAYS 12 (`key_count = 12`), one per lane of the game. A chart that uses fewer lanes keeps its lanes' numbers:
//!   the column of a note is where it is on the screen, so a layout (a 12K one, when it exists) does not depend on the chart.
//!   In a SUS the 12 lanes are `x = 2..=13` (`x` is the base-36 lane of the line, 0 and 1 and 14 and 15 are outside the stage:
//!   `susToUSC` drops notes on them); the column is `x - 2`. In a USC `lane` is the CENTRE of the note counted from the centre of
//!   the stage and `size` is HALF its width (`susToUSC`: `lane = x - 8 + width / 2`, `size = width / 2`), so the leftmost lane is
//!   `lane - size + 6` (0..=11) and the column is its value rounded to the nearest whole when it is within 1e-4 of one, else
//!   floored (a non-whole position is counted in [`SekaiReport::fractional_lanes`]).
//! - **Wide notes**: a note of width `w` is ONE note, on its LEFTMOST column; the other `w - 1` lanes it covers are not used (the
//!   width is lost). This keeps the note count of the chart (`w` simultaneous notes would multiply it), and matches the game's own
//!   identity of a note on its tick, which is its leftmost lane (the notes say overlaps are decided on it alone). A note that does
//!   not lie entirely on the stage (`x < 2` or `x + width > 14`; USC: left edge outside `0..=11` or right edge above 12) is dropped
//!   and counted ([`SekaiReport::notes_outside_lanes`]).
//!
//! ## Notes
//!
//! - **Taps**: a tap (`1`) and a critical tap (`2`) are [`rox::model::Note::tap`]; the critical flag is dropped and counted
//!   ([`SekaiReport::critical_flags`]). A flick (a tap with a directional overlapped on it; USC: `direction`) is still a tap,
//!   counted in [`SekaiReport::flicks`].
//! - **Slides**: a slide is ONE hold, from its first joint to its last, on the first joint's column. Every joint between them
//!   (visible relay, hidden relay, Bezier control point, attach relay, USC `tick`/`hidden`/`attach`/`ignore`) is dropped
//!   and counted ([`SekaiReport::slide_joints_dropped`]); the path across lanes is lost (a hold is on one column). What the notes
//!   call overlaps on a joint (critical, trace, elaser, ease, attach, flick end) are modifiers, not notes: they are dropped and
//!   counted ([`SekaiReport::slide_modifiers`]). A slide whose first joint is hidden (an elaser on it, USC `ignore`) is still a hold
//!   (the game judges it differently; the model cannot say so). A slide that ends on its first joint's time is a tap
//!   ([`SekaiReport::empty_slides_as_taps`]). The first joint is not a start or the last is not an end: dropped
//!   ([`SekaiReport::unterminated_slides`], [`SekaiReport::slide_points_without_start`]).
//! - **Dropped, never taps**: trace notes and critical traces ([`SekaiReport::traces`]; they are not hit by a tap), damage notes
//!   ([`SekaiReport::damage_notes`]), skill activations ([`SekaiReport::skill_markers`]), fever markers
//!   ([`SekaiReport::fever_markers`]), guide slides ([`SekaiReport::guides`]: SUS `#mmm9xy`, USC `guide` or a `slide` with
//!   `active: false`), and overlays with no note beneath ([`SekaiReport::stray_overlays`]).
//! - **Inside a hold**: a note on the column of a hold, between its start and its end, is dropped ([`SekaiReport::overlapping_notes_dropped`]):
//!   the model forbids it, and a slide of the game does pass over other lanes. Two taps on the same column at the same time: the first
//!   stays ([`SekaiReport::duplicate_taps`]).
//!
//! ## Time
//!
//! - **Beats**: a SUS position is `measure start + length x slot / slots`, in beats; a USC position is its `beat`. The tick of a SUS
//!   (`#REQUEST "ticks_per_beat n"`, 480 when absent or invalid) cancels out of the times, as it does in `susToUSC`: it only
//!   decides which overlapped objects are on the same tick (positions are compared in thousandths of a tick).
//! - **Measures** (SUS): `#mmm02: beats` (decimal, `0 < beats <= 1024`) sets the length from that measure on, until the next one;
//!   4 beats before the first (`sus-js`: 4/4 when none). `#MEASUREBS n` (SUS 2.7) adds `n` to the measure number of every line after
//!   it. The measure of a line is `mmm` + the base, in 0..=1 000 999.
//! - **Tempo**: `#BPMzz: bpm` defines (the id is case-sensitive, as the notes say), `#mmm08: zz...` changes (a slot is an id,
//!   `00` is none). USC: `bpm` objects. Several changes at one beat: the last wins. The BPM before the first change is the first
//!   change's (extended backwards: the format gives none), so a chart whose first change is late is timed from beat 0. A chart with no
//!   valid BPM is an error. A BPM that is not a finite number in `(0, 1e7]` is ignored and counted. `signature` of a timing point is the
//!   measure length (beats, rounded, 1..=255) at the change; 4 for a USC.
//! - **Offset**: times of a ROX chart count from the start of the audio. SUS `#WAVEOFFSET s` (seconds): the notes say a positive
//!   value makes the CHART start first and the audio `s` later, so the audio position of a note is `chart time - s`; USC `offset` is
//!   `-#WAVEOFFSET` (`susToUSC` writes `-WAVEOFFSET`, `rg_converters` writes it back negated), so the same shift is `chart time +
//!   offset`. Every note and tempo point moves by `-s` (= `offset`); a negative result is a negative time. `metadata.audio_offset_us`
//!   stays 0. A time beyond 10^7 seconds is dropped ([`SekaiReport::notes_out_of_range`]).
//! - **Scroll speed** (`#TILzz`, `#HISPEED`, `#BASEBPM`, USC `timeScale`/`timeScaleGroup`) does not change when a note is hit: ignored
//!   and counted ([`SekaiReport::time_scale_changes`]).
//!
//! ## Metadata
//!
//! SUS: `#TITLE`, `#ARTIST`, `#GENRE`, `#DESIGNER` (= creator), `#DIFFICULTY` (kept as written: the notes reserve 0 to 4 and give
//! no names, so `difficulty_name` is the text), `#PLAYLEVEL` (`difficulty_value`; a trailing `+` adds 0.5, the convention of
//! Chunithm's level lists), `#WAVE` (`audio_file`: the notes say a path relative to the `.sus`; EMPTY when absent), `#BACKGROUND`.
//! The title falls back to the file stem ([`SusDecoder::decode_with_title`]; `auto_decode*` pass it). USC: no metadata at all: the
//! title is the file stem, `audio_file` is empty (the BGM is part of a Sonolus level item). The audio is found next to the chart by
//! the caller when `audio_file` is empty or missing.
//!
//! ## Telling a Project SEKAI file from another one
//!
//! `.sus` and `.usc` are generic extensions: the SUS of Chunithm (Seaurchin, ched, Margrete) is the same text format with other notes (holds `#mmm2xy`,
//! air notes, 16 lanes) and nothing in a file's first bytes tells the two dialects apart, so a Chunithm `.sus` is read as a Project SEKAI chart: the
//! lanes outside the stage and the kinds this decoder does not know are dropped and counted, never taps. What the first bytes can tell is whether a file is
//! a SUS or a USC at all: [`looks_like_sus`] (text with a `#` line) and [`looks_like_usc`] (a JSON object with an `"objects"` key), over the first
//! [`SNIFF_BYTES`]. Prism's scan lists a `.sus` or `.usc` only when they say yes, so a file that merely has the extension is no failure.
//!
//! ## Robustness
//!
//! Files above 64 MiB (a `.usc`: 16 MiB, a JSON value takes several times its text in memory), more than 4 million objects, a JSON nesting beyond `serde_json`'s limit, text that is not UTF-8
//! (decoded lossily, flagged in [`SekaiReport::invalid_text`]) and every other malformed input is an error or a counted drop, never
//! a panic. A file with no playable note is an error.
//!
//! Not handled: the generic SUS 2.7 holds (`#mmm2xy`) and slides 2 (`#mmm4xy`) of Chunithm and Seaurchin (counted in
//! [`SekaiReport::unsupported_notes`]), Air notes, hi-speed, the sustained path of a slide, the USC `judgeType`/`ease`/`fake`
//! fields, the SUS dialect of hololive Dream and the MMW editor formats (`.mmws`, `.ccmmws`).
//!
//! What the decoder dropped or repaired is in the [`SekaiReport`] of `decode_with_report`.

mod bdon;
mod score;
mod sniff;
mod sus;
mod usc;

#[cfg(test)]
mod tests;

pub use bdon::{BdonDecoder, looks_like_bdon};
pub use score::{COLUMNS, SekaiReport};
pub use sniff::{SNIFF_BYTES, looks_like_sus, looks_like_usc};
pub use sus::SusDecoder;
pub use usc::UscDecoder;
