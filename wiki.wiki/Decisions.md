# Decisions Log

## 2026-02-02: Strict Coding Standards Pilot (ROX Codec)

**Decision**: Adopt strict coding standards for the `rox` format codec as a pilot for the entire codebase.
**Context**: Codebase audit revealed violations of best practices (files > 150 lines, `unwrap()` usage).
**Rules Adopted**:
- **Max File Size**: 150 lines (Production code).
- **Max Function Size**: 40 lines (Production code).
- **Safety**: Zero `unwrap()` or `expect()` in production code. `Result` propagation mandatory.
- **Tests**: Structural split (files/functions) required for maintainability, but strict line limits/safety rules are relaxed for test assertions.
**Impact**:
- `src/codec/formats/rox/mod.rs` will be split into `encoder.rs`, `decoder.rs`, and `tests.rs`.
- Future refactors will follow this pattern.
