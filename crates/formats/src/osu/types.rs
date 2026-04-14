#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// Parsed osu! beatmap.
#[derive(Debug, Clone, Default)]
pub struct OsuBeatmap {
    pub format_version: u8,
    pub general: OsuGeneral,
    pub metadata: OsuMetadata,
    pub difficulty: OsuDifficulty,
    pub background: Option<String>,
    pub timing_points: Vec<OsuTimingPoint>,
    pub hit_objects: Vec<OsuHitObject>,
}

/// `[General]` section.
#[derive(Debug, Clone, Default)]
pub struct OsuGeneral {
    pub audio_filename: String,
    pub audio_lead_in: i32,
    pub preview_time: i32,
    pub mode: u8,
}

/// `[Metadata]` section.
#[derive(Debug, Clone, Default)]
pub struct OsuMetadata {
    pub title: String,
    pub title_unicode: Option<String>,
    pub artist: String,
    pub artist_unicode: Option<String>,
    pub creator: String,
    pub version: String,
    pub source: Option<String>,
    pub tags: Vec<String>,
    pub beatmap_id: Option<i32>,
    pub beatmap_set_id: Option<i32>,
}

/// `[Difficulty]` section.
#[derive(Debug, Clone, Default)]
pub struct OsuDifficulty {
    pub circle_size: f32,
    pub overall_difficulty: f32,
    pub hp_drain_rate: f32,
}

/// A timing point (BPM or SV change).
#[derive(Debug, Clone)]
pub struct OsuTimingPoint {
    pub time: f64,
    pub beat_length: f64,
    pub meter: u8,
    pub sample_set: u8,
    pub sample_index: u8,
    pub volume: u8,
    pub uninherited: bool,
    pub effects: u8,
}

impl OsuTimingPoint {
    /// BPM value if this is an uninherited (BPM) point.
    #[must_use]
    pub fn bpm(&self) -> Option<f32> {
        if self.uninherited && self.beat_length > 0.0 {
            #[allow(clippy::cast_possible_truncation)] // ms/beats→f32: safe for any realistic BPM value
            Some((60_000.0 / self.beat_length) as f32)
        } else {
            None
        }
    }

    /// Scroll velocity multiplier (1.0 for uninherited points).
    #[must_use]
    pub fn scroll_velocity(&self) -> f32 {
        if self.uninherited {
            1.0
        } else {
            #[allow(clippy::cast_possible_truncation)] // ms/beats→f32: safe for any realistic SV value
            let sv = (-100.0 / self.beat_length) as f32;
            sv
        }
    }
}

/// A hit object (note or hold).
#[derive(Debug, Clone)]
pub struct OsuHitObject {
    pub x: i32,
    pub y: i32,
    pub time: i32,
    /// Bit 0: Circle (tap), Bit 7: Hold note.
    pub object_type: u8,
    pub hit_sound: u8,
    pub end_time: Option<i32>,
    pub extras: compact_str::CompactString,
}

impl OsuHitObject {
    #[must_use]
    pub fn is_hold(&self) -> bool { (self.object_type & 128) != 0 }

    #[must_use]
    pub fn column(&self, key_count: u8) -> u8 {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // value is non-negative in valid input; column fits in u8
        {
            let col = (self.x * i32::from(key_count)) / 512;
            col as u8
        }
    }

    #[must_use]
    pub fn duration_ms(&self) -> i32 {
        self.end_time.map_or(0, |e| e - self.time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(36, 7, 0)]
    #[case(109, 7, 1)]
    #[case(256, 7, 3)]
    #[case(475, 7, 6)]
    #[case(64, 4, 0)]
    #[case(192, 4, 1)]
    fn test_column_from_x(#[case] x: i32, #[case] key_count: u8, #[case] expected_col: u8) {
        let ho = OsuHitObject { x, y: 192, time: 0, object_type: 1, hit_sound: 0,
            end_time: None, extras: compact_str::CompactString::new("") };
        assert_eq!(ho.column(key_count), expected_col);
    }

    #[rstest]
    fn test_timing_point_bpm() {
        let tp = OsuTimingPoint { time: 0.0, beat_length: 322.58, meter: 4, sample_set: 0,
            sample_index: 0, volume: 100, uninherited: true, effects: 0 };
        assert!((tp.bpm().unwrap() - 186.0).abs() < 1.0);
    }

    #[rstest]
    fn test_timing_point_sv() {
        let tp = OsuTimingPoint { time: 0.0, beat_length: -133.33, meter: 4, sample_set: 0,
            sample_index: 0, volume: 100, uninherited: false, effects: 0 };
        assert!((tp.scroll_velocity() - 0.75).abs() < 0.01);
    }
}
