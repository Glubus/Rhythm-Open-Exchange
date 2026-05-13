#![warn(clippy::pedantic)]

use std::path::Path;

use rox::error::{RoxError, RoxResult};

/// Format kind detected from a file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedFormat {
    Osu,
    Sm,
    Qua,
    Fnf,
    Jrox,
    Yrox,
    Rox,
    Mc,
}

/// Detect format from the file extension of a path.
///
/// # Errors
///
/// Returns `UnsupportedFormat` if the extension is missing or unknown.
pub fn detect(path: &Path) -> RoxResult<DetectedFormat> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext.to_ascii_lowercase().as_str() {
        "osu" => Ok(DetectedFormat::Osu),
        "sm" => Ok(DetectedFormat::Sm),
        "qua" => Ok(DetectedFormat::Qua),
        "json" => Ok(DetectedFormat::Fnf),
        "jrox" => Ok(DetectedFormat::Jrox),
        "yrox" => Ok(DetectedFormat::Yrox),
        "rox" => Ok(DetectedFormat::Rox),
        "mc" => Ok(DetectedFormat::Mc),
        other => Err(RoxError::UnsupportedFormat(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("song.osu", DetectedFormat::Osu)]
    #[case("song.sm", DetectedFormat::Sm)]
    #[case("song.qua", DetectedFormat::Qua)]
    #[case("song.json", DetectedFormat::Fnf)]
    #[case("song.jrox", DetectedFormat::Jrox)]
    #[case("song.yrox", DetectedFormat::Yrox)]
    #[case("song.rox", DetectedFormat::Rox)]
    #[case("song.mc", DetectedFormat::Mc)]
    #[case("SONG.OSU", DetectedFormat::Osu)]
    fn test_detect_known(#[case] name: &str, #[case] expected: DetectedFormat) {
        assert_eq!(detect(Path::new(name)).unwrap(), expected);
    }

    #[rstest]
    #[case("song.mp3")]
    #[case("no_ext")]
    fn test_detect_unknown(#[case] name: &str) {
        assert!(detect(Path::new(name)).is_err());
    }
}
