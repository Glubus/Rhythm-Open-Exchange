#![warn(clippy::pedantic)]
//! Helpers of the `.dwi` and `.ksf` readers: the number parsing and the text decoding of
//! StepMania's loaders for those two formats (`StringToInt`, `StringToFloat`, `BeatToNoteRow`,
//! `HHMMSSToSeconds`, `ConvertString`), so that values the loaders read leniently are read the same.
use std::borrow::Cow;

use super::types::timing::ROWS_PER_BEAT;

/// The text of a file: UTF-8 when valid, else (`korean`) EUC-KR/CP949 or Latin-1. A byte-order
/// mark is removed. Undecodable bytes never reach the notes, which are ASCII.
pub(crate) fn decode_text(data: &[u8], korean: bool) -> Cow<'_, str> {
    let data = data.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(data);
    if let Ok(text) = core::str::from_utf8(data) {
        return Cow::Borrowed(text);
    }
    if korean {
        let (text, _, _) = encoding_rs::EUC_KR.decode(data);
        return text;
    }
    Cow::Owned(data.iter().map(|&byte| char::from(byte)).collect())
}

/// The longest prefix of `text` that is a decimal number (after leading blanks), as `strtof`
/// reads it; 0 when there is none.
pub(crate) fn string_to_float(text: &str) -> f32 {
    let text = text.trim_start();
    let bytes = text.as_bytes();
    let mut end = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        end += 1;
    }
    let digits = |from: usize| {
        bytes[from..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    let integer = digits(end);
    end += integer;
    let mut fraction = 0;
    if bytes.get(end) == Some(&b'.') {
        fraction = digits(end + 1);
        if integer + fraction > 0 {
            end += 1 + fraction;
        }
    }
    if integer + fraction == 0 {
        return 0.0;
    }
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let mut exponent = end + 1;
        if matches!(bytes.get(exponent), Some(b'+' | b'-')) {
            exponent += 1;
        }
        let exponent_digits = digits(exponent);
        if exponent_digits > 0 {
            end = exponent + exponent_digits;
        }
    }
    text[..end]
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
}

/// The leading integer of `text` (after leading blanks), as `atoi` reads it: `12.5` is 12; 0
/// when there is none.
pub(crate) fn string_to_int(text: &str) -> i32 {
    let text = text.trim_start();
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let value = digits
        .bytes()
        .take_while(u8::is_ascii_digit)
        .fold(0_i64, |acc, digit| {
            (acc * 10 + i64::from(digit - b'0')).min(i64::from(i32::MAX))
        });
    #[allow(clippy::cast_possible_truncation)] // clamped to i32 above
    let value = value as i32;
    if negative { -value } else { value }
}

/// `BeatToNoteRow`: the row (1/48 beat) nearest to `beat`, halves to even like `lrintf`.
pub(crate) fn beat_to_row(beat: f32) -> i64 {
    #[allow(clippy::cast_possible_truncation)] // a beat is far below 2^52 rows
    let row = (beat * ROWS_PER_BEAT as f32).round_ties_even() as i64;
    row
}

/// The beat of a row.
#[allow(clippy::cast_precision_loss)] // rows are far below 2^52
pub(crate) fn row_to_beat(row: i64) -> f64 {
    row as f64 / ROWS_PER_BEAT
}

/// `HHMMSSToSeconds`: `SS.ss`, `MM:SS.ss` or `HH:MM:SS.ss`.
pub(crate) fn hhmmss_to_seconds(text: &str) -> f32 {
    text.split(':')
        .map(string_to_float)
        .fold(0.0, |total, part| total * 60.0 + part)
}

/// `split(text, delimiter, parts, true)`: the non-empty pieces.
pub(crate) fn split_non_empty(text: &str, delimiter: char) -> impl Iterator<Item = &str> {
    text.split(delimiter).filter(|piece| !piece.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_read_by_prefix() {
        assert_eq!(string_to_int(" -300ms"), -300);
        assert_eq!(string_to_int("12.9"), 12);
        assert_eq!(string_to_int("abc"), 0);
        assert!((string_to_float("150.5x") - 150.5).abs() < 1e-6);
        assert!((string_to_float("1e2") - 100.0).abs() < 1e-6);
        assert!((string_to_float(".5") - 0.5).abs() < 1e-6);
        assert_eq!(string_to_float("."), 0.0);
        assert_eq!(string_to_float("-"), 0.0);
        assert_eq!(string_to_float(""), 0.0);
    }

    #[test]
    fn rows_and_clock_values() {
        assert_eq!(beat_to_row(0.5), 24);
        assert_eq!(beat_to_row(1.0 / 48.0), 1);
        assert!((hhmmss_to_seconds("1:05.5") - 65.5).abs() < 1e-4);
        assert!((hhmmss_to_seconds("1:00:00") - 3600.0).abs() < 1e-4);
        assert!((hhmmss_to_seconds("42") - 42.0).abs() < 1e-6);
    }

    #[test]
    fn text_falls_back_to_latin_1_or_euc_kr() {
        assert_eq!(decode_text(b"caf\xe9", false), "café");
        assert_eq!(decode_text("\u{feff}ok".as_bytes(), false), "ok");
        let (korean, _, _) = encoding_rs::EUC_KR.encode("한국어");
        assert_eq!(decode_text(&korean, true), "한국어");
    }
}
