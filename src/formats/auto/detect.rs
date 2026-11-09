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
    /// `notes.mid` (Clone Hero / Rock Band MIDI chart, matched by the file NAME, case-insensitively: any other `.mid` is an
    /// ordinary MIDI file and stays unsupported): several tracks in one file, read with the `song.ini` beside it. Decode only.
    CloneHeroMid,
    /// `.sng` (Clone Hero package, holds a `notes.mid` or a `notes.chart`): read with its own metadata. Decode only.
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
    /// `.sus` (Sliding Universal Score, the Project SEKAI dialect): ONE chart per file on 12 columns. Decode only.
    Sus,
    /// `.usc` (Universal Sekai Chart, JSON): ONE chart per file on 12 columns. Decode only.
    Usc,
    /// A Bestdori chart (BanG Dream, the JSON array of `BPM`/`Single`/`Long`/`Slide`/`Directional`): ONE chart per file on 7 columns. Decode only.
    /// `detect` says it only for the names no other format uses (`.bestdori`, `.bestdori.json`); `detect_content` also for the other names of
    /// a Bestdori chart (`expert.json`) when the content agrees. Any other `.json` stays `Fnf`.
    Bestdori,
    /// A BanG! Simulator `.txt` score (BanG Dream): ONE chart per file on 7 columns. Decode only. `detect` never says it (`.txt` is the most generic
    /// extension there is): `detect_content` does, when the first lines are a simulator score.
    BangSimulator,
    /// `.imd` (Rhythm Master classic mode): ONE chart per file on 4 to 6 columns; the lane count and the difficulty come from the file name
    /// (`<song>_<4|5|6>k_<ez|nm|hd>.imd`). Decode only. The extension is told by `detect`; whether a file is really one is `looks_like_imd`.
    Imd,
    /// `.dtx` (`DTXMania`): drums, guitar and bass of one song in one file, each instrument that has chips a variant. Decode only.
    Dtx,
    /// `.gda` and `.g2d` (`DTXMania`'s older `GuitarFreaks` / `DrumMania` files): the DTX grammar with two-letter channel names; one instrument per
    /// file in practice, read as a `.dtx`. Decode only.
    Gda,
    /// `.ksh` (`K-Shoot MANIA`, SOUND VOLTEX fan charts): ONE chart per file on 6 columns (BT A-D, FX-L, FX-R); the lasers are dropped. Decode only.
    /// The extension is also a Korn shell script's, so `detect` says it and whether a file is really one is `looks_like_ksh`.
    Ksh,
    /// A BDON chart (BanG Dream! Our Notes): ONE chart per file on the 12 lanes of the Project SEKAI model. Decode only. `detect` says it for
    /// `.bdon`; `detect_content` for a `.json` whose content is one (a JSON object with a `score` key).
    Bdon,
    /// `.msd` (the DDR'99 simulator script DWI grew from): the charts of a song, each playable one a variant (as `.dwi`). Decode only.
    Msd,
    /// `.step` (DDR 2nd to 3rd Mix Plus, "DDR Utility" export, binary): the charts of a song, each playable one a variant. Decode only.
    Step,
    /// `.xmk` (DJ Hero 2) and `.fsgmub` (DJ Hero): ONE chart per file on 5 columns (crossfader left, green, red, blue, crossfader right). Decode only.
    Xmk,
    /// `.fsc` (fluXis): ONE chart per file, as many columns as its highest lane. Decode only.
    Fsc,
}

