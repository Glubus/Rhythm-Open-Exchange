//! Auto-detection module for format conversion based on file extension.
//!
//! Provides automatic decoding and encoding based on file extensions.

mod decode;
mod encode;
mod types;

pub use decode::{auto_decode, decode_with_format, from_bytes, from_string};
pub use encode::{auto_convert, auto_encode, encode_with_format};
pub use types::{InputFormat, OutputFormat};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RoxChart;
    use tempfile::tempdir;

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
        #[cfg(feature = "compression")]
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
        #[cfg(feature = "compression")]
        assert_eq!(
            OutputFormat::from_extension("rox").unwrap(),
            OutputFormat::Rox
        );
        assert!(OutputFormat::from_extension("mp3").is_err());
    }

    #[test]
    fn test_auto_decode_osu_mania() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.osu");
        let data = crate::test_utils::get_test_asset("osu/mania_7k.osu");
        std::fs::write(&path, data).unwrap();

        let chart = auto_decode(&path).unwrap();
        assert_eq!(chart.key_count(), 7);
    }

    #[test]
    fn test_auto_decode_sm() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.sm");
        let data = crate::test_utils::get_test_asset("stepmania/4k.sm");
        std::fs::write(&path, data).unwrap();

        let chart = auto_decode(&path).unwrap();
        assert_eq!(chart.key_count(), 4);
    }

    #[test]
    fn test_auto_encode_osu() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.osu");
        let chart = RoxChart::new(4);

        auto_encode(&chart, &path).unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("osu file format v14"));
    }

    #[test]
    fn test_from_string_detection() {
        let data = crate::test_utils::get_test_asset("osu/mania_7k.osu");
        let s = String::from_utf8(data).unwrap();
        let chart = from_string(&s).unwrap();
        assert_eq!(chart.key_count(), 7);
    }

    #[test]
    fn test_from_bytes_detection() {
        let data = crate::test_utils::get_test_asset("osu/mania_7k.osu");
        let chart = from_bytes(&data).unwrap();
        assert_eq!(chart.key_count(), 7);
    }

    #[test]
    fn test_detect_osu_mode() {
        use super::decode::detect_osu_mode;
        let mania_data = b"Mode: 3\n[Metadata]";
        assert_eq!(detect_osu_mode(mania_data), 3);

        let taiko_data = b"Mode: 1\n[Metadata]";
        assert_eq!(detect_osu_mode(taiko_data), 1);

        let empty_data = b"";
        assert_eq!(detect_osu_mode(empty_data), 3); // Default
    }

    #[test]
    fn test_auto_convert() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("input.osu");
        let output = dir.path().join("output.sm");

        let data = crate::test_utils::get_test_asset("osu/mania_7k.osu");
        std::fs::write(&input, data).unwrap();

        auto_convert(&input, &output).unwrap();
        assert!(output.exists());
        let content = std::fs::read_to_string(&output).unwrap();
        assert!(content.contains("#TITLE:"));
    }
}
