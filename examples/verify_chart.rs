//! Verify the large_chart.rox file is complete.

use rhythm_open_exchange::{Decoder, RoxCodec};

fn main() {
    println!("Loading large_chart.rox...");

    let chart = RoxCodec::decode_from_path("large_chart.rox").expect("Failed to load chart");

    println!("\nChart Contents:");
    println!("  Version: {}", chart.version);
    println!("  Key count: {}K", chart.key_count);
    println!("  Notes: {}", chart.notes.len());
    println!("  Timing points: {}", chart.timing_points.len());
    println!("  Hitsounds: {}", chart.hitsounds.len());

    println!("\nMetadata:");
    println!("  Title: {}", chart.metadata.title);
    println!("  Artist: {}", chart.metadata.artist);
    println!("  Creator: {}", chart.metadata.creator);
    println!("  Difficulty: {}", chart.metadata.difficulty_name);

    println!("\nFirst 5 notes:");
    for (i, note) in chart.notes.iter().take(5).enumerate() {
        println!(
            "  [{}] time={}us col={} type={:?}",
            i, note.time_us, note.column, note.note_type
        );
    }

    println!("\nLast 5 notes:");
    let len = chart.notes.len();
    for (i, note) in chart.notes.iter().skip(len.saturating_sub(5)).enumerate() {
        println!(
            "  [{}] time={}us col={} type={:?}",
            len - 5 + i,
            note.time_us,
            note.column,
            note.note_type
        );
    }

    println!("\nFirst 3 timing points:");
    for (i, tp) in chart.timing_points.iter().take(3).enumerate() {
        println!(
            "  [{}] time={}us bpm={} sv={} inherited={}",
            i, tp.time_us, tp.bpm, tp.scroll_speed, tp.is_inherited
        );
    }

    // Verify all notes have reasonable timestamps
    let mut prev_time = i64::MIN;
    let mut out_of_order = 0;
    for note in &chart.notes {
        if note.time_us < prev_time {
            out_of_order += 1;
        }
        prev_time = note.time_us;
    }

    if out_of_order > 0 {
        println!("\n⚠️  WARNING: {} notes out of order!", out_of_order);
    } else {
        println!("\n✓ All notes in order");
    }

    println!("\nHash: {}", chart.short_hash());
    println!("\n✓ Verification complete!");
}
