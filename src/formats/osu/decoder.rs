#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap;
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Hitsound, Metadata, Note, RoxChart, TimingPoint};

use super::parser;

/// Options for [`OsuDecoder::decode_with_options`].
#[derive(Debug, Clone, Default)]
pub struct OsuDecodeOptions {
    /// Re-arrange the first BPM timing point to 1µs before the first note when
    /// it would otherwise fail the `BpmAfterFirstNote` validation.
    ///
    /// **Warning:** this silently repairs broken beatmaps. Prefer fixing the
    /// source file. Emits a `tracing::warn!` when the repair is applied.
    pub re_arrange_bpm: bool,
}

/// Decoder for osu!mania beatmaps.
pub struct OsuDecoder;
crate::impl_format!(OsuDecoder, ["osu"]);

impl OsuDecoder {
    /// The game mode a `.osu` file declares in its `[General]` section, read from its first lines only, so a file of
    /// another mode can be refused without decoding its timing points and hit objects (the bulk of the file).
    ///
    /// `data` is the start of the file when `complete` is false, the whole file when it is true. The answer is the one
    /// [`parser::parse`] would give: the last `Mode:` of `[General]` (`0`, osu!standard, when there is none or it is
    /// not a number), decided when the first section after `[General]` begins. `None` when `data` ends before that (more
    /// of the file is needed) or holds text that is not UTF-8 (the full parse reports that).
    #[must_use]
    pub fn peek_mode(data: &[u8], complete: bool) -> Option<u8> {
        let mut in_general = false;
        let mut mode = 0_u8;
        let mut rest = data;
        loop {
            let (line, next) = match memchr::memchr(b'\n', rest) {
                Some(end) => (&rest[..end], Some(&rest[end + 1..])),
                // The last line of a cut head may be a part of a line: only a whole file ends on one.
                None if complete => (rest, None),
                None => return None,
            };
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            let skippable = line.is_empty() || line.starts_with(b"//");
            if !skippable {
                if line.first() == Some(&b'[') && line.last() == Some(&b']') {
                    if in_general {
                        return Some(mode);
                    }
                    in_general = &line[1..line.len() - 1] == b"General";
                } else if in_general {
                    let text = core::str::from_utf8(line).ok()?.trim();
                    if let Some((key, value)) = text.split_once(':')
                        && key.trim() == "Mode"
                    {
                        mode = value.trim().parse().unwrap_or(0);
                    }
                }
            }
            match next {
                Some(next) => rest = next,
                None => return Some(mode),
            }
        }
    }

    /// Decode with explicit options.
    ///
    /// # Errors
    /// Returns an error if parsing or validation fails. With
    /// [`OsuDecodeOptions::re_arrange_bpm`] enabled, `BpmAfterFirstNote` is
    /// repaired instead of surfaced. A file of another game mode is refused from its header, before the rest is parsed.
    pub fn decode_with_options(data: &[u8], opts: &OsuDecodeOptions) -> RoxResult<RoxChart> {
        Self::refuse_other_mode(data)?;
        let beatmap = parser::parse(data)?;
        if beatmap.general.mode != 3 {
            return Err(not_mania(beatmap.general.mode));
        }
        let mut chart = from_beatmap(&beatmap);
        if opts.re_arrange_bpm {
            re_arrange_bpm_if_needed(&mut chart);
        }
        chart.validate()?;
        Ok(chart)
    }

    fn refuse_other_mode(data: &[u8]) -> RoxResult<()> {
        match Self::peek_mode(data, true) {
            Some(mode) if mode != 3 => Err(not_mania(mode)),
            _ => Ok(()),
        }
    }
}

fn not_mania(mode: u8) -> RoxError {
    RoxError::InvalidFormat(format!("Not a mania beatmap (mode={mode}, expected 3)"))
}

impl Decoder for OsuDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        Self::refuse_other_mode(data)?;
        let beatmap = parser::parse(data)?;
        if beatmap.general.mode != 3 {
            return Err(not_mania(beatmap.general.mode));
        }
        Ok(from_beatmap(&beatmap))
    }
}

/// Shifts the first BPM timing point to 1µs before the first note when it
/// would fail `BpmAfterFirstNote` validation.
fn re_arrange_bpm_if_needed(chart: &mut RoxChart) {
    let Some(first_note_time) = chart.notes.iter().map(|n| n.time_us).min() else {
        return;
    };
    let Some(tp) = chart.timing_points.iter_mut().find(|tp| tp.is_bpm()) else {
        return;
    };
    if let TimingPoint::Bpm { time_us, .. } = tp {
        if *time_us <= first_note_time {
            return;
        }
        tracing::warn!(
            bpm_time_us = *time_us,
            note_time_us = first_note_time,
            "BPM timing point is after first note — re-arranging to {}µs (re_arrange_bpm mode)",
            first_note_time - 1,
        );
        *time_us = first_note_time - 1;
    }
}

