//! Chart metadata (title, artist, etc.)

use compact_str::CompactString;
use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

/// Metadata describing the chart and associated media.
#[derive(
    Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize,
)]
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
    pub title: CompactString,
    /// Song artist.
    pub artist: CompactString,
    /// Chart creator/mapper.
    pub creator: CompactString,
    /// Difficulty name (e.g., "Hard", "Expert").
    pub difficulty_name: CompactString,
    /// Optional numeric difficulty value (format-dependent).
    pub difficulty_value: Option<f32>,

    // Media files
    /// Relative path to the audio file.
    pub audio_file: CompactString,
    /// Optional relative path to the background image.
    pub background_file: Option<CompactString>,

    // Audio timing
    /// Global audio offset in microseconds.
    pub audio_offset_us: i64,
    /// Preview start time in microseconds.
    pub preview_time_us: i64,
    /// Preview duration in microseconds.
    pub preview_duration_us: i64,

    // Additional info
    /// Source (anime, game, original, etc.)
    pub source: Option<CompactString>,
    /// Genre (electronic, rock, etc.)
    pub genre: Option<CompactString>,
    /// Language code (JP, EN, KR, etc.)
    pub language: Option<CompactString>,
    /// Tags for search/categorization.
    pub tags: Vec<CompactString>,

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
            title: CompactString::new(""),
            artist: CompactString::new(""),
            creator: CompactString::new(""),
            difficulty_name: CompactString::from("Normal"),
            difficulty_value: None,
            audio_file: CompactString::new(""),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_default() {
        let meta = Metadata::default();

        assert!(meta.title.is_empty());
        assert!(meta.artist.is_empty());
        assert!(meta.creator.is_empty());
        assert_eq!(meta.difficulty_name, "Normal");
        assert!(meta.difficulty_value.is_none());
        assert!(meta.audio_file.is_empty());
        assert!(meta.background_file.is_none());
        assert_eq!(meta.audio_offset_us, 0);
        assert_eq!(meta.preview_time_us, 0);
        assert_eq!(meta.preview_duration_us, 15_000_000); // 15 seconds
        assert!(meta.source.is_none());
        assert!(meta.genre.is_none());
        assert!(meta.language.is_none());
        assert!(meta.tags.is_empty());
    }
}
