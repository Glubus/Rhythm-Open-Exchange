# Decisions Log

## 2026-03-28: v0.7.0 Rewrite

**Decision**: Full rewrite of the codec, model, and format crates.

**Changes**:
- Replaced `bincode` with `rkyv` + zstd for the native format (`RoxNativeCodec`)
- Delta timestamp encoding before compression for better ratio
- `CompactString` for all metadata string fields
- `no_std` + `alloc` compatible `rox` core crate
- Template Method pattern for `Encoder`/`Decoder` — `encode_inner`/`decode_inner` are the extension points; `encode`/`decode` call validate automatically
- Validation split into dedicated `codec::validate` module, called by both traits
- `#[derive(Format)]` macro replaces manual extension registration
- FNF, osu!taiko, JROX, YROX added
- All formats have both unit tests and integration tests (`tests/integration_test.rs`)

## 2026-03-28: OsuDecodeOptions

**Decision**: Add `OsuDecodeOptions { re_arrange_bpm: bool }` rather than silently fixing bad beatmaps in the default decoder.

**Context**: Some osu!mania beatmaps (notably those with 50K+ notes from stress-test tooling) have the first BPM timing point placed after the first note, which fails `BpmAfterFirstNote` validation.

**Rules**:
- Default `OsuDecoder::decode` is strict — broken beatmaps return an error
- `OsuDecoder::decode_with_options` with `re_arrange_bpm: true` repairs the timing point and emits `tracing::warn!`
- The repair shifts the first BPM to `first_note_time - 1µs`

## 2026-02-02: Strict Coding Standards

**Decision**: Strict coding standards adopted across the entire codebase.

**Rules**:
- Max function size: 30 lines (production code)
- Zero `unwrap()` / `expect()` in production — use `?` and `RoxError`
- Tests: use `rstest` with `#[case]` parameterized tables
- `#[allow(...)]` suppressions require a justification comment on the same line
- `#![warn(clippy::pedantic)]` in all crates

## 2026-02-02: Template Method for Codec Traits

**Decision**: `Encoder::encode` and `Decoder::decode` are sealed entry points. Implementors override only `encode_inner` / `decode_inner`.

**Rationale**: Validation must always run — making it opt-in per implementor would inevitably be forgotten. The template method enforces the contract at the trait level.