pub(crate) fn from_beatmap(beatmap: &super::types::OsuBeatmap) -> RoxChart {
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    // value is non-negative in valid input; key count fits in u8
    let key_count = beatmap.difficulty.circle_size as u8;
    let mut chart = RoxChart::new(key_count);
    chart.metadata = build_metadata(beatmap);
    build_timing_points(beatmap, &mut chart);
    build_notes(beatmap, key_count, &mut chart);
    chart
}

fn build_metadata(beatmap: &super::types::OsuBeatmap) -> Metadata {
    let title = beatmap
        .metadata
        .title_unicode
        .clone()
        .unwrap_or_else(|| beatmap.metadata.title.clone());
    let artist = beatmap
        .metadata
        .artist_unicode
        .clone()
        .unwrap_or_else(|| beatmap.metadata.artist.clone());
    Metadata {
        #[allow(clippy::cast_sign_loss)] // value is non-negative in valid input
        chart_id: beatmap.metadata.beatmap_id.map(|id| id as u64),
        #[allow(clippy::cast_sign_loss)] // value is non-negative in valid input
        chartset_id: beatmap.metadata.beatmap_set_id.map(|id| id as u64),
        title: title.into(),
        artist: artist.into(),
        creator: beatmap.metadata.creator.clone().into(),
        difficulty_name: beatmap.metadata.version.clone().into(),
        difficulty_value: Some(beatmap.difficulty.overall_difficulty),
        audio_file: beatmap.general.audio_filename.clone().into(),
        background_file: beatmap.background.clone().map(Into::into),
        audio_offset_us: i64::from(beatmap.general.audio_lead_in) * 1000,
        preview_time_us: i64::from(beatmap.general.preview_time).max(0) * 1000,
        source: beatmap.metadata.source.clone().map(Into::into),
        tags: beatmap.metadata.tags.iter().map(|s| s.clone().into()).collect(),
        ..Metadata::default()
    }
}

fn build_timing_points(beatmap: &super::types::OsuBeatmap, chart: &mut RoxChart) {
    for tp in &beatmap.timing_points {
        #[allow(clippy::cast_possible_truncation)]
        // ms→µs i64: safe for any realistic timestamp or duration
        let time_us = (tp.time * 1000.0) as i64;
        if tp.uninherited {
            if let Some(bpm) = tp.bpm() {
                chart.timing_points.push(TimingPoint::Bpm {
                    time_us,
                    bpm,
                    signature: tp.meter,
                });
            }
        } else {
            chart
                .timing_points
                .push(TimingPoint::sv(time_us, tp.scroll_velocity()));
        }
    }
}

fn build_notes(beatmap: &super::types::OsuBeatmap, key_count: u8, chart: &mut RoxChart) {
    let mut hitsound_map: BTreeMap<String, u16> = BTreeMap::new();
    for ho in &beatmap.hit_objects {
        let column = ho.column(key_count);
        let time_us = i64::from(ho.time) * 1000;
        let mut note = if ho.is_hold() {
            Note::hold(time_us, i64::from(ho.duration_ms()) * 1000, column)
        } else {
            Note::tap(time_us, column)
        };
        assign_hitsound(ho, &mut note, chart, &mut hitsound_map);
        chart.notes.push(note);
    }
    chart.notes.sort_by_key(|n| n.time_us);
}

