#![warn(clippy::pedantic)]
//! How a BanG Dream chart is told from any other `.json` or `.txt`: by its NAME for a Bestdori JSON (the notes give no convention, see
//! `mod.rs`) and by its CONTENT for both. A caller that lists files (Prism's library scan) asks before it counts a file as a chart, and a
//! file that is not one is no failure.

use std::io::Read;
use std::path::Path;

/// How many bytes of the start of a file [`looks_like_simulator`] reads at most (a score's first lines tell it).
const SIMULATOR_SNIFF_BYTES: usize = 4096;
/// The note types a Bestdori chart names in its `type` fields.
const BESTDORI_TYPES: [&[u8]; 5] = [
    b"\"BPM\"",
    b"\"Single\"",
    b"\"Long\"",
    b"\"Slide\"",
    b"\"Directional\"",
];
/// The difficulties of Bestdori's chart API (`/api/charts/<id>/<difficulty>.json`, `bestdori-api-go` `ChartDifficultyName`).
const DIFFICULTIES: [&str; 5] = ["easy", "normal", "hard", "expert", "special"];

fn without_bom(head: &[u8]) -> &[u8] {
    head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Whether `name` (a file name) is one a Bestdori chart is listed under: `<name>.bestdori`, `<name>.bestdori.json`, or the name the site's API
/// gives a chart, `<difficulty>.json` (`easy`, `normal`, `hard`, `expert`, `special`) or `<id>.<difficulty>.json`, any case. The content decides
/// ([`looks_like_bestdori`]): a `.json` of any other name that is a Bestdori chart is read as one too (`detect_content`).
#[must_use]
pub fn is_bestdori_chart_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if let Some(stem) = lower.strip_suffix(".bestdori") {
        return !stem.is_empty();
    }
    let Some(stem) = lower.strip_suffix(".json") else {
        return false;
    };
    if let Some(stem) = stem.strip_suffix(".bestdori") {
        return !stem.is_empty();
    }
    let difficulty = stem.rsplit('.').next().unwrap_or("");
    DIFFICULTIES.contains(&difficulty)
}

/// Whether `head` (the first `SNIFF_BYTES` of a file, or all of it) looks like a Bestdori chart: a JSON array whose first
/// element is an object and that names one of the notes' types (`"BPM"`, `"Single"`, `"Long"`, `"Slide"`, `"Directional"`) in a `"type"` field.
/// An object (a legacy FNF chart, a `package.json`, a `.jrox`, a Project SEKAI `.usc`), the empty array and any other array fail.
#[must_use]
pub fn looks_like_bestdori(head: &[u8]) -> bool {
    let head = without_bom(head).trim_ascii_start();
    let Some(rest) = head.strip_prefix(b"[") else {
        return false;
    };
    !head.contains(&0)
        && rest.trim_ascii_start().first() == Some(&b'{')
        && contains(head, b"\"type\"")
        && BESTDORI_TYPES.iter().any(|name| contains(head, name))
}

/// A finite number, in the text of a simulator line.
fn is_number(field: &str) -> bool {
    field.trim().parse::<f64>().is_ok_and(f64::is_finite)
}

/// A line `beat/type/lane` (`beat/type/number/lane` for the numbered slides, types 71 to 77, and only for them).
fn is_note_line(text: &str) -> bool {
    let fields: Vec<&str> = text.split('/').collect();
    let kind = |field: &str| field.trim().parse::<u32>().ok();
    match fields.as_slice() {
        [beat, ty, lane] => {
            is_number(beat)
                && kind(ty).is_some_and(|ty| !(71..=77).contains(&ty))
                && is_number(lane)
        }
        [beat, ty, number, lane] => {
            is_number(beat)
                && kind(ty).is_some_and(|ty| (71..=77).contains(&ty))
                && kind(number).is_some()
                && is_number(lane)
        }
        _ => false,
    }
}

/// Whether `head` (the first bytes of a file, or all of it) looks like a BanG! Simulator score: text with no NUL byte, a number (the note
/// offset), a positive number (the BPM), then at least one note line, and EVERY line after those two is a note line `beat/type/lane`
/// (a line cut by the end of the sniffed bytes does not count). Blank lines are skipped. Any ordinary `.txt` fails.
#[must_use]
pub fn looks_like_simulator(head: &[u8]) -> bool {
    let head = without_bom(head);
    let cut = head.len() > SIMULATOR_SNIFF_BYTES;
    let head = &head[..head.len().min(SIMULATOR_SNIFF_BYTES)];
    if head.contains(&0) {
        return false;
    }
    let text = String::from_utf8_lossy(head);
    let mut lines: Vec<&str> = text.lines().collect();
    if cut {
        // The last line may be cut in the middle.
        lines.pop();
    }
    let mut lines = lines
        .into_iter()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let (Some(offset), Some(bpm), Some(first)) = (lines.next(), lines.next(), lines.next()) else {
        return false;
    };
    is_number(offset)
        && bpm
            .trim()
            .parse::<f64>()
            .is_ok_and(|bpm| bpm.is_finite() && bpm > 0.0)
        && is_note_line(first)
        && lines.all(is_note_line)
}

/// Whether the file at `path` is a BanG Dream chart: a `.txt` that [`looks_like_simulator`], a `.json` (any name) or a file with a Bestdori
/// name ([`is_bestdori_chart_name`]) that [`looks_like_bestdori`], or a `.json` or `.bdon` that is a BDON chart (`crate::formats::looks_like_bdon`,
/// BanG Dream! Our Notes). Reads the first bytes of the file; `false` when it can not be read.
#[must_use]
pub fn is_bandori_chart_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let extension = |wanted: &str| path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case(wanted));
    let looks_like: fn(&[u8]) -> bool = if extension("txt") {
        looks_like_simulator
    } else if extension("json") {
        |head| looks_like_bestdori(head) || crate::formats::looks_like_bdon(head)
    } else if is_bestdori_chart_name(name) {
        looks_like_bestdori
    } else if extension("bdon") {
        crate::formats::looks_like_bdon
    } else {
        return false;
    };
    let mut head = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| {
            file.take(crate::formats::sekai::SNIFF_BYTES as u64)
                .read_to_end(&mut head)
        })
        .is_ok()
        && looks_like(&head)
}
