//! Model tests for ROX data structures.

use rhythm_open_exchange::{Hitsound, Metadata, Note, NoteType, RoxChart, TimingPoint};

// =============================================================================
// Note Tests
// =============================================================================

#[test]
fn test_note_tap_constructor() {
    let note = Note::tap(1_000_000, 2);

    assert_eq!(note.time_us, 1_000_000);
    assert_eq!(note.column, 2);
    assert!(matches!(note.note_type, NoteType::Tap));
    assert!(note.hitsound_index.is_none());
}

#[test]
fn test_note_hold_constructor() {
    let note = Note::hold(2_000_000, 500_000, 1);

    assert_eq!(note.time_us, 2_000_000);
    assert_eq!(note.column, 1);
    assert!(matches!(
        note.note_type,
        NoteType::Hold {
            duration_us: 500_000
        }
    ));
}

#[test]
fn test_note_burst_constructor() {
    let note = Note::burst(3_000_000, 300_000, 3);

    assert_eq!(note.time_us, 3_000_000);
    assert_eq!(note.column, 3);
    assert!(matches!(
        note.note_type,
        NoteType::Burst {
            duration_us: 300_000
        }
    ));
}

#[test]
fn test_note_mine_constructor() {
    let note = Note::mine(4_000_000, 0);

    assert_eq!(note.time_us, 4_000_000);
    assert_eq!(note.column, 0);
    assert!(matches!(note.note_type, NoteType::Mine));
}

#[test]
fn test_note_is_hold() {
    assert!(!Note::tap(0, 0).is_hold());
    assert!(Note::hold(0, 100, 0).is_hold());
    assert!(!Note::burst(0, 100, 0).is_hold());
    assert!(!Note::mine(0, 0).is_hold());
}

#[test]
fn test_note_is_burst() {
    assert!(!Note::tap(0, 0).is_burst());
    assert!(!Note::hold(0, 100, 0).is_burst());
    assert!(Note::burst(0, 100, 0).is_burst());
    assert!(!Note::mine(0, 0).is_burst());
}

#[test]
fn test_note_is_mine() {
    assert!(!Note::tap(0, 0).is_mine());
    assert!(!Note::hold(0, 100, 0).is_mine());
    assert!(!Note::burst(0, 100, 0).is_mine());
    assert!(Note::mine(0, 0).is_mine());
}

#[test]
fn test_note_duration_us() {
    assert_eq!(Note::tap(0, 0).duration_us(), 0);
    assert_eq!(Note::hold(0, 500_000, 0).duration_us(), 500_000);
    assert_eq!(Note::burst(0, 300_000, 0).duration_us(), 300_000);
    assert_eq!(Note::mine(0, 0).duration_us(), 0);
}

#[test]
fn test_note_end_time_us() {
    assert_eq!(Note::tap(1_000_000, 0).end_time_us(), 1_000_000);
    assert_eq!(Note::hold(1_000_000, 500_000, 0).end_time_us(), 1_500_000);
    assert_eq!(Note::burst(2_000_000, 300_000, 0).end_time_us(), 2_300_000);
    assert_eq!(Note::mine(3_000_000, 0).end_time_us(), 3_000_000);
}

// =============================================================================
// TimingPoint Tests
// =============================================================================

#[test]
fn test_timing_point_bpm() {
    let tp = TimingPoint::bpm(0, 180.0);

    assert_eq!(tp.time_us, 0);
    assert_eq!(tp.bpm, 180.0);
    assert_eq!(tp.signature, 4);
    assert!(!tp.is_inherited);
    assert_eq!(tp.scroll_speed, 1.0);
}

#[test]
fn test_timing_point_sv() {
    let tp = TimingPoint::sv(1_000_000, 1.5);

    assert_eq!(tp.time_us, 1_000_000);
    assert_eq!(tp.bpm, 0.0);
    assert_eq!(tp.signature, 4);
    assert!(tp.is_inherited);
    assert_eq!(tp.scroll_speed, 1.5);
}

