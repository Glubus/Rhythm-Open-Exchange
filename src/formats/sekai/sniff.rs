#![warn(clippy::pedantic)]
//! What the first bytes of a `.sus` or a `.usc` say: `.sus` and `.usc` are generic extensions, so a caller that lists files (Prism's
//! library scan) asks before it counts a file as a chart, and a file that is not one is no failure. See `mod.rs`.

/// How many bytes of the start of a file [`looks_like_sus`] and [`looks_like_usc`] need.
pub const SNIFF_BYTES: usize = 64 * 1024;

fn without_bom(head: &[u8]) -> &[u8] {
    head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head)
}

/// Whether `head` (the first [`SNIFF_BYTES`] of a file, or all of it) looks like a `.sus`: text with no NUL byte and a line that starts with
/// `#` and a letter or a digit (`#TITLE`, `#BPM01:`, `#00010:`), which is all the notes require of a SUS. A SUS of Chunithm (Seaurchin, ched)
/// looks the same and passes: nothing in the first bytes tells the dialects apart.
#[must_use]
pub fn looks_like_sus(head: &[u8]) -> bool {
    let head = without_bom(head);
    !head.contains(&0)
        && head
            .split(|&byte| byte == b'\n' || byte == b'\r')
            .any(|line| {
                let line = line.trim_ascii_start();
                line.first() == Some(&b'#') && line.get(1).is_some_and(u8::is_ascii_alphanumeric)
            })
}

/// Whether `head` (the first [`SNIFF_BYTES`] of a file, or all of it) looks like a `.usc`: a JSON object (the first byte that is not
/// white space is `{`) with an `"objects"` key, bare or inside the editors' `{"usc": ...}` wrapper.
#[must_use]
pub fn looks_like_usc(head: &[u8]) -> bool {
    let head = without_bom(head).trim_ascii_start();
    head.first() == Some(&b'{')
        && head
            .windows(b"\"objects\"".len())
            .any(|window| window == b"\"objects\"")
}
