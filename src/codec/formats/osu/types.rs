//! Type definitions for osu! beatmap format.

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
    /// For mania, this is the key count (4, 5, 6, 7, 8, etc.)
    pub circle_size: f32,
    pub overall_difficulty: f32,
    pub hp_drain_rate: f32,
}

/// A timing point (BPM or SV change).
#[derive(Debug, Clone)]
pub struct OsuTimingPoint {
    /// Time in milliseconds.
    pub time: f64,
    /// Beat length in ms (if uninherited) or SV multiplier encoded (if inherited).
    /// For uninherited: BPM = 60000 / `beat_length`
    /// For inherited: SV = -100 / `beat_length`
    pub beat_length: f64,
    /// Time signature (beats per measure).
    pub meter: u8,
    /// Sample set (0=default, 1=normal, 2=soft, 3=drum).
    pub sample_set: u8,
    /// Sample index.
    pub sample_index: u8,
    /// Volume (0-100).
    pub volume: u8,
    /// True = BPM point, False = SV/inherited point.
    pub uninherited: bool,
    /// Effects (kiai, etc.)
    pub effects: u8,
}

impl OsuTimingPoint {
    /// Get BPM if this is an uninherited point.
    #[must_use]
    pub fn bpm(&self) -> Option<f32> {
        if self.uninherited && self.beat_length > 0.0 {
            #[allow(clippy::cast_possible_truncation)]
            Some((60000.0 / self.beat_length) as f32)
        } else {
            None
        }
    }

    /// Get scroll velocity multiplier if this is an inherited point.
    /// Returns 1.0 for uninherited points.
    #[must_use]
    pub fn scroll_velocity(&self) -> f32 {
        if self.uninherited {
            1.0
        } else {
            #[allow(clippy::cast_possible_truncation)]
            let sv = (-100.0 / self.beat_length) as f32;
            sv
        }
    }
}

/// A hit object (note).
#[derive(Debug, Clone)]
pub struct OsuHitObject {
    /// X position (0-512), used to calculate column.
    pub x: i32,
    /// Y position (always 192 for mania).
    pub y: i32,
    /// Time in milliseconds.
    pub time: i32,
    /// Object type bitfield.
    /// Bit 0: Circle (tap)
    /// Bit 1: Slider (not used in mania)
    /// Bit 7: Hold note
    pub object_type: u8,
    /// Hit sound.
    pub hit_sound: u8,
    /// End time for hold notes (in ms).
    pub end_time: Option<i32>,
    /// Additional parameters.
    pub extras: compact_str::CompactString,
}

impl OsuHitObject {
    /// Check if this is a hold note.
    #[must_use]
    pub fn is_hold(&self) -> bool {
        (self.object_type & 128) != 0
    }

    /// Check if this is a tap note.
    #[must_use]
    pub fn is_tap(&self) -> bool {
        (self.object_type & 1) != 0 && !self.is_hold()
    }

    /// Calculate column index from X position.
    #[must_use]
    pub fn column(&self, key_count: u8) -> u8 {
        let column = (self.x * i32::from(key_count)) / 512;
        // Safe: column is always 0..key_count which fits in u8
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let result = column as u8;
        result
    }

    /// Get duration in milliseconds for hold notes.
    #[must_use]
    pub fn duration_ms(&self) -> i32 {
        self.end_time.map_or(0, |e| e - self.time)
    }
}
