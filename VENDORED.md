# rhythm-open-exchange (vendored)

- Upstream: https://github.com/Glubus/rhythm-open-exchange (MIT, the owner's own crate), release 0.8.1 from crates.io
  (`https://crates.io/api/v1/crates/rhythm-open-exchange/0.8.1/download`). The library is `rox` (`[lib] name = "rox"` in the manifest).
- Copied as published, with the ORIGINAL manifest (`Cargo.toml.orig` of the crates.io package, here `Cargo.toml`): the generated
  `Cargo.toml`, `Cargo.lock`, `.cargo_vcs_info.json` and the upstream `.github/` folder were left out. The upstream code was not changed; Prism's additions are listed in "Prism's additions" below.
  The package name and version are unchanged on purpose: the root workspace and the site's workspace (`services/api/Cargo.toml`)
  replace the crates.io crate with this folder through `[patch.crates-io]`, so EVERY dependent uses it (the game, `ratings`,
  `game-core`, the game server, `ingest`, the rating service, the site) and they all read charts with the same code.
  The four site and game-server Dockerfiles (`services/api`, `services/ingest`, `services/rating`, `services/gameserver`) copy this
  folder, and their `Dockerfile.dockerignore` allow it.

## Why it is vendored

Owner decision (2026-10-28), lifting the earlier "never re-vendor ROX" rule. Metron stays a crates.io dependency.
The workspace dependency keeps its name and its features: `rox = { package = "rhythm-open-exchange", version = "0.8.1",
default-features = false, features = ["formats"] }`.

## Vérifications

- `cargo check -p game-core -p ratings -p library` (racine) : réussi. `cargo tree -i rhythm-open-exchange` résout vers `vendor/rhythm-open-exchange`.
- `cargo check -p map_index` (`services/api`) : réussi. Même résolution vers `vendor/rhythm-open-exchange`.
- Avertissement préexistant, hors périmètre : champ `failures` jamais lu dans `crates/library/src/scan.rs`.

## Prism's additions

The package name and version stay `rhythm-open-exchange` 0.8.1 on purpose (the `[patch.crates-io]` of both workspaces). Everything below is
Prism's, not upstream's, and must be kept when ROX is updated from crates.io (owner decision 2026-10-28: ROX changes are made in this copy).

- **BMS family decoder** (2026-10-28): `src/formats/bms/` (`mod.rs` decisions, `source.rs` charset, `parser.rs` lexing and `#RANDOM`, `timing.rs`
  beats and seconds, `decoder.rs` lanes, long notes, layout, `tests.rs`), `DetectedFormat::Bms`/`Pms` in `src/formats/auto/detect.rs`, the dispatch
  arms in `src/formats/auto/dispatch.rs`, the exports in `src/formats/mod.rs`, the optional dependency `encoding_rs` 0.8 (in the `formats` feature)
  in `Cargo.toml`, the audio plan (`src/audio_plan.rs`, `decode_with_audio_plan` in `bms/decoder.rs` and `bms/bmson.rs`), the `.bmson` decoder `src/formats/bms/bmson.rs` (+ `bmson_tests.rs`, `DetectedFormat::Bmson`), the test charts `assets/bms/features.bms`, `assets/bms/nine.pms` and `assets/bms/example.bmson` (hand-written, public domain), README and CHANGELOG entries.
  Decode only. Cross-checked against Bemuse's `bms-js` (MIT) and `bms-rs` (Apache-2.0), read but not copied; the BMS spec used is the hitkey
  "BMS command memo". Details: `vendor/vsrg-format-docs/vsrg/bms/`.
