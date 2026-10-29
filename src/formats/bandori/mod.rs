#![warn(clippy::pedantic)]
//! BanG Dream! Girls Band Party lane charts: the Bestdori chart JSON ([`BestdoriDecoder`]) and the BanG! Simulator `.txt`
//! ([`SimulatorDecoder`]). Decode only.
//!
//! Both describe one chart per file on the 7 lanes of the game. Format notes: `vsrg-format-docs/vsrg/bang-dream/{bestdori-json,txt}` in
//! Prism's repository, cross-checked against
//!
//! - `ayachan` (6QHTSK; MIT; commit e9647d8): `model/bestdoriChart/bestdoriV2.go` (the fields of a note and of a connection, the sorting, what
//!   `ChartCheck` requires, what `DecodeToAyachan` drops: notes before the first BPM, lanes clamped, a connection list of length 1 is a
//!   single, of length 0 is invalid, the absolute value of a BPM);
//! - `BanGConverter` (loader3229; MIT; commit c44de0e): `in/bestdori.js` (Bestdori to simulator: lane + 1, the BPM at beat 0 is the
//!   simulator's BPM line and 120 when there is none, the flags `skill`/`flick` and the type numbers they become, directional types
//!   `50 + width + 10 x (Right)`, the long note 21/25, the slide chains A and B and the numbered chain 71 to 77), `out/bestdori.js` (the
//!   simulator to Bestdori, a block that is commented out: a long note is paired by its LANE, a slide by its chain), `out/bangsim.js` and
//!   `converter.js` (`offsetter`: only the notes move, the first three lines stay, and the download is CRLF);
//! - the simulator's own notes `Bang info.txt` (thtl1999/BanG-simulator, commit 1aedf90; the repository has NO licence file: read for the format
//!   facts, nothing copied), which is the only specification of the `.txt`: the line layout, the note types 0 to 67, the example;
//! - `bestdori-api-go` (`WindowsSov8forUs`; MIT; commit 4e17aab): `bestdori/dto/charts.go` (the fields `skill`, `charge`, `hidden`, `flick` of a
//!   note and of a connection, the `Directional` direction values) and `bestdori/endpoints/api.go` (`/api/charts/<id>/<difficulty>.json`);
//! - `bangbangboom-editor` (K024; MIT; commit bf08bee): `src/MapFormats/bestdori.ts` (a Bestdori note's `lane` is "from 0 to 6", a slide is
//!   `connections` of `{beat, lane, flick?}`, a BPM is `{type, beat, bpm}`).
//!
//! Every decision below is pinned by `tests.rs`, whose files are all written by hand (no real chart is in the repository: community charts
//! have no licence to redistribute them, and no real chart was decoded while writing this).
//!
//! ## Lanes and columns
//!
//! - **Columns**: ALWAYS 7 (`key_count = 7`), one per lane of the game, left to right. A Bestdori `lane` is 0 to 6 (`bangbangboom` documents
//!   "from 0 to 6" and `BanGConverter` writes `lane + 1` into the simulator), a simulator lane is 1 to 7 (`8/1/3`: "the 3rd lane"); the column
//!   is the lane minus the first. A lane within 1e-4 of a whole number is that number; a lane that is not a whole number is dropped
//!   ([`BandoriReport::fractional_lanes`]) and a whole one outside the 7 is dropped ([`BandoriReport::lanes_out_of_range`]). `ayachan` clamps
//!   a lane into 0..=7 instead; a note on a lane the game does not have is a note the player can not hit, so this decoder does not move it.
//!
//! ## Notes
//!
//! - **Taps**: a `Single` (simulator type 1) is [`rox::model::Note::tap`]. A flick (`flick: true`; type 2) is still a tap, counted in
//!   [`BandoriReport::flicks`]. A `Directional` (types 51 to 57 left, 61 to 67 right, width 1 to 7) is a tap on its lane: the direction and
//!   the width are dropped, counted in [`BandoriReport::directional_flicks`] (the `direction` and `width` values are not validated: `ayachan`
//!   accepts a width of 0 to 3, the simulator 1 to 7). The skill flag (`skill`; types 11, 31 to 36, 75, 76) and the fever charge flag
//!   (`charge`; type 10) mark a note that is played like any other: they are counted ([`BandoriReport::skill_flags`],
//!   [`BandoriReport::charge_flags`]) and dropped, the note stays. Flags are counted for the notes that are read, a slide or a note dropped
//!   later still counted them.
//! - **Long notes and slides**: a `Long` or a `Slide` (Bestdori: `connections`, the first is the head and the last the release; the
//!   top-level `beat` and `lane` are not used; simulator: a long note `21`/`31` .. `25`/`26`/`32` paired by its lane, a slide of chain A
//!   `3`/`33` .. `4`/`41` .. `5`/`12`/`34`, of chain B `6`/`35` .. `7`/`42` .. `8`/`13`/`36`, a numbered one `71`/`75` .. `72`/`77` ..
//!   `73`/`74`/`76` with its number in the second field) is ONE hold from its first joint to its last, on the FIRST joint's column. LOSSY:
//!   every joint between them (a relay, a hidden one) is dropped ([`BandoriReport::slide_joints_dropped`]) and the lanes the slide goes
//!   through are lost (a hold is on one column; [`BandoriReport::slides_with_lane_changes`] counts the slides that leave their lane). A
//!   flick on the last joint is counted. A connection list with one joint is a tap ([`BandoriReport::single_connection_slides`], `ayachan`);
//!   a slide whose last joint is on its first joint's time is a tap ([`BandoriReport::empty_slides_as_taps`]); one with no connection, a
//!   connection that has no `beat` or `lane`, or that goes back in time is dropped ([`BandoriReport::invalid_slides`]); in a simulator
//!   score a start before the previous one ended, the end of the file before an end ([`BandoriReport::unterminated_slides`]) and a joint or an
//!   end with no start ([`BandoriReport::slide_points_without_start`]) are dropped. The simulator reads its lines sorted by beat, file order
//!   at one beat (`BanGConverter` sorts them the same way; "notes at the same time should be sequentially ordered").
//! - **Inside a hold**: a note on the column of a hold, between its start and its end, is dropped
//!   ([`BandoriReport::overlapping_notes_dropped`]): the model forbids it, and a slide of the game does pass over other lanes. Two taps on
//!   the same column at the same time: the first stays ([`BandoriReport::duplicate_taps`]).
//! - **Unknown**: a `type` this decoder does not know (a Bestdori v1 chart is `System` and `Note` elements: it is not read) or a simulator
//!   type it does not know is counted ([`BandoriReport::unknown_types`]); an element that is not an object
//!   ([`BandoriReport::invalid_objects`]) and a note with a field missing or not a number or a simulator line that is not `beat/type/lane`
//!   ([`BandoriReport::invalid_notes`]) too.
//!
//! ## Time
//!
//! - **Beats**: a Bestdori `beat` and a simulator beat are in beats (quarter notes), real numbers. The time of a beat is the sum of
//!   `beats x 60 / bpm` over the tempo segments (fiche).
//! - **Tempo**: a Bestdori `BPM` element, a simulator `beat/20/bpm` line (the BPM is in the lane field). The BPM at beat 0 is the LAST
//!   event at or before beat 0 (`BanGConverter` takes the first one in the file as the simulator's BPM line; the simulator's own second line
//!   is such an event at beat 0), and the later events change it at their beat; the last of several at one beat wins. A chart with no BPM
//!   at or before beat 0 is at 120 BPM until its first event (`BanGConverter` writes 120 as the simulator's BPM line in that case).
//!   `ayachan` instead moves the first BPM to beat 0 and ignores every note before it; this decoder does not shift anything. A BPM that is
//!   not a finite number in `(0, 1e7]` (its absolute value, for Bestdori: [`BandoriReport::negative_bpms`]) is ignored and counted
//!   ([`BandoriReport::invalid_bpm`]). The signature of every timing point is 4 (the formats have no measures).
//! - **Offset**: times of a ROX chart count from the start of the audio. A Bestdori chart has no offset (the audio's is the site's
//!   business, fiche). The simulator's first line, the "note offset" (46 in every file `BanGConverter` writes), is READ and NOT APPLIED
//!   ([`BandoriReport::note_offset`]): the notes give no unit (the fiche infers milliseconds) and no sign, and it reads as the simulator's
//!   calibration of its latency (the `Offset` of its `setting.ini` has the same default), not as a fact about the chart. `0/0/0` ("music
//!   starts at beat 0") says at which beat the audio starts: the first such line sets the origin of the times, every beat before it is before
//!   the audio (a later one is ignored, [`BandoriReport::extra_music_starts`]). `metadata.audio_offset_us` stays 0.
//! - **Before the audio**: a note (or the head of a slide) before beat 0 (a Bestdori chart) or before the simulator's `0/0/0` has a negative
//!   time, so it can not be played against the audio: it is dropped and counted ([`BandoriReport::notes_before_start`]); `ayachan` drops them
//!   too. A time beyond 10^7 seconds is dropped ([`BandoriReport::notes_out_of_range`]).
//!
//! ## Metadata
//!
//! None in either file: the title is the file stem (a trailing `.bestdori` removed, so `Song.bestdori.json` is "Song"), the artist, creator
//! and difficulty are the model's defaults, `audio_file` is EMPTY. The caller finds the audio next to the chart (Prism: by the chart's name,
//! then the folder's audio file).
//!
//! ## Telling a BanG Dream file from another one
//!
//! `.json` and `.txt` are the two most generic extensions there are, so ROX does NOT claim them: [`detect`](crate::formats::auto::detect::detect)
//! gives `.json` to FNF and `.txt` to nothing, as before, and says `Bestdori` only for the names no other format uses (`.bestdori`,
//! `.bestdori.json`). `auto_decode*` reads a file as BanG Dream only when the content tells it: a simulator score for a `.txt`
//! ([`looks_like_simulator`]: a number, a positive number, then nothing but `beat/type/lane` lines over the first 4 KiB), a Bestdori chart
//! for a file with a Bestdori name ([`is_bestdori_chart_name`]: `.bestdori`, `.bestdori.json`, or the name the site's API gives a chart,
//! `<difficulty>.json` / `<id>.<difficulty>.json`; the notes give no convention, so this is the decision) that [`looks_like_bestdori`] (a JSON
//! array that starts with an object and names a note type). A legacy FNF chart, a `package.json`, a `.jrox` or a `.usc` is an object, so
//! none of them can pass; a random `.txt` or `.json` fails the grammar. [`is_bandori_chart_file`] is that rule applied to a file on disk,
//! for the callers that list files.
//!
//! ## Robustness
//!
//! Files above 16 MiB, more than 4 million elements or lines, a JSON nesting beyond `serde_json`'s limit, text that is not UTF-8 (a
//! simulator score is read lossily, flagged in [`BandoriReport::invalid_text`]), NaN or infinite numbers and every other malformed input is
//! an error or a counted drop, never a panic. A file with no playable note is an error.
//!
//! Not handled: the Bestdori v1 JSON (`System`/`Note` elements), the BDON chart of BanG Dream! Our Notes (a board of 24 units with free
//! positions and widths, the fiche is partial and defines no 7-lane reading), the hit offset, hi-speed.
//!
//! What the decoder dropped or repaired is in the [`BandoriReport`] of `decode_with_report`.

mod bestdori;
mod score;
mod simulator;
mod sniff;

#[cfg(test)]
mod tests;

use std::path::Path;

pub use bestdori::BestdoriDecoder;
pub use score::{BandoriReport, COLUMNS};
pub use simulator::SimulatorDecoder;
pub use sniff::{
    is_bandori_chart_file, is_bestdori_chart_name, looks_like_bestdori, looks_like_simulator,
};

/// The title of a chart: the file stem, without a trailing `.bestdori` (`Song.bestdori.json` is "Song").
pub(crate) fn title_of(path: &Path) -> String {
    let stem = crate::formats::ssq::decoder::title_of(path);
    match stem.strip_suffix(".bestdori") {
        Some(title) if !title.is_empty() => title.to_owned(),
        _ => stem,
    }
}
