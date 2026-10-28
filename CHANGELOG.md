# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] (Prism's vendored copy)

### Added

- BMS family decoder, `rox::formats::bms`: `BmsDecoder` (`.bms`, `.bme`, `.bml`) and `PmsDecoder` (`.pms`), `DetectedFormat::Bms`/`Pms`, `auto_decode*` support (`auto_encode` refuses them: decode only), and `decode_with_report` returning a `BmsReport` of what was chosen, dropped or repaired. Measure lengths (channel 02), BPM (channels 03 and 08 with `#BPMxx`/`#EXBPMxx`), STOP (channel 09 with `#STOPxx`), the three long-note notations (`#LNTYPE 1` channels 5x/6x, `#LNOBJ`, `#LNTYPE 2`), landmines as mines, `#RANDOM`/`#SETRANDOM`/`#IF`/`#ELSEIF`/`#ELSE`/`#ENDIF`/`#ENDRANDOM` resolved reproducibly (`#RANDOM` always draws 1), Shift-JIS / EUC-KR / UTF-8 / UTF-16 text, note keysounds in the hitsound table. Layout from the lanes used: `[scratch] 5 or 7 keys`, double play `[scratch1] keys | keys [scratch2]`, PMS 9 buttons. When a chart has exactly one defined background sound it becomes `metadata.audio_file` and the time origin. Hand-written charts in `assets/bms/`.
- `.bmson` 1.x decoder, `BmsonDecoder` (`rox::formats::bms::bmson`), `DetectedFormat::Bmson`, `BmsonReport`: pulses (`info.resolution`), BPM and stop events, holds from `l`, layout from `info.mode_hint` (beat-5k/7k/10k/14k, popn-5k/9k, generic-Nkeys, keyboard-24k), same column order and single-background-sound audio rule as BMS, channel files in the hitsound table. Test chart `assets/bms/example.bmson`.
- `rox::AudioPlan` / `SoundEvent` (`src/audio_plan.rs`): the sounds of a chart (time, file, slice, volume), returned BESIDE the chart by `BmsDecoder`/`PmsDecoder`/`BmsonDecoder::decode_with_audio_plan`; never part of `RoxChart`, `.rox` or any hash. BMS: channel-01 sounds and every playable object's keysound; bmson: channels sliced as beatoraja does.
- File names of BMS (`#WAVxx`, `#STAGEFILE`, `#BACKBMP`) that are absolute, hold a drive letter or `..` are read as undefined (`BmsReport::unsafe_names`).
- New optional dependency `encoding_rs` (feature `formats`).
- O2Jam, `rox::formats::o2jam` (decode only): `OjnDecoder` (`.ojn`, `DetectedFormat::Ojn`) gives seven-column charts with taps and long notes, BPM from channel 1 and the header, measure fractions from channel 0, `variants` / `decode_variant` for the playable difficulties of a file (an empty one is not a variant; `auto_decode_variant` takes the n-th playable difficulty of an `.ojn`, `auto_decode` the first), `decode_with_report` (`OjnReport`) and `decode_with_audio_plan`. Broken long notes follow Open2Jam's "O2Jam" policy. `OjmContainer` parses the `.ojm` sample banks (M30 with the plain / `nami` / `0412` flags, OMC with the block rearrangement and the "acc xor", OJM) and returns Ogg streams and complete WAV files (RIFF header built for the headerless PCM); no audio decoding.
- `SoundEvent::index` (`Option<u32>`) and `SoundEvent::in_container`: with `Some`, `file` names a container (an `.ojm`) and `index` the sample inside it. `SoundEvent::whole` and the BMS/bmson decoders set `None`.
- FNF V-Slice, `rox::formats::fnf_vslice` (decode only): `VSliceDecoder` reads `<song>-chart.json` with its `<song>-metadata.json` (`variants`, `decode_variant`, `decode_with_report` (`VSliceReport`), `decode_with_audio_plan`, `decode_with_audio_plan_in`) and a `.fnfc` package (`FnfcPackage`, `fnfc_variants`, `decode_fnfc_variant`, `decode_fnfc_with_report`, `decode_fnfc_with_audio_plan`). One variant per difficulty of the file's `notes` map (easy, normal, hard, erect, nightmare, then by name), player strumline only as a four-column chart, holds from `l`, BPM and signature from `timeChanges`; the scroll speed is reported, not mapped. The audio plan names `Inst[-id].ogg` and `Voices-<character>[-variation].ogg` (optional) and shifts them by the metadata `offsets` (a positive offset delays the stem). `.fnfc` entries are limited (count, declared and real unpacked size, unsafe names). `DetectedFormat::Fnfc`; `auto_decode` / `auto_decode_variant` route `<song>-chart[-variation].json` (when it is not a legacy chart) and `.fnfc`; helpers `is_vslice_chart_name` and `vslice_metadata_name`. See `src/formats/fnf_vslice/mod.rs` for every decision.
- New optional dependency `zip` (feature `formats`, range `>=4.6, <9`, inflate only) for `.fnfc`.