- **O2Jam decoder** (2026-10-28): `src/formats/o2jam/` (`mod.rs` decisions, `ojn.rs` header and packages, `ojm.rs` sample container M30/OMC/OJM,
  `decoder.rs` chart, variants and audio plan, `tests.rs` with every file built in memory), `DetectedFormat::Ojn` and the `.ojn` arm of `auto_decode_variant`
  (`src/formats/auto/`), `SoundEvent::index` / `SoundEvent::in_container` in `src/audio_plan.rs` (a sample inside a container file; `bms/bmson.rs` sets
  `None`), `bms::bmson::safe_name` made `pub(crate)` for the `.ojm` name. Decode only. Written from `vendor/vsrg-format-docs/vsrg/o2jam/{ojn,ojm}` and
  cross-checked against Open2Jam (`OJNParser`, `OJMParser`, `EventList`, the renderer's timing; Artistic License 2.0), Raindrop (`NoteLoaderOJN`) and CXO2
  (`OmcArchive`, no licence file: read for comparison only), none of it copied. The 290-entry rearrangement table of the OMC scramble is a constant of the
  format: Open2Jam's and CXO2's copies were compared and are identical, it is the only data taken from them. No `.ojm` file exists in the repository: the
  containers are tested on files built in memory (hand-computed vectors and round trips); the ten real `.ojn` of `vsrg-format-docs` were used locally only.
- **FNF V-Slice decoder** (2026-10-28): `src/formats/fnf_vslice/` (`mod.rs` decisions, `data.rs` JSON types, versions and tempo map, `decoder.rs` chart, variants and report,
  `audio.rs` audio plan, `fnfc.rs` package, `files.rs` file names, `tests.rs` with every file built in memory), `DetectedFormat::Fnfc` and the V-Slice routing of
  `auto_decode*` (`src/formats/auto/`), the exports in `src/formats/mod.rs`, the optional dependency `zip` (`>=4.6, <9`, `deflate-flate2-zlib-rs`, in the `formats` feature:
  the root workspace resolves it to 8, `services/api` to 4; licence MIT) in `Cargo.toml`, and in the legacy `src/formats/fnf/` the `format` field and the `psych_v1` lane
  rule. Decode only. Written from `vendor/vsrg-format-docs/vsrg/friday-night-funkin/{vslice-chart-json,vslice-metadata-json,fnfc}` and checked against FunkinCrew/Funkin
  (`SongData.hx`, `SongRegistry.hx`, `Conductor.hx`, `Song.hx`, `VoicesGroup.hx`, `PlayState.hx`, `Paths.hx`; Apache-2.0, read not copied) and ShadowMario/FNF-PsychEngine
  (`Song.hx`, `PlayState.hx`; Apache-2.0, read not copied). No real file is in the repository: the game's songs (Funkin.assets) are proprietary, so tests use charts and zips
  built in memory; the 61 chart files of Funkin.assets (168 difficulties) were decoded locally only.
- **Clone Hero `.mid` decoder** (2026-10-28): `src/formats/clone_hero/smf.rs` (Standard MIDI File reader), `mid.rs` (`MidDecoder`, tracks, notes, tempo map; shares `settle_notes`, `audio_shift` and `metadata` with
  `decoder.rs`, which was split for it), `mid_tests.rs` (every file built byte by byte), `DetectedFormat::CloneHeroMid` (matched by the file NAME `notes.mid`, `src/formats/auto/detect.rs`) and its arms in
  `src/formats/auto/dispatch.rs`, `SngPackage` reading `notes.mid` (`sng.rs`), the exports in `src/formats/mod.rs`. Decode only. Written from `vendor/vsrg-format-docs/vsrg/clone-hero/mid` and checked against
  TheNathannator/GuitarGame_ChartFormats (CC0; commit 3d36405, `docs/Chart-File-Formats/mid-format/`) and the `.mid` reader of Moonscraper kept in YARC-Official/YARG.Core (commit 9dafd34,
  `MoonscraperChartParser/IO/Midi/MidReader*.cs`, `PhaseShiftSysEx.cs`, `MidIOHelper.cs`, `SongEntry.IniBase.cs`, `CacheHandler.cs`: the tempo map from the first chunk, the track names, the key map, the
  pairing of notes, the `resolution / 3` sustain cutoff and the `sustain_cutoff_threshold` override, `[ENHANCED_OPENS]`, the Phase Shift SysEx, the order `notes.mid` / `notes.midi` / `notes.chart`).
  Licences read: YARG.Core is LGPL-3.0 (its `README.md` and `LICENSE`; the Moonscraper parser inside it is BSD-3-Clause, `MoonscraperChartParser/LICENSE`), read not copied, no code was ported. NOTE: the
  older Clone Hero entry below says YARG.Core is MIT; its `LICENSE` is the LGPL-3.0. No real `.mid` is in the repository (community charts have no licence): every test file is built in memory, and no
  real chart was decoded.
- **Clone Hero decoders** (2026-10-28): `src/formats/clone_hero/` (`mod.rs` decisions, `chart.rs` text sections, `[Song]`, `[SyncTrack]` and the tempo map, `ini.rs` `song.ini`,
- **`.sng` dispatch and `auto_decode_all`** (2026-10-28): `DetectedFormat::Sng` (`src/formats/auto/detect.rs`), the `.sng` arms and `auto_decode_all` / `FileVariant` in `src/formats/auto/dispatch.rs` (a scan decodes every difficulty of a multi-chart file from one read), exported by `src/formats/mod.rs`; test `auto_decode_routes_a_sng_by_its_path_and_lists_every_variant` in `clone_hero/tests.rs`. `cargo test` of the crate: 386 + 1 doc test pass.
  `decoder.rs` tracks, variants, report and metadata, `plan.rs` audio plan, `sng.rs` `.sng` package, `tests.rs` with every file built in memory), `DetectedFormat::CloneHero` and its
  routing in `auto_decode*` (`src/formats/auto/`, which read the `song.ini` of the folder), the exports in `src/formats/mod.rs`. Decode only. Written from
  `vendor/vsrg-format-docs/vsrg/clone-hero/{chart,ini,sng}` and checked against TheNathannator/GuitarGame_ChartFormats (CC0), the `.chart` reader of Moonscraper kept in
  YARC-Official/YARG.Core (`MoonscraperChartParser/IO/Chart/ChartReader.cs`, `ParseSettings`, `SongEntry.IniBase.cs`, `SngFile.cs`; MIT) and mdsitton/SngFileFormat (MIT), read not
  copied. No real file is in the repository (community charts have no licence): every test file is built in memory, and no real chart was decoded.
- **`.ssq` and `.edit` decoders** (2026-10-28): `src/formats/ssq/` (`mod.rs` decisions, `chunks.rs` chunk walk, `tempo.rs` tick-to-time map, `decoder.rs` variants, report and notes,
  `tests.rs` with every file built in memory), `src/formats/sm_edit/` (`mod.rs` decisions, `decoder.rs`, `tests.rs`), `src/formats/sm/song_header.rs` (a song's header apart from its charts) and
  `parser::chart_tags` made `pub(crate)` in `src/formats/sm/`, `DetectedFormat::Edit`/`Ssq` and their arms in `src/formats/auto/` (`auto_decode*` of an `.edit` is `InvalidFormat("edit needs its song")`,
  `auto_decode_all` lists the charts of an `.ssq`), the exports in `src/formats/mod.rs`. Decode only. Written from `vendor/vsrg-format-docs/vsrg/{ddr/ssq,stepmania/edit}`; the `.ssq` reader is checked against
  SaxxonPike/rhythm-game-formats (`ddr/ssq.md`, MIT) and skogaby/ddr-chart-tools (`docs/ssq_format.md`, `ssq_mine_chunk_format.md`, `src/ssq/{chunk,tempo,steps}.rs`; Apache-2.0), read not copied; the `.edit` reader
  against StepMania (`NotesLoaderSM.cpp` and `NotesLoaderSSC.cpp` `LoadEditFromMsd`, `SongManager::FindSong`, `Song::Matches`, `NotesLoaderSM.h` `MAX_EDIT_STEPS_SIZE_BYTES`; MIT, commit
  825467bcd81c812b33ad684dc04dd151b2d5dec3, whose `Docs/Licenses.txt` was read), read not copied: the only value taken from it is the 60 KiB size limit of an edit. No code was ported, so there is no
  "Adapted from StepMania" comment. No real `.ssq` or `.edit` is in the repository (Konami's data and community edits have no licence): every test file is built in memory, and the tempo chunk of the
  format notes' example (skogaby's `ssq_format.md` section 3.5) is a byte-for-byte test vector.
- **`sm::first_audio_file` and `sm::audio_files` made `pub`** (2026-10-28): the `.sm` rule "a song that names no audio file, or one that is missing, plays the first audio file of its folder" is
  what Prism applies to `.dwi`, `.ksf` and `.ssq` (`game_core::dance_audio`), and `audio_files` lists the folder's audio files sorted by lower-cased name so a `.ssq` can prefer the file of its own stem.
  No behaviour changed (`first_audio_file` is `audio_files(..).first()`).
- **`.dwi` and `.ksf` decoders** (2026-10-28): `src/formats/dwi/` (`mod.rs` decisions, `parser.rs`, `decoder.rs`, `tests.rs`), `src/formats/ksf/` (same files), `src/formats/sm/legacy.rs` (number parsing, row rounding and text decoding of the StepMania loaders),
  `tokenize_raw` in `src/formats/sm/msd.rs`, `SongTiming::new` and the `pub(crate)` visibility of `SongTiming`, `timing_for` and `bpm_points` in `src/formats/sm/ssc_parser.rs`, `DetectedFormat::Dwi`/`Ksf` and their arms in `src/formats/auto/`, the exports in `src/formats/mod.rs`.
  Decode only. Written from `vendor/vsrg-format-docs/vsrg/{dwi/dwi,kick-it-up/ksf}` and PORTED from StepMania (MIT; commit 825467bcd81c812b33ad684dc04dd151b2d5dec3, whose `Docs/Licenses.txt` was read):
  `src/NotesLoaderDWI.cpp` (`DWIcharToNote` table, the step-string grammar, `Is192`, the hold pairing, the tags and their units, `ParseBrokenDWITimestamp`, `DwiCompatibleStringToDifficulty`) and
  `src/NotesLoaderKSF.cpp` (the file-name classification, `HandleBunki`, the row and hold rules, the Direct Move `|` rows, `LoadTags`); each parser carries an `Adapted from StepMania (MIT)` comment. Two deliberate differences:
  a DWI `<..>` jump places every character on one slot (the documented grammar; the loader's loop, as read, skips every other character), and a KSF "KIU compliant" flag is judged per file instead of by the last file of the folder.
  The permission notice of those two files, to be kept with copies:
  `(c) 2001-2004 Chris Danford, Glenn Maynard` (`NotesLoaderDWI.cpp`), `(c) 2001-2006 Chris Danford, Glenn Maynard, Jason Felds` (`NotesLoaderKSF.cpp`): "Permission is hereby granted, free of charge, to any person obtaining a
  copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, and/or
  sell copies of the Software, and to permit persons to whom the Software is furnished to do so, provided that the above copyright notice(s) and this permission notice appear in all copies of the Software and that both the
  above copyright notice(s) and this permission notice appear in supporting documentation. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
  MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF THIRD PARTY RIGHTS. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS NOTICE BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR
  CONSEQUENTIAL DAMAGES, OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE
  OF THIS SOFTWARE." No real `.dwi` or `.ksf` is in the repository (community files have no licence; StepMania's `Docs/SimfileFormats/KSF/sample.ksf` is reproduced as a synthetic test): every test file is built in memory.