/// Detect format from the file extension of a path (`.mid` is the exception: only a file named `notes.mid` is a chart).
///
/// # Errors
///
/// Returns `UnsupportedFormat` if the extension is missing or unknown.
pub fn detect(path: &Path) -> RoxResult<DetectedFormat> {
    if path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.to_ascii_lowercase().ends_with(".bestdori.json"))
    {
        return Ok(DetectedFormat::Bestdori);
    }
    if path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(crate::formats::clone_hero::is_mid_chart_name)
    {
        return Ok(DetectedFormat::CloneHeroMid);
    }
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
        "step" => Ok(DetectedFormat::Step),
        "dwi" => Ok(DetectedFormat::Dwi),
        "msd" => Ok(DetectedFormat::Msd),
        "ksf" => Ok(DetectedFormat::Ksf),
        "sus" => Ok(DetectedFormat::Sus),
        "usc" => Ok(DetectedFormat::Usc),
        "imd" => Ok(DetectedFormat::Imd),
        "dtx" => Ok(DetectedFormat::Dtx),
        "gda" | "g2d" => Ok(DetectedFormat::Gda),
        "ksh" => Ok(DetectedFormat::Ksh),
        "xmk" | "fsgmub" => Ok(DetectedFormat::Xmk),
        "fsc" => Ok(DetectedFormat::Fsc),
        "bestdori" => Ok(DetectedFormat::Bestdori),
        "bdon" => Ok(DetectedFormat::Bdon),
        other => Err(RoxError::UnsupportedFormat(other.to_string())),
    }
}

/// Whether `path` is a file `detect_content` may tell by its content: a `.txt`, or a `.json` (a Bestdori chart is saved under any name).
pub(super) fn is_content_candidate(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("txt") || ext.eq_ignore_ascii_case("json"))
        || path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(crate::formats::bandori::is_bestdori_chart_name)
}

/// The formats `detect` does not claim because their extension is generic, told by the first bytes of the file `head` (or all of it): a
/// BanG! Simulator score for a `.txt` ([`crate::formats::looks_like_simulator`]), a Bestdori chart for a `.json` or a `.bestdori` whose content
/// is one ([`crate::formats::looks_like_bestdori`]: a JSON array of Bestdori notes; an FNF chart, a V-Slice file or a `.jrox` is an object), a BDON
/// chart for a `.json` ([`crate::formats::looks_like_bdon`]: an object with a `score` key). `None` for anything else, an ordinary `.txt` or `.json`
/// included.
#[must_use]
pub fn detect_content(path: &Path, head: &[u8]) -> Option<DetectedFormat> {
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"))
    {
        return crate::formats::bandori::looks_like_simulator(head)
            .then_some(DetectedFormat::BangSimulator);
    }
    if !is_content_candidate(path) {
        return None;
    }
    if crate::formats::bandori::looks_like_bestdori(head) {
        Some(DetectedFormat::Bestdori)
    } else {
        crate::formats::sekai::looks_like_bdon(head).then_some(DetectedFormat::Bdon)
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
    #[case("notes.mid", DetectedFormat::CloneHeroMid)]
    #[case("songs/Some Song/NOTES.MID", DetectedFormat::CloneHeroMid)]
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
    #[case("master.sus", DetectedFormat::Sus)]
    #[case("MASTER.SUS", DetectedFormat::Sus)]
    #[case("master.usc", DetectedFormat::Usc)]
    #[case("MASTER.USC", DetectedFormat::Usc)]
    #[case("DOUBLE.KSF", DetectedFormat::Ksf)]
    #[case("keluodiya_4k_hd.imd", DetectedFormat::Imd)]
    #[case("SONG_6K_EZ.IMD", DetectedFormat::Imd)]
    #[case("drums.dtx", DetectedFormat::Dtx)]
    #[case("DRUMS.DTX", DetectedFormat::Dtx)]
    #[case("exh.ksh", DetectedFormat::Ksh)]
    #[case("INF.KSH", DetectedFormat::Ksh)]
    #[case("SONG.OSU", DetectedFormat::Osu)]
    #[case("song.bestdori", DetectedFormat::Bestdori)]
    #[case("Song.Bestdori.JSON", DetectedFormat::Bestdori)]
    #[case("expert.json", DetectedFormat::Fnf)]
    fn test_detect_known(#[case] name: &str, #[case] expected: DetectedFormat) {
        assert_eq!(detect(Path::new(name)).unwrap(), expected);
    }

    #[rstest]
    #[case("song.mp3")]
    #[case("song.mid")]
    #[case("my notes.mid")]
    #[case("notes.mid.bak")]
    #[case("readme.txt")]
    #[case("score.txt")]
    #[case("no_ext")]
    fn test_detect_unknown(#[case] name: &str) {
        assert!(detect(Path::new(name)).is_err());
    }
}
