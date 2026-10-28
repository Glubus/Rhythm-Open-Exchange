#![warn(clippy::pedantic)]

use std::path::Path;

use rox::error::{RoxError, RoxResult};

/// Format kind detected from a file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedFormat {
    Osu,
    Sm,
    Ssc,
    Qua,
    Fnf,
    Jrox,
    Yrox,
    Rox,
    Mc,
    /// `.bms`, `.bme`, `.bml`.
    Bms,
    /// `.pms`.
    Pms,
    /// `.bmson`.
    Bmson,
    /// `.ojn` (`O2Jam`): three charts in one file. Decode only.
    Ojn,
    /// `.fnfc` (FNF V-Slice package, a ZIP): decoded as its default variation. Decode only. A loose V-Slice chart
    /// (`<song>-chart.json`) is still `Fnf` here: `auto_decode` tells it apart by name with `is_vslice_chart_name`.
    Fnfc,
    /// `.chart` (Clone Hero / Moonscraper): several tracks in one file, read with the `song.ini` beside it. Decode only.
    CloneHero,
    /// `.sng` (Clone Hero package, holds a `notes.chart`): read with its own metadata. Decode only.
    Sng,
    /// `.edit` (StepMania edit chart): a fragment that names its song with `#SONG` and has no timing or metadata of its own, so a
    /// path alone can not be decoded: `auto_decode` fails with `InvalidFormat("edit needs its song")`; read it with
    /// `EditDecoder` and the song's `.sm`/`.ssc`.
    Edit,
    /// `.ssq` (DDR step file): every Single and Double chart of a song in one file, read without title or audio. Decode only.
    Ssq,
    /// `.dwi` (Dance With Intensity): every Single, Solo and Double chart of a song in one file. Decode only.
    Dwi,
    /// `.ksf` (Kick It Up / Direct Move): ONE chart per file; its step type and difficulty come from the file name. Decode only.
    Ksf,
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
        "ssc" => Ok(DetectedFormat::Ssc),
        "qua" => Ok(DetectedFormat::Qua),
        "json" => Ok(DetectedFormat::Fnf),
        "jrox" => Ok(DetectedFormat::Jrox),
        "yrox" => Ok(DetectedFormat::Yrox),
        "rox" => Ok(DetectedFormat::Rox),
        "mc" => Ok(DetectedFormat::Mc),
        "bms" | "bme" | "bml" => Ok(DetectedFormat::Bms),
        "pms" => Ok(DetectedFormat::Pms),
        "bmson" => Ok(DetectedFormat::Bmson),
        "ojn" => Ok(DetectedFormat::Ojn),
        "chart" => Ok(DetectedFormat::CloneHero),
        "fnfc" => Ok(DetectedFormat::Fnfc),
        "sng" => Ok(DetectedFormat::Sng),
        "edit" => Ok(DetectedFormat::Edit),
        "ssq" => Ok(DetectedFormat::Ssq),
        "dwi" => Ok(DetectedFormat::Dwi),
        "ksf" => Ok(DetectedFormat::Ksf),
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
    #[case("song.ssc", DetectedFormat::Ssc)]
    #[case("SONG.SSC", DetectedFormat::Ssc)]
    #[case("song.qua", DetectedFormat::Qua)]
    #[case("song.json", DetectedFormat::Fnf)]
    #[case("song.jrox", DetectedFormat::Jrox)]
    #[case("song.yrox", DetectedFormat::Yrox)]
    #[case("song.rox", DetectedFormat::Rox)]
    #[case("song.mc", DetectedFormat::Mc)]
    #[case("song.bms", DetectedFormat::Bms)]
    #[case("song.BME", DetectedFormat::Bms)]
    #[case("song.bml", DetectedFormat::Bms)]
    #[case("song.pms", DetectedFormat::Pms)]
    #[case("song.BMSON", DetectedFormat::Bmson)]
    #[case("song.ojn", DetectedFormat::Ojn)]
    #[case("song.chart", DetectedFormat::CloneHero)]
    #[case("NOTES.CHART", DetectedFormat::CloneHero)]
    #[case("song.sng", DetectedFormat::Sng)]
    #[case("SONG.SNG", DetectedFormat::Sng)]
    #[case("song.fnfc", DetectedFormat::Fnfc)]
    #[case("song.edit", DetectedFormat::Edit)]
    #[case("My Edit.EDIT", DetectedFormat::Edit)]
    #[case("a01_all.ssq", DetectedFormat::Ssq)]
    #[case("A01_ALL.SSQ", DetectedFormat::Ssq)]
    #[case("song.dwi", DetectedFormat::Dwi)]
    #[case("SONG.DWI", DetectedFormat::Dwi)]
    #[case("Crazy_1.ksf", DetectedFormat::Ksf)]
    #[case("DOUBLE.KSF", DetectedFormat::Ksf)]
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
