use rox::codec::{Decoder, Encoder, Format};
use rox::model::{Note, RoxChart};
use rox::RoxResult;

// Minimal codec for testing
struct CountCodec;

impl Encoder for CountCodec {
    fn encode_inner(chart: &RoxChart) -> RoxResult<Vec<u8>> {
        Ok((chart.note_count() as u64).to_le_bytes().to_vec())
    }
}

impl Decoder for CountCodec {
    fn decode_inner(_data: &[u8]) -> RoxResult<RoxChart> {
        Ok(RoxChart::new(4))
    }
}

#[derive(rox_macros::Format)]
#[format(extensions = ["test", "tst"])]
struct TestFormat;

#[test]
fn test_encode_rejects_invalid_chart() {
    let mut chart = RoxChart::new(4);
    chart.notes.push(Note::tap(0, 99)); // column 99 invalid for 4K
    assert!(CountCodec::encode(&chart).is_err());
}

#[test]
fn test_encode_valid_chart_succeeds() {
    let chart = RoxChart::new(4);
    assert!(CountCodec::encode(&chart).is_ok());
}

#[test]
fn test_decode_validates_output() {
    assert!(CountCodec::decode(&[]).is_ok());
}

#[test]
fn test_format_derive_extensions() {
    assert_eq!(TestFormat::EXTENSIONS, &["test", "tst"]);
    assert!(TestFormat::supports_extension("test"));
    assert!(TestFormat::supports_extension("TEST")); // case-insensitive
    assert!(TestFormat::supports_extension("TST"));
    assert!(!TestFormat::supports_extension("osu"));
}
