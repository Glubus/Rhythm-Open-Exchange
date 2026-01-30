# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