// =============================================================================
// Hitsound Tests
// =============================================================================

#[test]
fn test_hitsound_new() {
    let hs = Hitsound::new("kick.wav");

    assert_eq!(hs.file, "kick.wav");
    assert!(hs.volume.is_none());
}

#[test]
fn test_hitsound_with_volume() {
    let hs = Hitsound::with_volume("snare.ogg", 75);

    assert_eq!(hs.file, "snare.ogg");
    assert_eq!(hs.volume, Some(75));
}

#[test]
fn test_hitsound_volume_clamped_to_100() {
    let hs = Hitsound::with_volume("loud.wav", 150);

    assert_eq!(hs.volume, Some(100));
}

// =============================================================================
// Metadata Tests
// =============================================================================

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

// =============================================================================
// RoxChart Tests
// =============================================================================

#[test]
fn test_rox_chart_new() {
    let chart = RoxChart::new(4);

    assert_eq!(chart.version, 2);
    assert_eq!(chart.key_count(), 4);
    assert!(chart.timing_points.is_empty());
    assert!(chart.notes.is_empty());
    assert!(chart.hitsounds.is_empty());
}

#[test]
fn test_rox_chart_new_7k() {
    let chart = RoxChart::new(7);
    assert_eq!(chart.key_count(), 7);
}

#[test]
fn test_rox_chart_duration_empty() {
    let chart = RoxChart::new(4);
    assert_eq!(chart.duration_us(), 0);
}

#[test]
fn test_rox_chart_duration_with_notes() {
    let mut chart = RoxChart::new(4);
    chart.notes.push(Note::tap(1_000_000, 0));
    chart.notes.push(Note::tap(2_000_000, 1));
    chart.notes.push(Note::hold(3_000_000, 500_000, 2)); // ends at 3.5s

    assert_eq!(chart.duration_us(), 3_500_000);
}

#[test]
fn test_rox_chart_note_count() {
    let mut chart = RoxChart::new(4);
    assert_eq!(chart.note_count(), 0);

    chart.notes.push(Note::tap(0, 0));
    chart.notes.push(Note::hold(1_000_000, 500_000, 1));
    chart.notes.push(Note::mine(2_000_000, 2));

    assert_eq!(chart.note_count(), 3);
}

#[test]
fn test_rox_chart_validate_valid() {
    let mut chart = RoxChart::new(4);
    chart.notes.push(Note::tap(0, 0));
    chart.notes.push(Note::tap(0, 1));
    chart.notes.push(Note::tap(0, 2));
    chart.notes.push(Note::tap(0, 3));

    chart.timing_points.push(TimingPoint::bpm(0, 120.0));

    assert!(chart.validate().is_ok());
}

#[test]
fn test_rox_chart_validate_invalid_column() {
    let mut chart = RoxChart::new(4);
    chart.notes.push(Note::tap(0, 4)); // Invalid: column 4 doesn't exist in 4K

    assert!(chart.validate().is_err());
}

#[test]
fn test_rox_chart_hash_consistency() {
    let mut chart = RoxChart::new(4);
    chart.notes.push(Note::tap(0, 0));

    let hash1 = chart.hash();
    let hash2 = chart.hash();

    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 64); // BLAKE3 produces 32 bytes = 64 hex chars
}

#[test]
fn test_rox_chart_short_hash() {
    let chart = RoxChart::new(4);
    let short = chart.short_hash();

    assert_eq!(short.len(), 16);
    assert!(chart.hash().starts_with(&short));
}

#[test]
fn test_rox_chart_hash_changes_with_content() {
    let mut chart1 = RoxChart::new(4);
    let mut chart2 = RoxChart::new(4);

    chart1.notes.push(Note::tap(0, 0));
    chart2.notes.push(Note::tap(0, 1));

    assert_ne!(chart1.hash(), chart2.hash());
}
