use rox::codec::{Decoder, Encoder};
use rox_formats::{
    FnfDecoder, FnfEncoder, JroxDecoder, JroxEncoder, McDecoder, McEncoder, OsuDecoder,
    OsuEncoder, SmDecoder, SmEncoder, TaikoDecoder,
};
use rox_test_utils::get_test_asset;

#[cfg(feature = "std")]
use rox_formats::{QuaDecoder, QuaEncoder};

use rstest::rstest;

// ---------------------------------------------------------------------------
// osu! mania — decode-only (basic properties)
// ---------------------------------------------------------------------------

#[rstest]
#[case("osu/mania_4k.osu", 4)]
#[case("osu/mania_7k.osu", 7)]
fn osu_decode_basic(#[case] asset: &str, #[case] expected_keys: u8) {
    let data = get_test_asset(asset);
    let chart = OsuDecoder::decode(&data).expect("OsuDecoder::decode failed");
    assert_eq!(chart.key_count, expected_keys, "key_count mismatch for {asset}");
    assert!(!chart.notes.is_empty(), "notes should not be empty for {asset}");
    assert!(
        !chart.timing_points.is_empty(),
        "timing_points should not be empty for {asset}"
    );
}

// ---------------------------------------------------------------------------
// osu! mania — roundtrip
// ---------------------------------------------------------------------------

#[rstest]
#[case("osu/mania_4k.osu")]
#[case("osu/mania_7k.osu")]
fn osu_roundtrip(#[case] asset: &str) {
    let data = get_test_asset(asset);
    let chart = OsuDecoder::decode(&data).expect("OsuDecoder::decode failed");
    let encoded = OsuEncoder::encode(&chart).expect("OsuEncoder::encode failed");
    let chart2 = OsuDecoder::decode(&encoded).expect("OsuDecoder::decode (roundtrip) failed");
    assert_eq!(
        chart2.key_count, chart.key_count,
        "roundtrip key_count mismatch for {asset}"
    );
    assert_eq!(
        chart2.notes.len(),
        chart.notes.len(),
        "roundtrip notes.len mismatch for {asset}"
    );
}

// ---------------------------------------------------------------------------
// osu! taiko — decode-only
// ---------------------------------------------------------------------------

#[test]
fn taiko_decode_basic() {
    let data = get_test_asset("osu/taiko.osu");
    let chart = TaikoDecoder::decode(&data).expect("TaikoDecoder::decode failed");
    assert!(chart.key_count > 0, "key_count should be > 0");
    assert!(!chart.notes.is_empty(), "taiko notes should not be empty");
    assert!(
        !chart.timing_points.is_empty(),
        "taiko timing_points should not be empty"
    );
}

// ---------------------------------------------------------------------------
// StepMania — decode-only
// ---------------------------------------------------------------------------

#[test]
fn sm_decode_basic() {
    let data = get_test_asset("stepmania/4k.sm");
    let chart = SmDecoder::decode(&data).expect("SmDecoder::decode failed");
    assert_eq!(chart.key_count, 4, "expected 4K for stepmania/4k.sm");
    assert!(!chart.notes.is_empty(), "sm notes should not be empty");
    assert!(
        !chart.timing_points.is_empty(),
        "sm timing_points should not be empty"
    );
}

// ---------------------------------------------------------------------------
// StepMania — roundtrip
// ---------------------------------------------------------------------------

#[test]
fn sm_roundtrip() {
    let data = get_test_asset("stepmania/4k.sm");
    let chart = SmDecoder::decode(&data).expect("SmDecoder::decode failed");
    let encoded = SmEncoder::encode(&chart).expect("SmEncoder::encode failed");
    let chart2 = SmDecoder::decode(&encoded).expect("SmDecoder::decode (roundtrip) failed");
    assert_eq!(chart2.key_count, chart.key_count, "roundtrip key_count mismatch");
    assert_eq!(
        chart2.notes.len(),
        chart.notes.len(),
        "roundtrip notes.len mismatch"
    );
}

// ---------------------------------------------------------------------------
// Quaver — decode-only (std-only feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "std")]
#[rstest]
#[case("quaver/4K.qua", 4)]
#[case("quaver/7K.qua", 7)]
fn qua_decode_basic(#[case] asset: &str, #[case] expected_keys: u8) {
    let data = get_test_asset(asset);
    let chart = QuaDecoder::decode(&data).expect("QuaDecoder::decode failed");
    assert_eq!(chart.key_count, expected_keys, "key_count mismatch for {asset}");
    assert!(!chart.notes.is_empty(), "qua notes should not be empty for {asset}");
    assert!(
        !chart.timing_points.is_empty(),
        "qua timing_points should not be empty for {asset}"
    );
}

// ---------------------------------------------------------------------------
// Quaver — roundtrip (std-only feature)
// Note: only 4K roundtrip is tested here because the QuaEncoder does not yet
// persist the GameMode field, so a 7K encode decodes back as 4K (InvalidColumn).
// ---------------------------------------------------------------------------

