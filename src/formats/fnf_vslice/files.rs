#![warn(clippy::pedantic)]
//! File-name conventions of V-Slice songs (`SongRegistry`: `$id-chart${variation == 'default' ? '' : '-$variation'}`).

/// The variation of a V-Slice chart file name: `Some("default")` for `<song>-chart.json`, `Some(<variation>)` for
/// `<song>-chart-<variation>.json`, `None` for any other name. Case-insensitive; the variation keeps the case of `file_name`.
/// A directory part of `file_name` is ignored. Pure: no file is read, so a legacy `.json` that happens to be named like this
/// is not told apart here (`auto_decode` checks the content).
#[must_use]
pub fn is_vslice_chart_name(file_name: &str) -> Option<&str> {
    split_chart_name(file_name).map(|(_, variation)| variation)
}

/// The metadata file name that goes with a chart file name: `<song>-chart.json` -> `<song>-metadata.json`,
/// `<song>-chart-<variation>.json` -> `<song>-metadata-<variation>.json`. `None` when `chart_name` is not a V-Slice chart name.
#[must_use]
pub fn vslice_metadata_name(chart_name: &str) -> Option<String> {
    let (song, variation) = split_chart_name(chart_name)?;
    Some(if variation == "default" {
        format!("{song}-metadata.json")
    } else {
        format!("{song}-metadata-{variation}.json")
    })
}

/// `(song id, variation)` of a chart file name.
fn split_chart_name(file_name: &str) -> Option<(&str, &str)> {
    let name = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    // ASCII lower-casing keeps every byte offset.
    let lower = name.to_ascii_lowercase();
    let stem = lower.strip_suffix(".json")?;
    if let Some(song) = stem.strip_suffix("-chart") {
        return (!song.is_empty()).then(|| (&name[..song.len()], "default"));
    }
    let at = stem.rfind("-chart-")?;
    let variation = &name[at + "-chart-".len()..stem.len()];
    (at > 0 && !variation.is_empty()).then(|| (&name[..at], variation))
}

/// The file-name suffix of a variation: empty for the default one (`""` or `"default"`), else `-<variation>`.
pub(super) fn suffix(variation: &str) -> String {
    if variation.is_empty() || variation == "default" {
        String::new()
    } else {
        format!("-{variation}")
    }
}

/// Whether JSON `bytes` look like a V-Slice chart rather than a legacy `{ "song": ... }` one: no top-level `song`. Bytes that
/// are not JSON at all count as V-Slice (the V-Slice decoder then reports the syntax error).
#[must_use]
pub fn looks_like_vslice_chart(bytes: &[u8]) -> bool {
    #[derive(serde::Deserialize)]
    struct Probe {
        song: Option<serde::de::IgnoredAny>,
    }
    serde_json::from_slice::<Probe>(bytes).is_ok_and(|probe| probe.song.is_none())
        || serde_json::from_slice::<serde::de::IgnoredAny>(bytes).is_err()
}
