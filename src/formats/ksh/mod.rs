#![warn(clippy::pedantic)]
//! `K-Shoot MANIA` charts, `.ksh` ([`KshDecoder`]): the fan charts of SOUND VOLTEX. Decode only.
//!
//! One `.ksh` is ONE difficulty of a song (a song folder holds one file per difficulty: light, challenge, extended, infinite). Format notes:
//! `vsrg-format-docs/vsrg/sound-voltex/ksh` in Prism's repository, checked against the official specification (`kshootmania/ksm-chart-format`,
//! `ksh_format.md`, CC0) and against two readers, `kshootmania/libkson` (`src/IO/KshIOIn.cpp`; MIT) and `Drewol/unnamed-sdvx-clone`
//! (`Beatmap/src/BeatmapFromKSH.cpp`, `KShootMap.cpp`; MIT), read for facts and not copied. Every decision below is pinned by `tests.rs`,
//! whose files are all written by hand (no real chart is in the repository: fan charts have no licence to redistribute them).
//!
//! ## File
//!
//! Text, line by line (a line ends at `\n`, `\r` or `\r\n`; empty lines are ignored; the ends of a line are trimmed). The FIRST bar line `--`
//! ends the HEADER (option lines `name=value`, the value is everything after the first `=`); the BODY is measures of chart lines, each measure
//! ended by a bar line; `#define_fx` and `#define_filter` lines (the footer) are counted and not read. A file with no bar line is refused.
//! Comment lines are `//...`, and the unofficial `;...` the format says to expect. A line with a `|` that is not a chart line is an ERROR (a line
//! lost would move every time after it); any other line the format does not have is dropped and counted ([`KshReport::unknown_lines`]).
//!
//! - **Text**: a byte order mark says UTF-8, else valid UTF-8 is UTF-8 (ASCII included), else `Shift_JIS` (CP932), else GBK (CP936), else
//!   `Shift_JIS` with U+FFFD: the format says old charts (before editor 1.32) are in the locale's code page and that the community tool `rmstZ` tries
//!   CP932 then CP936. The chart lines are ASCII and do not depend on it.
//! - **Header options read**: `title` (the file stem when empty or absent, as `.sus`), `artist`, `effect` (the chart AUTHOR: `creator`), `difficulty`,
//!   `level`, `t`, `beat`, `m`, `o`, `po`, `plength`, `jacket`, `bg`. The last of two lines of one name wins. An option the format has and this decoder
//!   does not read (`mvol`, `layer`, `ver`, ...) is ignored, a name it does not have is counted ([`KshReport::unknown_options`]).
//!
//! ## Columns
//!
//! `key_count` is 6, always: columns 0 to 3 are the BT buttons A, B, C, D, columns 4 and 5 the FX buttons FX-L and FX-R: the order of a chart line
//! (`bbbb|ff|ll`), which is also the cabinet's: four BT buttons side by side and the two FX buttons under them, left then right. (The highway shows
//! the FX lanes wide, under two BT lanes each, which a single row of columns can not show; the BT buttons keep their four columns.) The lasers
//! are analog (two knobs that sweep a position between 51 steps) and slide, not press: they are NOT a column. The catalog's layouts have 4 to 9
//! columns and 6 is one of them, so a `.ksh` is 6K and playable.
//!
//! ## Notes
//!
//! A chart line is `bbbb|ff|ll`, then an optional lane spin. BT characters (`0` empty, `1` chip, `2` long: the format keeps BT and FX
//! digits OPPOSITE on purpose) and FX characters (`0` empty, `2` chip, any other character long):
//!
//! | lane | character | ROX |
//! |---|---|---|
//! | BT | `1` | tap |
//! | BT | `2` | hold: the run of `2` of the lane |
//! | BT | any other except `0` | empty ([`KshReport::unknown_buttons`]) |
//! | FX | `2` | tap |
//! | FX | `0` | empty |
//! | FX | any other | hold: the run of such characters of the lane |
//!
//! A hold is a RUN: it starts at the line of its first character and ends at the line where the run stops (the first line with another
//! character, a chip included, or a `0`), across bar lines: a `2` is "a continuation of the previous note", so a single `2` is a hold one line
//! long, not a chip. A run still going at the end of the last measure ends there ([`KshReport::holds_open_at_end`]). The legacy letters of an
//! FX hold (`S`, `G`, `F`, ..., from before v1.60: each named an audio effect) and the `fx-l`/`fx-r` options of the newer files only choose an
//! audio effect: they are dropped and counted ([`KshReport::fx_legacy_effect_holds`], [`KshReport::fx_options`]), and a run of mixed letters is
//! ONE hold (the format: "continuing while the next lines keep a non-`0`/`2` character").
//!
//! ## Lasers: dropped
//!
//! **Lasers are NOT supported for now** (owner decision 2026-10-29: no laser model in ROX, nothing played by Prism): the chart is the buttons' half of the game.
//!
//! Each laser lane is a character per line: `-` none, `:` a linear connection, else one of 51 positions (`0123456789ABCD...lmno`). A run of
//! them is a laser note, two positions 1/32 of a whole note or less apart are a slam (an instant move), a suffix `@(`, `@)`, `@<`, `@>`, `S<`,
//! `S>` after the two characters is a lane spin, half spin or swing, and `laserrange_*` makes a laser wide. Nothing of it is a press, so all of it is
//! dropped and counted: [`KshReport::laser_notes`], [`KshReport::laser_slams`], [`KshReport::lane_spins`], [`KshReport::half_spins`],
//! [`KshReport::swings`], [`KshReport::laser_options`]. (Alternatives considered and rejected: a column per laser with a tap at each
//! position change, which would turn a sweep into a stream of presses nobody plays; and a hold per laser note, which a knob does not hold. The
//! chart without lasers is the buttons' half of the game, and that is what the columns are.)
//!
//! ## Time
//!
//! - **Lines**: a measure of `n` chart lines is cut in `n` EQUAL parts and line `i` (from 0) is at `i / n` of it (the format: "the lines divide the
//!   measure evenly"; no rounding to a grid: `libkson` and `unnamed-sdvx-clone` both round to pulses, a source of drift this decoder has not).
//!   A measure with no line is as long as it says and holds no note ([`KshReport::empty_measures`]; `libkson` does the same, `unnamed-sdvx-clone`
//!   stops reading there).
//! - **Length of a measure**: `beat=n/d` (4/4 when absent) is `n` beats of a `1/d` note: `4 n / d` quarter notes. Both must be whole numbers above
//!   0, else the option is ignored and counted ([`KshReport::invalid_time_signatures`]). The format says a `beat` that is not "just after the bar
//!   line" applies to the NEXT measure, and so it does here (a `beat` written before the first line of a measure, after the bar line or at the
//!   end of the measure before, is for that measure). The model keeps the signature as quarter notes, rounded (as the `.chart` decoder does).
//! - **Tempo**: the header `t` (120 when absent) is the tempo at the start; a body `t=` applies FROM THE NEXT CHART LINE, in the middle of
//!   a measure too: the lines before it last at the old tempo. Options "take effect at the next chart line", also across a bar line. A tempo
//!   must be a finite number in `(0, 1e7]`: a range (`t=120-180`) in the body, a negative number (the community's stop convention of `rmstZ`,
//!   which the format does not have), zero or NaN is ignored and counted ([`KshReport::invalid_bpms`]). The header `t` of a chart with tempo
//!   changes is a range string (`180-220`, for the song list): the real tempo is the first `t=` just after the first bar line, and the start of
//!   the range is the fallback ([`KshReport::header_bpm_range`]).
//! - **Offset**: `o` is "the starting point of the song audio" in ms, an integer that may be negative: "setting a larger value will make note
//!   judgments later". The first measure starts at `o` ms of the audio (`unnamed-sdvx-clone` puts its first timing point at `o`), so EVERY note and
//!   timing point is moved by `+o`, as `.sm` bakes `#OFFSET`; `audio_offset_us` is 0. A value that is not an integer within +-1e9 is the default
//!   0, counted ([`KshReport::invalid_option_values`]).
//! - **`stop`**: NOT a pause of time. The format says it is "equivalent to setting `scroll_speed` to 0 for a duration": the notes stop SCROLLING
//!   (a hi-speed effect) while the song and the judgment go on, and `libkson` bakes it into the scroll-speed graph, never into the tempo.
//!   So it moves no time and is only counted ([`KshReport::stops`]), as `scroll_speed`, `tilt`, `zoom_*`, `center_split`, `rotation_deg` and
//!   the curves are ([`KshReport::scroll_options`]). (A `.sm` or `.bms` stop does stop time: that is not this.)
//! - **BPM points**: one timing point at the start of the first measure, then one at each tempo or signature change (two at the same
//!   microsecond: the last wins; a point that repeats the one before is not written).
//!
//! ## Metadata
//!
//! - `difficulty_name` is `Light` (the default), `Challenge`, `Extended` or `Infinite`; any other word is `Infinite`, as the format says
//!   ([`KshReport::unknown_difficulty`]). `difficulty_value` is `level` (1 to 20, 1 when absent or out of range).
//! - `audio_file` is the FIRST file of `m` (`song.ogg;song_f.ogg` or four files: the others are the same song with the effects pre-rendered
//!   and are counted, [`KshReport::extra_audio_files`], not read), as written, `\` read as `/`; a name that is absolute or climbs out of the
//!   folder (`..`) is refused ([`KshReport::unsafe_names`]) and `audio_file` is empty. The caller finds the file in the chart's folder.
//! - `background_file` is the first file of `bg` when it is not one of the 13 presets (`desert`, `grass`, ...), else the `jacket` when it is
//!   not one of the 3 presets (`nowprinting1` to `3`); a name is checked as `audio_file` is.
//! - `preview_time_us` and `preview_duration_us` are `po` and `plength` (ms of the audio, so the offset does not move them).
//!
//! ## Telling a `.ksh` of K-Shoot MANIA from another file
//!
//! `.ksh` is ALSO the extension of Korn shell scripts. [`looks_like_ksh`] (the first [`crate::formats::SNIFF_BYTES`] of the file) says yes only when
//! the first line is `title=...` (the format requires it to be first), every line up to the first bar line `--` is an option or a `//` comment,
//! and that bar line is there; Prism's scan lists a `.ksh` only then, so a script is no map and no failure.
//!
//! Not handled: `.kson` (the JSON successor) and `.vox` (the arcade's own format), course files, the lasers, the audio effects and key sounds, the
//! camera, scroll, tilt and `stop` events, the background and layer animations, `mvol` and the other volumes, the `ver` behaviours (the
//! `t` limit, the `mvol` factor): none moves a button.
//!
//! What the decoder dropped or simplified is in the [`KshReport`] of `decode_with_report`.

mod decoder;
mod sniff;

#[cfg(test)]
mod tests;

pub use decoder::{COLUMNS, KshDecoder, KshReport};
pub use sniff::{is_ksh_chart_file, looks_like_ksh};
