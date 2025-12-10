//! Tests for auto-detection format conversion.

use rhythm_open_exchange::codec::{InputFormat, OutputFormat};

#[test]
fn test_input_format_detection() {
    assert_eq!(
        InputFormat::from_extension("osu").unwrap(),
        InputFormat::Osu
    );
    assert_eq!(
        InputFormat::from_extension("OSU").unwrap(),
        InputFormat::Osu
    );
    assert_eq!(InputFormat::from_extension("sm").unwrap(), InputFormat::Sm);
    assert_eq!(
        InputFormat::from_extension("rox").unwrap(),
        InputFormat::Rox
    );
    assert!(InputFormat::from_extension("mp3").is_err());
}

#[test]
fn test_output_format_detection() {
    assert_eq!(
        OutputFormat::from_extension("osu").unwrap(),
        OutputFormat::Osu
    );
    assert_eq!(
        OutputFormat::from_extension("sm").unwrap(),
        OutputFormat::Sm
    );
    assert_eq!(
        OutputFormat::from_extension("rox").unwrap(),
        OutputFormat::Rox
    );
    assert!(OutputFormat::from_extension("mp3").is_err());
}
