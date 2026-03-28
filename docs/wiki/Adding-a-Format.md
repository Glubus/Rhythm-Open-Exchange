# Adding a Format

This guide walks through adding a new format codec to `rox-formats`. Follow every step — skipping any of them will cause the PR to be rejected.

---

## 1. Create the module directory

```
crates/formats/src/<name>/
  mod.rs       — public re-exports
  types.rs     — format-native types (intermediate structs, enums)
  parser.rs    — raw byte/text → format-native types
  decoder.rs   — format-native types → RoxChart (implements Decoder)
  encoder.rs   — RoxChart → format-native types → bytes (implements Encoder)
```

For decode-only formats, omit `encoder.rs`. If the format has no intermediate representation, `types.rs` and `parser.rs` can be merged into one file — justify this in the PR.

---

## 2. Implement `decode_inner` and `encode_inner`

In `decoder.rs`:

```rust
use rox::{traits::Decoder, RoxChart, RoxResult};

pub struct MyFormatDecoder;

impl Decoder for MyFormatDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        // 1. Parse raw bytes into format-native types (call parser.rs)
        // 2. Map format-native types to RoxChart fields
        // 3. Return Ok(chart) — do NOT call validate here
    }
}
```

In `encoder.rs`:

```rust
use rox::{traits::Encoder, RoxChart, RoxResult};

pub struct MyFormatEncoder;

impl Encoder for MyFormatEncoder {
    fn encode_inner(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        // 1. Map RoxChart fields to format-native types
        // 2. Serialise to bytes
        // 3. Return Ok(bytes) — do NOT call validate here
    }
}
```

Validation is called automatically by the `decode` and `encode` blanket implementations. Never call `validate` inside `decode_inner` or `encode_inner`.

---

## 3. Annotate with `#[derive(Format)]`

Add the proc-macro annotation to your decoder and encoder structs. The `extensions` attribute registers the file extensions for auto dispatch.

```rust
use rox_macros::Format;

#[derive(Format)]
#[format(extensions = ["myfmt", "mf"])]
pub struct MyFormatDecoder;

#[derive(Format)]
#[format(extensions = ["myfmt", "mf"])]
pub struct MyFormatEncoder;
```

If a single type implements both `Decoder` and `Encoder` (like `RoxNativeCodec`), apply the macro once.

---

## 4. Write a failing test first

Create a real asset file and write a test that fails before you implement anything. Place assets in `crates/test-utils/assets/<name>/`.

```rust
// crates/formats/tests/my_format_test.rs
use rox_formats::my_format::MyFormatDecoder;
use rox::traits::Decoder;
use rstest::rstest;

#[test]
fn decode_basic_chart_fails_before_impl() {
    let data = include_bytes!("../../test-utils/assets/my_format/basic.myfmt");
    let result = MyFormatDecoder::decode(data);
    assert!(result.is_ok());  // this will fail until decode_inner is implemented
}
```

Commit the failing test before writing the implementation.

---

## 5. Implement

Fill in `parser.rs`, `decoder.rs`, and `encoder.rs`. Confirm the test from step 4 now passes.

Tips:
- Keep `parse` and `decode` logic separate. Parsing converts bytes to structured intermediate types; decoding maps those to `RoxChart`.
- Return `RoxError::InvalidFormat(description)` for any format-level violation discovered during parsing.
- Return `RoxError::Deserialize(description)` for unexpected structural failures (e.g. missing required field).
- Avoid `.unwrap()` and `.expect()` in library code — propagate errors with `?`.

---

## 6. Write table tests for multiple scenarios

Use `rstest` `#[case]` parameterisation to cover multiple assets and expected outcomes:

```rust
#[rstest]
#[case("basic.myfmt", 4, 100)]
#[case("coop.myfmt", 8, 200)]
#[case("empty.myfmt", 4, 0)]
fn decode_key_count_and_note_count(
    #[case] asset: &str,
    #[case] expected_keys: u8,
    #[case] expected_notes: usize,
) {
    let path = format!("../../test-utils/assets/my_format/{asset}");
    let data = std::fs::read(&path).unwrap();
    let chart = MyFormatDecoder::decode(&data).unwrap();
    assert_eq!(chart.key_count, expected_keys);
    assert_eq!(chart.notes.len(), expected_notes);
}
```

Cover at minimum: a standard chart, a chart with SVs, an invalid file, and (if applicable) each decode option variant.

---

## 7. Add an integration test

Add a round-trip test to `crates/formats/tests/integration_test.rs`:

```rust
#[test]
fn my_format_round_trip() {
    let data = include_bytes!("../../test-utils/assets/my_format/basic.myfmt");
    let chart = MyFormatDecoder::decode(data).unwrap();
    let encoded = MyFormatEncoder::encode(&chart).unwrap();
    let chart2 = MyFormatDecoder::decode(&encoded).unwrap();

    assert_eq!(chart.key_count, chart2.key_count);
    assert_eq!(chart.notes.len(), chart2.notes.len());
    assert_eq!(chart.metadata.title, chart2.metadata.title);
}
```

---

## 8. Register in `lib.rs`

In `crates/formats/src/lib.rs`, add the module declaration and re-export your public types:

```rust
pub mod my_format;
pub use my_format::{MyFormatDecoder, MyFormatEncoder};
```

Verify auto dispatch picks up the new extensions by running:

```sh
cargo test -p rox-formats auto_dispatch
```

---

## 9. Run clippy pedantic and fix all warnings

```sh
cargo clippy -p rox-formats -- -W clippy::pedantic -D warnings
```

Every suppressed lint must have a `#[allow(...)]` with a comment explaining why. Do not suppress `clippy::pedantic` wholesale.

---

## 10. Write a wiki page for the format

Add a section to [rox-formats.md](rox-formats.md) under "Format details" covering:

- What the format is and where it comes from
- The public API (decoder, encoder, any options structs)
- Known quirks or lossy mappings
- Any std-gating requirements

If the format has unique options (like `OsuDecodeOptions` or `FnfSide`), document each field.
