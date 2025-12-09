#![allow(clippy::doc_markdown, clippy::match_same_arms)]
//! Type definitions for StepMania (`.sm`) file format.

/// Note types in StepMania format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmNoteType {
    /// Empty - no note (0)
    Empty,
    /// Tap note (1)
    Tap,
    /// Hold head - start of a hold note (2)
    HoldHead,
    /// Tail - end of hold or roll (3)
    Tail,
    /// Roll head - start of a roll/burst note (4)
    RollHead,
    /// Mine - avoid this note (M)
    Mine,
    /// Lift - release note (L)
    Lift,
    /// Fake - decorative, doesn't count (F)
    Fake,
}

impl SmNoteType {
    /// Parse a character into a note type.
    #[must_use]
    pub fn from_char(c: char) -> Self {
        match c {
            '0' => Self::Empty,
            '1' => Self::Tap,
            '2' => Self::HoldHead,
            '3' => Self::Tail,
            '4' => Self::RollHead,
            'M' | 'm' => Self::Mine,
            'L' | 'l' => Self::Lift,
            'F' | 'f' => Self::Fake,
            _ => Self::Empty,
        }
    }

    /// Convert to character for encoding.
    #[must_use]
    pub fn to_char(self) -> char {
        match self {
            Self::Empty => '0',
            Self::Tap => '1',
            Self::HoldHead => '2',
            Self::Tail => '3',
            Self::RollHead => '4',
            Self::Mine => 'M',
            Self::Lift => 'L',
            Self::Fake => 'F',
        }
    }

    /// Check if this is an actionable note (not empty or fake).
    #[must_use]
    pub fn is_note(self) -> bool {
        !matches!(self, Self::Empty | Self::Fake)
    }
}

/// A parsed StepMania file.
#[derive(Debug, Clone, Default)]
pub struct SmFile {
    /// Song metadata.
    pub metadata: SmMetadata,
    /// Global offset in microseconds (positive = notes appear later).
    pub offset_us: i64,
    /// BPM changes: (time_us, bpm).
    pub bpms: Vec<(i64, f32)>,
    /// Stops/freezes: (time_us, duration_us).
    pub stops: Vec<(i64, i64)>,
    /// All charts in the file.
    pub charts: Vec<SmChart>,
}

/// Song metadata from SM file.
#[derive(Debug, Clone, Default)]
pub struct SmMetadata {
    pub title: String,
    pub subtitle: String,
    pub artist: String,
    pub title_translit: String,
    pub artist_translit: String,
    pub credit: String,
    pub music: String,
    pub banner: String,
    pub background: String,
    pub sample_start: f64,
    pub sample_length: f64,
}

/// A single chart/difficulty in an SM file.
#[derive(Debug, Clone)]
pub struct SmChart {
    /// Steps type: "dance-single", "dance-double", etc.
    pub stepstype: String,
    /// Description (usually empty or author name).
    pub description: String,
    /// Difficulty name: "Beginner", "Easy", "Medium", "Hard", "Challenge", "Edit".
    pub difficulty: String,
    /// Numeric difficulty rating (meter).
    pub meter: u32,
    /// Radar values (stream, voltage, air, freeze, chaos).
    pub radar_values: Vec<f64>,
    /// Number of columns (4 for dance-single, 8 for dance-double).
    pub column_count: u8,
    /// Parsed notes with timing.
    pub notes: Vec<SmNote>,
}

impl Default for SmChart {
    fn default() -> Self {
        Self {
            stepstype: String::new(),
            description: String::new(),
            difficulty: String::new(),
            meter: 0,
            radar_values: Vec::new(),
            column_count: 4,
            notes: Vec::new(),
        }
    }
}

impl SmChart {
    /// Determine column count from stepstype.
    #[must_use]
    #[allow(clippy::match_same_arms)]
    pub fn column_count_from_stepstype(stepstype: &str) -> u8 {
        match stepstype.trim().to_lowercase().as_str() {
            "dance-single" | "pump-single" => 4,
            "dance-double" | "pump-double" => 8,
            "dance-couple" => 8,
            "dance-solo" | "pump-halfdouble" => 6,
            // For unknown types, try to infer from note data later
            _ => 4,
        }
    }
}

/// A single note with timing information.
#[derive(Debug, Clone)]
pub struct SmNote {
    /// Time in microseconds.
    pub time_us: i64,
    /// Column index (0-indexed).
    pub column: u8,
    /// Note type.
    pub note_type: SmNoteType,
}

/// Timing constants for StepMania's row-based system.
#[allow(clippy::cast_precision_loss)]
pub mod timing {
    /// Rows per beat (48 is standard).
    pub const ROWS_PER_BEAT: f64 = 48.0;
    /// Rows per measure (4 beats * 48 rows = 192).
    pub const ROWS_PER_MEASURE: f64 = 192.0;

    /// Convert rows to microseconds at a given BPM.
    #[must_use]
    pub fn rows_to_us(rows: f64, bpm: f32) -> i64 {
        let beats = rows / ROWS_PER_BEAT;
        let seconds = beats / (f64::from(bpm) / 60.0);
        #[allow(clippy::cast_possible_truncation)]
        let result = (seconds * 1_000_000.0) as i64;
        result
    }

    /// Convert microseconds to rows at a given BPM.
    #[must_use]
    pub fn us_to_rows(us: i64, bpm: f32) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let seconds = us as f64 / 1_000_000.0;
        let beats = seconds * (f64::from(bpm) / 60.0);
        beats * ROWS_PER_BEAT
    }
}
