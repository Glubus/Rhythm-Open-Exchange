#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

/// Note types in `StepMania` format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmNoteType {
    Empty, Tap, HoldHead, Tail, RollHead, Mine, Lift, Fake,
}

impl SmNoteType {
    #[must_use]
    pub fn from_char(c: char) -> Self {
        match c {
            '1' => Self::Tap, '2' => Self::HoldHead, '3' => Self::Tail,
            '4' => Self::RollHead, 'M' | 'm' => Self::Mine,
            'L' | 'l' => Self::Lift, 'F' | 'f' => Self::Fake, _ => Self::Empty,
        }
    }

    #[must_use]
    pub fn is_note(self) -> bool { !matches!(self, Self::Empty | Self::Fake) }
}

#[derive(Debug, Clone, Default)]
pub struct SmFile {
    pub metadata: SmMetadata,
    pub offset_us: i64,
    /// BPM changes: (`time_us`, bpm).
    pub bpms: Vec<(i64, f32)>,
    pub charts: Vec<SmChart>,
}

#[derive(Debug, Clone, Default)]
pub struct SmMetadata {
    pub title: String, pub artist: String, pub credit: String,
    pub music: String, pub banner: String, pub background: String,
    pub sample_start: f64, pub sample_length: f64,
}

#[derive(Debug, Clone)]
pub struct SmChart {
    pub stepstype: String, pub difficulty: String,
    pub meter: u32, pub column_count: u8,
    pub notes: Vec<SmNote>,
}

impl Default for SmChart {
    fn default() -> Self {
        Self { stepstype: String::new(), difficulty: String::new(),
               meter: 0, column_count: 4, notes: Vec::new() }
    }
}

impl SmChart {
    #[must_use]
    pub fn column_count_from_stepstype(stepstype: &str) -> u8 {
        match stepstype.trim().to_ascii_lowercase().as_str() {
            "dance-double" | "pump-double" | "dance-couple" => 8,
            "dance-solo" | "pump-halfdouble" => 6,
            _ => 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SmNote { pub time_us: i64, pub column: u8, pub note_type: SmNoteType }

pub mod timing {
    pub const ROWS_PER_BEAT: f64 = 48.0;
    pub const ROWS_PER_MEASURE: f64 = 192.0;

    #[must_use]
    pub fn rows_to_us(rows: f64, bpm: f32) -> i64 {
        let beats = rows / ROWS_PER_BEAT;
        let seconds = beats / (f64::from(bpm) / 60.0);
        #[allow(clippy::cast_possible_truncation)]
        let result = (seconds * 1_000_000.0) as i64;
        result
    }

    #[must_use]
    pub fn us_to_rows(us: i64, bpm: f32) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let seconds = us as f64 / 1_000_000.0;
        seconds * (f64::from(bpm) / 60.0) * ROWS_PER_BEAT
    }
}