fn assign_hitsound(
    ho: &super::types::OsuHitObject,
    note: &mut Note,
    chart: &mut RoxChart,
    map: &mut BTreeMap<String, u16>,
) {
    if ho.extras.is_empty() {
        return;
    }
    let parts: Vec<&str> = ho.extras.split(':').collect();
    let filename_idx = if ho.is_hold() { 5 } else { 4 };
    let Some(&filename) = parts.get(filename_idx) else {
        return;
    };
    let filename = filename.trim();
    if filename.is_empty() {
        return;
    }
    let idx = if let Some(&idx) = map.get(filename) {
        idx
    } else {
        let volume_idx = if ho.is_hold() { 4 } else { 3 };
        let volume: Option<u8> = parts
            .get(volume_idx)
            .and_then(|v| v.parse().ok())
            .filter(|&v: &u8| v > 0);
        let hs = match volume {
            Some(vol) => Hitsound::with_volume(filename, vol),
            None => Hitsound::new(filename),
        };
        #[allow(clippy::cast_possible_truncation)]
        // hitsound index fits in u16 for any realistic chart
        let idx = chart.hitsounds.len() as u16;
        chart.hitsounds.push(hs);
        map.insert(filename.to_string(), idx);
        idx
    };
    note.hitsound_index = Some(idx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rox::codec::Decoder;
    use rstest::rstest;

    #[rstest]
    fn test_decode_7k() {
        let data = rox_test_utils::get_test_asset("osu/mania_7k.osu");
        let chart = OsuDecoder::decode(&data).expect("decode failed");
        assert_eq!(chart.key_count, 7);
        assert!(!chart.notes.is_empty());
        assert!(!chart.timing_points.is_empty());
        assert_eq!(chart.metadata.difficulty_name, "7K Awakened");
    }

    #[rstest]
    fn test_decode_rejects_non_mania() {
        let data = b"osu file format v14\n\n[General]\nMode: 1\n\n[Difficulty]\nCircleSize:4\n";
        assert!(OsuDecoder::decode(data).is_err());
    }

    /// The header answer is the full parser's: the last `Mode:` of `[General]`, `0` without one, decided at the next section.
    #[rstest]
    #[case(b"osu file format v14\n\n[General]\nAudioFilename: a.mp3\nMode: 3\n\n[Editor]\n".as_slice(), Some(3))]
    #[case(b"\xEF\xBB\xBFosu file format v14\r\n\r\n[General]\r\nMode:0\r\n[Metadata]\r\n".as_slice(), Some(0))]
    #[case(b"[General]\nMode: 1\nMode: 3\n[Difficulty]\n".as_slice(), Some(3))]
    #[case(b"[General]\nAudioFilename: a.mp3\n[Metadata]\n".as_slice(), Some(0))]
    #[case(b"[Metadata]\nTitle:Mode: 3\n[General]\nMode: 2\n[Events]\n".as_slice(), Some(2))]
    #[case(b"[General]\n// Mode: 2\nMode: x\n[Events]\n".as_slice(), Some(0))]
    #[case(b"[General]\nMode: 3\n".as_slice(), None)]
    #[case(b"[General]\nMode: 3".as_slice(), None)]
    fn peek_mode_reads_the_general_section_only(#[case] head: &[u8], #[case] expected: Option<u8>) {
        // A cut head decides only once the section after `[General]` began; a whole file always decides.
        assert_eq!(OsuDecoder::peek_mode(head, false), expected);
        assert!(
            OsuDecoder::peek_mode(head, true).is_some(),
            "a whole file decides"
        );
        if let Some(mode) = expected {
            assert_eq!(
                parser::parse(head).unwrap().general.mode,
                mode,
                "the parser reads the same mode"
            );
        }
    }

    #[rstest]
    fn peek_mode_never_decides_inside_a_cut_line_or_on_bad_text() {
        assert_eq!(
            OsuDecoder::peek_mode(b"[General]\nMode: 3\n[Metadata]\n[Meta", false),
            Some(3)
        );
        assert_eq!(
            OsuDecoder::peek_mode(b"[General]\nMode: 3\n[Meta", false),
            None
        );
        assert_eq!(OsuDecoder::peek_mode(b"[General]\nMod", false), None);
        assert_eq!(
            OsuDecoder::peek_mode(b"[General]\nMode: \xFF\n[Events]\n", true),
            None
        );
    }

    /// A file of another mode is refused whatever follows its header: even garbage that the full parse would report.
    #[rstest]
    fn another_mode_is_refused_before_the_rest_is_read() {
        let mut data = b"osu file format v14\n\n[General]\nMode: 0\n\n[Events]\n".to_vec();
        data.extend_from_slice(&[0xFF; 64]);
        let error = OsuDecoder::decode_with_options(&data, &OsuDecodeOptions::default())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("Not a mania beatmap (mode=0, expected 3)"),
            "{error}"
        );
    }

    #[rstest]
    fn test_decode_hitsounds() {
        let data = rox_test_utils::get_test_asset("osu/mania_hitsound.osu");
        let chart = OsuDecoder::decode(&data).expect("decode failed");
        assert_eq!(chart.key_count, 4);
        assert_eq!(chart.hitsounds.len(), 4);
    }

    #[rstest]
    fn test_decode_50k_fails_without_re_arrange() {
        let data = rox_test_utils::get_test_asset("osu/mania_4K_50K_notes.osu");
        assert!(OsuDecoder::decode(&data).is_err());
    }

    #[rstest]
    fn test_decode_50k_succeeds_with_re_arrange() {
        let data = rox_test_utils::get_test_asset("osu/mania_4K_50K_notes.osu");
        let opts = OsuDecodeOptions {
            re_arrange_bpm: true,
        };
        let chart = OsuDecoder::decode_with_options(&data, &opts).expect("decode failed");
        assert_eq!(chart.key_count, 4);
        assert!(!chart.notes.is_empty());
        // first BPM must be <= first note
        let first_note = chart.notes.iter().map(|n| n.time_us).min().unwrap();
        let first_bpm = chart
            .timing_points
            .iter()
            .find(|tp| tp.is_bpm())
            .map(TimingPoint::time_us)
            .unwrap();
        assert!(first_bpm <= first_note);
    }
}