#[cfg(feature = "std")]
#[test]
fn qua_roundtrip_4k() {
    let asset = "quaver/4K.qua";
    let data = get_test_asset(asset);
    let chart = QuaDecoder::decode(&data).expect("QuaDecoder::decode failed");
    let encoded = QuaEncoder::encode(&chart).expect("QuaEncoder::encode failed");
    let chart2 = QuaDecoder::decode(&encoded).expect("QuaDecoder::decode (roundtrip) failed");
    assert_eq!(chart2.key_count, chart.key_count, "roundtrip key_count mismatch");
    assert_eq!(chart2.notes.len(), chart.notes.len(), "roundtrip notes.len mismatch");
}

// ---------------------------------------------------------------------------
// FNF — decode-only
// ---------------------------------------------------------------------------

#[test]
fn fnf_decode_basic() {
    let data = get_test_asset("fnf/test-song.json");
    let chart = FnfDecoder::decode(&data).expect("FnfDecoder::decode failed");
    assert!(chart.key_count > 0, "fnf key_count should be > 0");
    assert!(!chart.notes.is_empty(), "fnf notes should not be empty");
    assert!(
        !chart.timing_points.is_empty(),
        "fnf timing_points should not be empty"
    );
}

// ---------------------------------------------------------------------------
// FNF — roundtrip
// ---------------------------------------------------------------------------

#[test]
fn fnf_roundtrip() {
    let data = get_test_asset("fnf/test-song.json");
    let chart = FnfDecoder::decode(&data).expect("FnfDecoder::decode failed");
    let encoded = FnfEncoder::encode(&chart).expect("FnfEncoder::encode failed");
    let chart2 = FnfDecoder::decode(&encoded).expect("FnfDecoder::decode (roundtrip) failed");
    assert_eq!(chart2.key_count, chart.key_count, "roundtrip key_count mismatch");
    assert_eq!(
        chart2.notes.len(),
        chart.notes.len(),
        "roundtrip notes.len mismatch"
    );
}

// ---------------------------------------------------------------------------
// JROX — roundtrip (encode from osu chart, decode back)
// ---------------------------------------------------------------------------

#[test]
fn jrox_roundtrip_via_osu() {
    let data = get_test_asset("osu/mania_4k.osu");
    let chart = OsuDecoder::decode(&data).expect("OsuDecoder::decode failed");
    let encoded = JroxEncoder::encode(&chart).expect("JroxEncoder::encode failed");
    let chart2 = JroxDecoder::decode(&encoded).expect("JroxDecoder::decode failed");
    assert_eq!(chart2.key_count, chart.key_count, "jrox roundtrip key_count mismatch");
    assert_eq!(
        chart2.notes.len(),
        chart.notes.len(),
        "jrox roundtrip notes.len mismatch"
    );
}

#[test]
fn jrox_decode_basic_from_sm() {
    let data = get_test_asset("stepmania/4k.sm");
    let chart = SmDecoder::decode(&data).expect("SmDecoder::decode failed");
    let encoded = JroxEncoder::encode(&chart).expect("JroxEncoder::encode failed");
    let chart2 = JroxDecoder::decode(&encoded).expect("JroxDecoder::decode failed");
    assert!(chart2.key_count > 0, "jrox key_count should be > 0");
    assert!(!chart2.notes.is_empty(), "jrox notes should not be empty");
    assert!(
        !chart2.timing_points.is_empty(),
        "jrox timing_points should not be empty"
    );
}

// ---------------------------------------------------------------------------
// Malody .mc — decode + roundtrip
// ---------------------------------------------------------------------------

#[test]
fn malody_decode_basic() {
    let data = get_test_asset("malody/ZUN (Arr.sun3) - STAR OF ANDROMEDA (Seiryuu)[key].mc");
    let chart = McDecoder::decode(&data).expect("McDecoder::decode failed");
    assert_eq!(chart.key_count, 4, "malody chart should be 4K");
    assert!(!chart.notes.is_empty(), "malody notes should not be empty");
    assert!(
        !chart.timing_points.is_empty(),
        "malody timing_points should not be empty"
    );
}

#[test]
fn malody_roundtrip() {
    let data = get_test_asset("malody/ZUN (Arr.sun3) - STAR OF ANDROMEDA (Seiryuu)[key].mc");
    let chart = McDecoder::decode(&data).expect("McDecoder::decode failed");
    let encoded = McEncoder::encode(&chart).expect("McEncoder::encode failed");
    let chart2 = McDecoder::decode(&encoded).expect("McDecoder::decode (roundtrip) failed");
    assert_eq!(chart2.key_count, chart.key_count, "roundtrip key_count mismatch");
    assert_eq!(chart2.notes.len(), chart.notes.len(), "roundtrip notes.len mismatch");
}
