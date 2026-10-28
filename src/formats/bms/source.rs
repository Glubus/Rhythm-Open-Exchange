#![warn(clippy::pedantic)]
//! Text decoding of a BMS file.
//!
//! The format does not name its charset. The rule is deterministic and uses the bytes only:
//!
//! 1. a UTF-8 or UTF-16 byte order mark decides;
//! 2. bytes that are valid UTF-8 are UTF-8 (pure ASCII falls here);
//! 3. otherwise `Shift_JIS` (Windows-31J, what almost every Japanese chart uses), when every byte sequence is valid in it;
//! 4. otherwise EUC-KR (Windows-949, Korean charts), when every byte sequence is valid in it;
//! 5. otherwise `Shift_JIS` with U+FFFD for the bytes it cannot read.

use std::borrow::Cow;

use encoding_rs::{EUC_KR, SHIFT_JIS, UTF_8, UTF_16BE, UTF_16LE};

/// The text of a BMS file and the name of the charset it was read with.
pub(super) struct Text<'a> {
    pub text: Cow<'a, str>,
    pub charset: &'static str,
}

pub(super) fn decode(data: &[u8]) -> Text<'_> {
    if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (text, _) = UTF_8.decode_with_bom_removal(data);
        return Text {
            text,
            charset: "UTF-8",
        };
    }
    if data.starts_with(&[0xFF, 0xFE]) {
        let (text, _) = UTF_16LE.decode_with_bom_removal(data);
        return Text {
            text,
            charset: "UTF-16LE",
        };
    }
    if data.starts_with(&[0xFE, 0xFF]) {
        let (text, _) = UTF_16BE.decode_with_bom_removal(data);
        return Text {
            text,
            charset: "UTF-16BE",
        };
    }
    if let Ok(text) = std::str::from_utf8(data) {
        return Text {
            text: Cow::Borrowed(text),
            charset: "UTF-8",
        };
    }
    if let Some(text) = SHIFT_JIS.decode_without_bom_handling_and_without_replacement(data) {
        return Text {
            text,
            charset: "Shift_JIS",
        };
    }
    if let Some(text) = EUC_KR.decode_without_bom_handling_and_without_replacement(data) {
        return Text {
            text,
            charset: "EUC-KR",
        };
    }
    let (text, _) = SHIFT_JIS.decode_without_bom_handling(data);
    Text {
        text,
        charset: "Shift_JIS (lossy)",
    }
}