- Clone Hero, `rox::formats::clone_hero` (decode only): `ChartDecoder` reads `.chart` (`DetectedFormat::CloneHero`; `variants` / `decode_variant` / `decode_with_ini` / `decode_with_report` / `decode_with_audio_plan` / `decode_file`): one five-column chart per 5-lane track (`Single`, `DoubleGuitar`, `DoubleBass`, `DoubleRhythm`, `Keyboard` at Expert, Hard, Medium, Easy; drums, GHL and `SingleBass` are reported, not read), tempo map from `B` events (thousandths of a BPM) and `TS`, sustains as holds (no cutoff in `.chart` unless the `song.ini` gives `sustain_cutoff_threshold`), forced/tap flags and open notes counted and dropped, `[Song]` `Offset` and `song.ini` `delay` applied to the times (the `song.ini` wins), metadata from `song.ini` over `[Song]`. `SongIni::parse` reads the `song.ini` tags; `SngPackage::open` reads a `.sng` (header, metadata, file index, masked data; hard length checks), with `variants`, `decode_variant` and `decode_with_audio_plan`. The audio plan holds every stem (`song`, `guitar`, `rhythm`, `bass`, `keys`, `drums`, `drums_1..4`, `vocals`, `vocals_1/2`, `crowd` and the `[Song]` stream tags) as a whole file at time 0: optional events without a file listing, exact with one (`ChartContext::files`, or the entries of a `.sng`, as `index` events on the `.sng`). `auto_decode` / `auto_decode_variant` read the `song.ini` of the folder. The `.mid` is read by `MidDecoder` (next entry).
- `.sng` in the automatic dispatch: `DetectedFormat::Sng`, `auto_decode` / `auto_decode_variant` / `auto_decode_bytes` read a Clone Hero package (its own metadata, `notes.chart`). New `auto_decode_all(path)` (`FileVariant`): every playable difficulty of an `.ojn`, a V-Slice `<song>-chart[-<variation>].json`, a `.fnfc` (default variation), a Clone Hero `.chart` (with the `song.ini` beside it) or `.sng`, each decoded on its own (one that fails is its own `Err`), from one read of the file; `Ok(None)` for every other file.
- Clone Hero `.mid`, `rox::formats::clone_hero::MidDecoder` (decode only; `variants` / `decode_variant` / `decode_with_ini` / `decode_with_report` / `decode_with_audio_plan` / `decode_file`, `is_mid_chart_name`): a Rock Band / Clone Hero MIDI chart is read ONLY when the file is NAMED `notes.mid` (`DetectedFormat::CloneHeroMid`; any other `.mid` stays unsupported, `.mid` is the extension of every MIDI file). Own Standard MIDI File reader (`clone_hero::smf`: format 0 and 1, ticks per quarter note, running status not reset by SysEx or meta events, hard limits on size, chunks, events and notes). One five-column chart per 5-lane track (`PART GUITAR`, `PART GUITAR COOP`, `PART BASS`, `PART RHYTHM`, `PART KEYS` at Expert 96..=100, Hard 84..=88, Medium 72..=76, Easy 60..=64; drums, vocals, harmonies, GHL, `PART REAL_*`, `PART DANCE` and `T1 GEMS` are reported, not read), tempo map and time signatures from the first track in exact µs per quarter note, sustain cutoff of a third of the resolution (the `song.ini` `sustain_cutoff_threshold` replaces it), forced/tap markers counted, open notes (`[ENHANCED_OPENS]` keys and the Phase Shift open SysEx) dropped and counted, `song.ini` `delay` applied, metadata from the `song.ini`, the audio plan of the `.chart`. `auto_decode`, `auto_decode_variant`, `auto_decode_all` and `auto_decode_bytes` route `notes.mid` (`FileVariant::name` is `PART GUITAR Expert`...); `auto_encode` refuses it. `SngPackage` reads a `notes.mid` entry, preferred to `notes.chart` (the order of `YARG.Core`). `ChartReport` has three new counters (`unpaired_midi_events`, `sync_events_off_first_track`, `duplicate_tracks_ignored`).
- DDR `.ssq`, `rox::formats::ssq` (decode only): `SsqDecoder` (`DetectedFormat::Ssq`; `variants` / `decode_variant` / `decode_with_report` (`SsqReport`) / `decode_with_title` / `decode_file`): one 4-column (Single) or 8-column (Double) chart per playable step chunk (the ten difficulty codes of the game: Beginner, Basic, Difficult, Expert, Challenge), tempo chunk to time by the game's linear interpolation (BPM points per segment, stops as equal offsets, TPS 1000/150/75 or any), freeze block to holds, shock arrows (`0x0F`, `0xF0`, `0xFF`) to mines on the panels of the side, the DDR World mine chunk (type 20). No title, artist or audio in the file: `audio_file` empty, title = file stem through `auto_decode*` / `decode_file`. `auto_decode` reads the first chart, `auto_decode_variant` the n-th, `auto_decode_all` lists every one. Truncated, oversized or inconsistent chunks end the walk or skip the chart, never panic.
- StepMania `.edit`, `rox::formats::sm_edit` (decode only): `EditDecoder::song_path` (the normalised, safe `#SONG`), `decode_for_song` / `decode_for_header` / `decode_for_song_file` / `parse_for_song` (the edit's one chart laid over a song's `.sm`/`.ssc` header: title, artist, offset, BPMs, stops, delays, warps, fakes, `#MUSIC` relative to the song's folder; an `.ssc`-style edit's own timing tags win), new `sm::song_header::SongHeader` / `parse_header_file`, `DetectedFormat::Edit`. `auto_decode*` of a lone `.edit` fails with `InvalidFormat("edit needs its song")`; `auto_decode_all` gives `None`. Both the `.ssc` and the `.sm` layout are read, the difficulty is forced to `Edit` (named by the description), files over 60 KiB are refused like the game does.
- Dance With Intensity `.dwi`, `rox::formats::dwi` (decode only): `DwiDecoder` (`DetectedFormat::Dwi`; `parse` into the `.sm` types, `variants` / `decode_variant` / `decode_with_report` (`DwiReport`)): every `#SINGLE` (4 columns), `#SOLO` (6) and `#DOUBLE` (8) chart of the file is a variant, parsed into `sm::types::SmFile` so timing goes through `sm::decoder::from_chart`; `#COUPLE` is skipped with a message in `SmFile::skipped`. Note grammar as StepMania's `NotesLoaderDWI.cpp` (hex pad codes `0-9 A-M`, `(..)` 1/16, `[..]` 1/24, `{..}` 1/64, `` `..' `` 1/192, `<..>` jumps and the old 1/192 marker, `show!hold` holds released by the next arrow of the column); `#GAP` (integer ms, later when positive), `#BPM`, `#CHANGEBPM` and `#FREEZE` (positions divided by 4, freezes in ms) as the loader reads them; `#FILE` is `audio_file` as written; text is UTF-8 else Latin-1. `auto_decode`, `auto_decode_variant`, `auto_decode_all` and `auto_decode_bytes` route `.dwi`.
- Kick It Up / Direct Move `.ksf`, `rox::formats::ksf` (decode only): `KsfDecoder` (`DetectedFormat::Ksf`; `decode_named` / `decode_with_report` with a `KsfContext` (file name, folder name) / `parse`, `KsfReport`): one chart per file, step type and difficulty from the FILE NAME by the loader's rules (`pump-single` 5 columns and `pump-halfdouble` 6 are played; `pump-double` and `pump-couple` are refused with a message), `#TICKCOUNT` rows, taps, holds (first to last `4`), mines, lifts (fakes not played), `#STARTTIME` (centiseconds), `#BPM2`/`#BPM3` at `#BUNKI`/`#BUNKI2` times as StepMania's `HandleBunki`, Direct Move `|T` `|B` `|E` `|D` rows; titles in EUC-KR/CP949 or UTF-8, `#SONGFILE` as `audio_file` (empty when absent). `auto_decode*` route `.ksf` with the file and folder names; `auto_decode_all` gives `None` (one chart per file).
- `sm::msd::tokenize_raw` (no backslash unescaping, for `.dwi` and `.ksf`), `sm::legacy` (the loaders' lenient number parsing and text decoding) and `SongTiming`/`timing_for`/`bpm_points` of `sm::ssc_parser` made `pub(crate)` for the new readers.

### Fixed

- FNF (legacy `fnf` decoder): a Psych `psych_v1` file (`format` field) gives the lanes 0-3 to the player and 4-7 to the opponent whatever `mustHitSection` says (Psych Engine `PlayState.generateSong`); the decoder applied the pre-1.0 `mustHitSection` flip and read the opponent's notes as the player's in every section with `mustHitSection: false`. Files without `format` are read as before.

## [0.8.1] - 2026-10-25

### Security

- Native `.rox` decoding is now checked. It used `rkyv::from_bytes_unchecked` on an unbounded zstd inflation, so a malformed or hostile file (a download, for instance) was undefined behaviour and a zstd bomb could exhaust memory. Now the zstd frame is inflated into a bounded, aligned buffer (`MAX_DECOMPRESSED`, 64 MiB; past it `RoxError::InvalidFormat`), the rkyv archive is validated with `bytecheck` (`rkyv` feature `bytecheck`) before anything is read (damaged files give `RoxError::Deserialize`), and the declared counts are capped (`MAX_NOTES` 4 000 000, `MAX_TIMING_POINTS` and `MAX_HITSOUNDS` 1 000 000). There is no unchecked path; measured on a 200 000-note chart the validation costs about 35 µs of a 4.4 ms decode (zstd is 2.4 ms). Delta-decoded note times wrap instead of overflowing. No API change.

## [0.8.0] - 2026-10-23

### Added

- `.ssc` (StepMania 5 / Etterna): `SscDecoder`, `SscEncoder`, `sm::ssc_parser` and `DetectedFormat::Ssc`. Each `#NOTEDATA` chart keeps its own step type (`dance-single/solo/double`, `pump-single/halfdouble`, `kb7-single`), meta, BPMs, stops, delays, warps, fakes and offset; unsupported step types and malformed charts are skipped with a message in `SmFile::skipped`.
- `sm::parse_file`, `sm::is_stepmania` and `auto_decode_variant` (chart `n` of a `.sm`/`.ssc` file, the only chart of any other format).
- `sm::msd` (the `#TAG:value;` tokenizer) and `sm::timing_data` (beat to time with stops, delays and warps).
- `formats::names`: `romanised_names` / `romanised_names_from_bytes` (`RomanisedNames`) read a chart's original and romanised title and artist from its header only (osu! `TitleUnicode`/`Title`, StepMania `#TITLE`/`#TITLETRANSLIT`).
- `OsuDecoder::peek_mode`: the game mode of a `.osu` from its first lines, so a file of another mode is refused before its hit objects are parsed (`decode_with_options` does it first).
- `RoxChart::remove_unsupported_notes` (mines and rolls, head and tail together) returning `RemovedNotes { mines, rolls }`.

### Changed

- `.sm`/`.ssc` `#OFFSET` is baked into the decoded times (beat 0 is at `-offset` s of the audio, as in StepMania and Etterna); `metadata.audio_offset_us` stays 0. Before, the offset was ignored and every chart with a non-zero offset was out of sync by that amount. `.sm` now shares the `.ssc` timing pipeline (one `TimingData`: BPMs, stops, delays, warps, fakes); rows are rounded to 1/48 beat like the games.
- `.sm`/`.ssc` refuse a chart they cannot time faithfully: a zero or negative BPM, a negative stop, delay or warp (the old `.sm` warp encoding).
- `.sm`/`.ssc` without a usable `#MUSIC` play the folder's first audio file.
- `.qua`: `InitialScrollVelocity` becomes a scroll velocity point at the start of the map.

## [0.7.0] - 2026-03-27

### Breaking Changes

- **Single-crate layout**: The public library, codecs, and analysis modules are shipped as one `rox` crate with feature flags.
- **`TimingPoint` is now an enum**: Use `TimingPoint::Bpm { time_us, bpm, signature }` and `TimingPoint::Sv { time_us, scroll_speed }`. Constructors: `TimingPoint::bpm(time_us, bpm)` and `TimingPoint::sv(time_us, scroll_speed)`
- **`key_count` moved**: From `Metadata` to `RoxChart` directly
- **`.rox` binary format v3**: Files encoded with version 2 are rejected with `RoxError::UnsupportedVersion(2)`. The `TimingPoint` enum has a different rkyv binary layout than the old struct

### Added

- `rox-macros`: `#[derive(Format)]` proc-macro — generates `impl Format for T` from `#[format(extensions = [...])]`
- `validate()` enforced at trait level via Template Method — `Encoder::encode` and `Decoder::decode` call `validate()` automatically
- Six focused validators: `validate_metadata`, `validate_timing_points`, `validate_notes_sorted`, `validate_note_columns`, `validate_note_overlaps`, `validate_note_durations`
- `BpmAfterFirstNote` error variant now actually checked in `validate_timing_points` (was defined but never called in v0.6.x)
- `no_std + alloc` support in `rox` crate
- `rox-test-utils`: internal dev-only crate for test asset loading
- `convert<D, E>` and `convert_file<D, E>` generic utilities in `rox`
- `rox::prelude` module for convenient wildcard imports

## [0.6.2] - 2026-02-02

### Changed

- **Strict Standards Pilot (ROX Codec)**:
  - Refactored `rox` codec into modular structure (`encoder`, `decoder`, `tests`).
  - Enforced strict safety: Removed all `unwrap()`/`expect()` calls in production code.
  - Enforced strict file limits: Split `mod.rs` (553 lines) into smaller files (< 150 lines).
  - Improved test organization by splitting monolithic tests.

## [0.6.1] - 2026-02-01

### Performance

- **Aggressive Optimization of `.osu` Parser**:
  - Implemented **SIMD** parsing using `memchr` and `atoi` for the `HitObjects` section.
  - Switched to **Zero-Copy I/O** using `memmap2`.
  - Optimized memory usage with `compact_str` for metadata fields and `Vec` pre-allocation.
  - **Result**: ~58% reduction in load time (14.6ms -> 6.2ms for 50k objects).
- **Data Layout Optimization**:
  - Reordered `Note` struct fields to minimize padding (size reduced from 40 to 32 bytes).
  - significantly improved `RoxChart` binary encoding/decoding speed:
    - **Decode**: ~0.82ms (from ~2.7ms)
    - **Encode**: ~1.69ms
    - **Throughput**: ~61 Million Notes/sec

### Documentation

- Added `wiki.wiki/Performance-Optimizations.md` detailing the optimization techniques.
- Updated `wiki.wiki/Format-Converters.md` with new benchmark results.

## [0.6.0] - 2026-01-31

### Added

- **Pattern Recognition**: Full rewrite of the module to match Quattern C# implementation (1:1 port).
  - Implemented `CrossSegmentAnalyzer` (sliding window) for timeline generation.
  - Implemented `TimingAnalyzer` for density-based BPM calculation.
  - Implemented pattern merging logic for compatible pattern types.
  - Supports detection of Streams, Jacks, Jumps, Jumpstreams, Chordjacks, etc.
  - Available in CLI via `-aa` / `--advanced-analysis` flag.
  - Exposed via `RoxAnalysis` trait and FFI bindings.
- **CLI**: Added `-aa` flag to `rox info` command.

### Documentation

- Added `wiki.wiki/Pattern-Recognition.md` detailing the algorithms and decisions.
- Updated all README links and status tables.

### Fixed

- Fixed documentation discrepancies regarding `osu!taiko` (now marked as read-only).
- Fixed broken links in `README.md` files.

## [0.5.6] - 2026-01-31

### Changed

- Standardized strict `MAX_FILE_SIZE` (100MB) checks across all format parsers, including `jrox`, `yrox`, and `rox`, to prevent memory exhaustion.

## [0.5.5] - 2026-01-31

### Changed
- Refactored `src/codec/auto.rs` into a modular `auto` directory to improve maintainability and strictly adhere to file size limits.
- Refactored `src/codec/formats/osu/parser.rs` into a modular `osu/parser` directory to strictly adhere to file size limits.

## [0.5.4] - 2026-01-30

### Changed
- Refactored `taiko`, `fnf`, and `qua` parsers to include architectural documentation and strict safety limits (MAX_FILE_SIZE).
- Added `tracing` warnings to `taiko` parser for robustness.

## [0.5.3] - 2026-01-30

### Changed

- Refactored `taiko`, `fnf`, and `qua` parsers to include architectural documentation and strict safety limits (MAX_FILE_SIZE).
- Added `tracing` warnings to `taiko` parser for robustness.

## [0.5.2] - 2026-01-30

### Changed

- Refactored `sm` parser to use `tracing` for observability and replaced silent failures with warnings.
- Added strict `MAX_FILE_SIZE` (100MB) checks to `osu` and `sm` parsers to prevent resource exhaustion.
- Enhanced architectural documentation in parsers.

## [0.5.1] - 2026-01-30

### Added

- Added `tracing` for observability in parsers.
- Added benchmarks to GitHub Actions CI.

### Changed

- Refactored `osu` parser to log warnings instead of silently ignoring malformed fields.
- Removed `.drone.yml` in favor of GitHub Actions.
