#![warn(clippy::pedantic)]
//! What the first bytes of a file say about being a K-Shoot MANIA `.ksh`: `.ksh` is ALSO the extension of Korn shell scripts, so a caller that
//! lists files (Prism's library scan) asks before it counts a file as a chart. See `mod.rs`.

use std::io::Read;
use std::path::Path;

use crate::formats::sekai::SNIFF_BYTES;

/// Whether `head` (the first [`SNIFF_BYTES`] of a file, or all of it) looks like a K-Shoot MANIA chart, which is what the format says its
/// start is: the FIRST line is `title=...` (the format: "this option must be placed at the beginning of a KSH chart file. No comments are
/// allowed before this option"), every line up to the first bar line `--` is an option line `name=value` or a `//` comment, and that bar
/// line is there. A byte 0 is not text. A shell script fails at its first line (`#!/bin/ksh`, a comment, a command), and one that
/// begins with `title=` and only assignments would still need a line that is exactly `--`.
///
/// When `head` is cut (it is [`SNIFF_BYTES`] long) before the bar line, the lines it holds are still checked and the file passes: the
/// decoder reads the rest.
#[must_use]
pub fn looks_like_ksh(head: &[u8]) -> bool {
    if head.contains(&0) {
        return false;
    }
    let head = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    let cut = head.len() >= SNIFF_BYTES;
    let mut lines: Vec<&[u8]> = head
        .split(|&byte| byte == b'\n' || byte == b'\r')
        .map(<[u8]>::trim_ascii)
        .filter(|line| !line.is_empty())
        .collect();
    if cut {
        // The last line may be cut in the middle.
        lines.pop();
    }
    let mut lines = lines.into_iter();
    if !lines
        .next()
        .is_some_and(|first| first.starts_with(b"title="))
    {
        return false;
    }
    for line in lines {
        if line == b"--" {
            return true;
        }
        let option = line
            .iter()
            .position(|&byte| byte == b'=')
            .is_some_and(|at| {
                at > 0
                    && line[..at]
                        .iter()
                        .all(|&byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            });
        if !option && !line.starts_with(b"//") {
            return false;
        }
    }
    cut
}

/// Whether the file at `path` is a K-Shoot MANIA chart: its extension is `.ksh` (any case) and the file [`looks_like_ksh`]. Reads the
/// first bytes of the file; `false` when it can not be read.
#[must_use]
pub fn is_ksh_chart_file(path: &Path) -> bool {
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ksh"))
    {
        return false;
    }
    let mut head = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(SNIFF_BYTES as u64).read_to_end(&mut head))
        .is_ok_and(|_| looks_like_ksh(&head))
}
