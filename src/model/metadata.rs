//! Chart metadata (title, artist, etc.)

use bincode::{Decode, Encode};

/// Metadata describing the chart and associated media.
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Metadata {
    // Identifiers
    /// Optional chart ID (for online databases).
    pub chart_id: Option<u64>,
    /// Optional chartset ID (for online databases).
    pub chartset_id: Option<u64>,

    // Key configuration
    /// Number of columns/keys (e.g., 4 for 4K, 7 for 7K).
    pub key_count: u8,

    /// Song title.
    pub title: String,
    /// Song artist.
    pub artist: String,
    /// Chart creator/mapper.
    pub creator: String,
    /// Difficulty name (e.g., "Hard", "Expert").
    pub difficulty_name: String,
    /// Optional numeric difficulty value (format-dependent).
    pub difficulty_value: Option<f32>,

    // Media files
    /// Relative path to the audio file.
    pub audio_file: String,
    /// Optional relative path to the background image.
    pub background_file: Option<String>,

    // Audio timing
    /// Global audio offset in microseconds.
    pub audio_offset_us: i64,
    /// Preview start time in microseconds.
    pub preview_time_us: i64,
    /// Preview duration in microseconds.
    pub preview_duration_us: i64,

    // Additional info
    /// Source (anime, game, original, etc.)
    pub source: Option<String>,
    /// Genre (electronic, rock, etc.)
    pub genre: Option<String>,
    /// Language code (JP, EN, KR, etc.)
    pub language: Option<String>,
    /// Tags for search/categorization.
    pub tags: Vec<String>,

    // Coop/multiplayer info
    /// Whether this chart is designed for 2-player coop mode.
    /// When true, columns are split evenly: P1 = `0..key_count/2`, P2 = `key_count/2..key_count`.
    /// Examples: 8K with `is_coop=true` → 4K+4K, 16K with `is_coop=true` → 8K+8K.
    pub is_coop: bool,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            chart_id: None,
            chartset_id: None,
            key_count: 4,
            title: String::new(),
            artist: String::new(),
            creator: String::new(),
            difficulty_name: String::from("Normal"),
            difficulty_value: None,
            audio_file: String::new(),
            background_file: None,
            audio_offset_us: 0,
            preview_time_us: 0,
            preview_duration_us: 15_000_000, // 15 seconds default
            source: None,
            genre: None,
            language: None,
            tags: Vec::new(),
            is_coop: false,
        }
    }
}
