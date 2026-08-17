# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
