#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use compact_str::CompactString;
use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

/// Chart metadata. Does NOT include `key_count` — that lives on `RoxChart`.
#[derive(Debug, Clone, PartialEq)]
#[derive(Archive, Serialize, Deserialize)]
#[derive(SerdeSerialize, SerdeDeserialize)]
pub struct Metadata {
    pub chart_id: Option<u64>,
    pub chartset_id: Option<u64>,
    pub title: CompactString,
    pub artist: CompactString,
    pub creator: CompactString,
    pub difficulty_name: CompactString,
    pub difficulty_value: Option<f32>,
    pub audio_file: CompactString,
    pub background_file: Option<CompactString>,
    pub audio_offset_us: i64,
    pub preview_time_us: i64,
    pub preview_duration_us: i64,
    pub source: Option<CompactString>,
    pub genre: Option<CompactString>,
    pub language: Option<CompactString>,
    pub tags: Vec<CompactString>,
    pub is_coop: bool,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            chart_id: None,
            chartset_id: None,
            title: CompactString::new(""),
            artist: CompactString::new(""),
            creator: CompactString::new(""),
            difficulty_name: CompactString::from("Normal"),
            difficulty_value: None,
            audio_file: CompactString::new(""),
            background_file: None,
            audio_offset_us: 0,
            preview_time_us: 0,
            preview_duration_us: 15_000_000,
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
    fn test_metadata_default_values() {
        let m = Metadata::default();
        assert!(m.title.is_empty());
        assert!(m.artist.is_empty());
        assert_eq!(m.difficulty_name, "Normal");
        assert_eq!(m.preview_duration_us, 15_000_000);
        assert!(!m.is_coop);
        assert!(m.tags.is_empty());
        assert!(m.chart_id.is_none());
    }
}
