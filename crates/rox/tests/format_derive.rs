use rox::codec::Format;
use rox_macros::Format as DeriveFormat;

#[derive(DeriveFormat)]
#[format(extensions = ["sm", "ssc"])]
struct SmCodec;

#[derive(DeriveFormat)]
#[format(extensions = ["osu"])]
struct OsuCodec;

#[derive(DeriveFormat)]
#[format(extensions = [])]
struct EmptyFormat;

#[test]
fn test_single_extension() {
    assert_eq!(OsuCodec::EXTENSIONS, &["osu"]);
    assert!(OsuCodec::supports_extension("osu"));
    assert!(OsuCodec::supports_extension("OSU"));
    assert!(!OsuCodec::supports_extension("sm"));
}

#[test]
fn test_multiple_extensions() {
    assert_eq!(SmCodec::EXTENSIONS, &["sm", "ssc"]);
    assert!(SmCodec::supports_extension("sm"));
    assert!(SmCodec::supports_extension("SSC"));
    assert!(!SmCodec::supports_extension("osu"));
}

#[test]
fn test_empty_extensions() {
    assert!(EmptyFormat::EXTENSIONS.is_empty());
    assert!(!EmptyFormat::supports_extension("anything"));
}
