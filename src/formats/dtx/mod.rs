//! `DTXMania`: `.dtx` charts ([`DtxDecoder`]). Decode only.
//!
//! One `.dtx` is one chart of `DTXMania` (a free GITADORA simulator) and holds up to three instruments at once: drums,
//! guitar and bass. Each instrument that has chips is a variant ([`DtxDecoder::variants`]), numbered like the `.ojn` and
//! `.ssq` ones: `Drums`, `Guitar`, `Bass`, in that order; an instrument with no chip is not a variant. The file is a BMS
//! dialect, but the BMS parser does not read it (`#NAME: value` with a colon, a base-36 measure number, hexadecimal
//! channels), so `parser.rs` is its own; the beat and time code (`bms::timing`) and the charset rule (`bms::source`) are the
//! BMS ones. Format notes: `vsrg-format-docs/vsrg/dtxmania/{dtx,set-def}` in Prism's repository, checked against
//! limyz/DTXmaniaNX (`CDTX.cs`, `CChip.cs`, `EChannel.cs`, `CConversion.cs`, `CStagePerfCommonScreen.cs`; its
//! `Runtime/Licenses/dtxmania.txt` is a permissive licence of the MIT kind; read, not copied).
//!
//! Decisions (the format leaves them to the player; the unit tests pin each of them):
//!
//! - **Text**: byte order mark, else valid UTF-8, else `Shift_JIS`, else EUC-KR (`bms::source`; the game itself always reads
//!   `Shift_JIS`). Lines end at `\n`, `\r` or `\r\n`; a line that does not start with `#` is a comment; `;` starts a comment inside a
//!   line; names are case-insensitive and match by prefix, so `#TITLEfoo` is a title (see `parser.rs`).
//! - **Time**: a measure is 384 ticks and an object `i` of `n` sits at tick `384 * i / n`, INTEGER division, as in the game (a
//!   quintuplet is rounded to the tick). The length of a measure (channel `02`, a real number) is kept from the measure that sets it
//!   until the next channel-`02` line (BMS: that measure only). The tempo is `#BPM` (`#BPM00`, the FIRST one of the file), 120 when
//!   absent; channel `03` is `#BASEBPM` plus a hexadecimal pair; channel `08` is the `#BPMzz` of its object plus `#BASEBPM` (except
//!   `#BPM00`); `#BASEBPM` is the last one of the file. A BPM that is not positive is ignored (counted). There is no STOP. Chart
//!   time 0 is the start of measure `000` (the game puts one empty measure before it: the same shift for every sound, not
//!   part of the chart). `#DTXVPLAYSPEED` (a speed of `DTXViewer`) is not read, and counted.
//! - **Tables**: `#WAVzz`, `#VOLUMEzz` (`#WAVVOLzz`) and `#BPMzz` may be used before they are defined and defined again. A chip
//!   uses the definition read last BEFORE its line, else the first one of the file. A `#VOLUMEzz` applies to the `#WAVzz` it
//!   precedes (else to the ones before its first line). `#PANzz` is not used. File names go through `#PATH` (concatenated, no
//!   separator added, as the game does), `\` becomes `/`, and a name that is absolute or climbs out of the folder reads as undefined
//!   (counted). `#PATH_WAV` is a folder of the author's machine: not read, counted. `#RANDOM` always draws 1, the value is 0 before
//!   it, `#IF` keeps its block when the draw equals it, no `#ELSE`.
//! - **Drums, channels** `11` hi-hat, `12` snare, `13` bass drum, `14` high tom, `15` low tom, `16` cymbal (right crash), `17`
//!   floor tom, `18` open hi-hat, `19` ride, `1A` left cymbal, `1B` left pedal, `1C` left bass drum. `DTXMania` plays them on TEN
//!   lanes (`nチャンネル0Atoレーン07` of `CStagePerfCommonScreen.cs`): LC, HH (closed and open share it), LP (left pedal and left
//!   bass drum share it), SD, HT, BD, LT, FT, CY, RD (the on-screen order, from the bonus-lane switch of `CDTX.cs`). The catalog has no
//!   ten-column layout (4 to 9), so this decoder FOLDS the left pedal lane onto the bass drum: the columns are, left to right,
//!   `LC HH SD HT BD LT FT CY RD`, and a column exists only when the chart has a chip for it, so a drum chart has from 1 to 9
//!   columns, 4 to 9 playable (a chart that needs fewer is refused by the library scan, not here). Notes of the open hi-hat and
//!   of the left pedal / left bass drum folds are counted ([`DtxReport::hi_hat_open_notes`], `left_pedal_notes_folded`); two
//!   notes of one column at one time (the two pedals, a line written twice) keep the first and count the other.
//! - **Guitar and bass**: the channel is a CHORD of buttons (`21`..`27`, `A1`..`A7`: red 4, green 2, blue 1; the XG chords add yellow
//!   `93`..`9A`, pink `9B`..`9F`, `A9`..`AB`, and both `AC`..`AF`, `D0`..`D3` for the guitar, `C5` `C6` `C8`..`CF`, `DA`..`DF`, `E1`..`E8` for
//!   the bass), `20` and `A0` the open pick. A chord is one note per button, at one time. Columns, left to right: `Open R G B`, and
//!   `Y P` after them when any chip of the instrument uses yellow or pink: 4 or 6 columns whatever the chart uses (an empty open
//!   column is the price of a chart that is always playable). Wailing (`28`, `A8`, `2F`) is counted and dropped. A hold is a marker
//!   on channel `2C` (guitar) or `2D` (bass), paired as the game does it (see `decoder.rs`): the value of the marker is not read.
//! - **Dropped and counted** (named counters of [`DtxReport`]): hidden drum chips `31`..`3C` (they make no sound), blank-hit
//!   sound settings `B1`..`BE`, fill-in markers `1F` and `53`, pictures, videos, bar and beat lines (`04 07 50 51 54`..`5A 60 C1 C2
//!   C4 C7 D5`..`D9 E0`), channels that change nothing here (bonus, MIDI chorus, click, mixer, flow speed, reserved), and every
//!   channel the table does not list. The chips of the other two instruments are counted for the chart that does not play them.
//! - **Sounds**: a chip's value is the `#WAVzz` it plays. The AUDIO PLAN holds the background sounds (channel `01` and the effect
//!   channels `61`..`69`, `70`..`79`, `80`..`89`, `90`..`92`, all played by themselves; the "muting" effect channels are not
//!   muted) and the sound of EVERY chip of the chosen instrument, also the ones the chart drops (the folded pedal, a note inside a
//!   hold), at chart time, with `#VOLUMEzz` as the event volume; the chips of the other instruments make none (the game plays
//!   none of them either, `CStagePerfDrumsScreen.cs`). Each note's sound is also in the chart's hitsound table, as BMS does.
//! - **Audio file**: when the file has exactly ONE background sound and the instrument's chips make no sound at all, that
//!   sound is the chart's `audio_file` and the time origin (every time moves back to its position), the BMS rule. Otherwise
//!   `audio_file` is empty and the plan is the audio, to be mixed: a `DTXMania` song is nearly always one backing track on
//!   channel `01` PLUS one sample per drum chip, so applying the BMS rule to the background sound alone would play the song without
//!   its drums.
//! - **Metadata**: `#TITLE`, `#ARTIST`, `#GENRE`; the difficulty name is the instrument (`Drums`, `Guitar`, `Bass`) and its value
//!   the level as the game shows it: `#DLEVEL` (`#PLAYLEVEL`), `#GLEVEL`, `#BLEVEL` clamped to 0..=1000, shown as tenths (`76` is
//!   7.6; from 100 on, the tens are the tenths and the last digit the hundredths: `576` is 5.76) plus `#DLVDEC`, `#GLVDEC`, `#BLVDEC`
//!   in hundredths; 0 is no value. The background is `#BACKGROUND` (`#WALL`), else `#BACKGROUND_GR` (first for guitar and bass),
//!   else `#STAGEFILE`, else `#PREIMAGE`. `#PREVIEW` is a sound file, not a time range: not read. `set.def` (the list of a folder's
//!   charts, with a title and a label per difficulty) is not read.
//!
//! Not handled: `.gda` and `.g2d` (the older formats), the `#RANDOM` dice, the per-sample pan, the muting effect channels, the
//! pictures and videos, the fill-in and bonus chips.
//!
//! What the decoder dropped or repaired is in the [`DtxReport`] of `decode_with_report`.

mod channels;
pub mod decoder;
mod parser;

#[cfg(test)]
mod tests;

pub use decoder::{DtxDecoder, DtxReport, Part, Variant};
