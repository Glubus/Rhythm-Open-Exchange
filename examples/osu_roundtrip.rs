//! Example: Test osu!mania roundtrip conversion.
//!
//! Decodes .osu files from assets, converts to ROX, then encodes back to .osu.
//! This validates the entire parsing and encoding pipeline.
//!
//! Run with: `cargo run --example osu_roundtrip`

use rhythm_open_exchange::RoxCodec;
use rhythm_open_exchange::codec::formats::osu::{OsuDecoder, OsuEncoder};
use rhythm_open_exchange::codec::{Decoder, Encoder};
use std::fs;

fn test_map(name: &str, path: &str) {
    println!("\n============================================================");
    println!("Testing: {}", name);
    println!("============================================================");

    // Read original .osu
    let original = fs::read(path).expect("Failed to read .osu file");
    println!("Original .osu size: {} bytes", original.len());

    // Decode to RoxChart
    let chart = OsuDecoder::decode(&original).expect("Failed to decode .osu");
    println!("\nDecoded RoxChart:");
    println!("  Key count: {}K", chart.key_count);
    println!("  Notes: {}", chart.notes.len());
    println!("  Timing points: {}", chart.timing_points.len());
    println!("  Title: {}", chart.metadata.title);
    println!("  Artist: {}", chart.metadata.artist);
    println!("  Difficulty: {}", chart.metadata.difficulty_name);

    // Count note types
    let taps = chart
        .notes
        .iter()
        .filter(|n| matches!(n.note_type, rhythm_open_exchange::NoteType::Tap))
        .count();
    let holds = chart
        .notes
        .iter()
        .filter(|n| matches!(n.note_type, rhythm_open_exchange::NoteType::Hold { .. }))
        .count();
    println!("  Taps: {}, Holds: {}", taps, holds);

    // Count timing point types
    let bpm_points = chart
        .timing_points
        .iter()
        .filter(|tp| !tp.is_inherited)
        .count();
    let sv_points = chart
        .timing_points
        .iter()
        .filter(|tp| tp.is_inherited)
        .count();
    println!("  BPM points: {}, SV points: {}", bpm_points, sv_points);

    // Encode to ROX format
    let rox_encoded = RoxCodec::encode(&chart).expect("Failed to encode to ROX");
    println!("\nROX format:");
    println!(
        "  Size: {} bytes ({:.2} KB)",
        rox_encoded.len(),
        rox_encoded.len() as f64 / 1024.0
    );

    // Encode back to .osu
    let osu_encoded = OsuEncoder::encode(&chart).expect("Failed to encode to .osu");
    println!("\nRe-encoded .osu size: {} bytes", osu_encoded.len());

    // Decode the re-encoded .osu to verify
    let chart2 = OsuDecoder::decode(&osu_encoded).expect("Failed to decode re-encoded .osu");

    // Verify
    println!("\nVerification:");
    let notes_match = chart.notes.len() == chart2.notes.len();
    let timing_match = chart.timing_points.len() == chart2.timing_points.len();
    let key_match = chart.key_count == chart2.key_count;

    println!(
        "  Key count match: {} ({} == {})",
        if key_match { "✓" } else { "✗" },
        chart.key_count,
        chart2.key_count
    );
    println!(
        "  Notes count match: {} ({} == {})",
        if notes_match { "✓" } else { "✗" },
        chart.notes.len(),
        chart2.notes.len()
    );
    println!(
        "  Timing points match: {} ({} == {})",
        if timing_match { "✓" } else { "✗" },
        chart.timing_points.len(),
        chart2.timing_points.len()
    );

    // Verify note timestamps
    let mut timestamp_errors = 0;
    for (i, (n1, n2)) in chart.notes.iter().zip(chart2.notes.iter()).enumerate() {
        // Allow 1ms tolerance due to roundtrip precision
        if (n1.time_us - n2.time_us).abs() > 1000 {
            if timestamp_errors < 5 {
                println!(
                    "  Note {} time mismatch: {} vs {}",
                    i, n1.time_us, n2.time_us
                );
            }
            timestamp_errors += 1;
        }
    }
    if timestamp_errors == 0 {
        println!("  Note timestamps: ✓ (all match)");
    } else {
        println!("  Note timestamps: ✗ ({} mismatches)", timestamp_errors);
    }

    // Verify column distribution
    let mut col_distribution = vec![0u32; chart.key_count as usize];
    for note in &chart.notes {
        col_distribution[note.column as usize] += 1;
    }
    println!("\n  Column distribution:");
    for (col, count) in col_distribution.iter().enumerate() {
        let bar_len = (*count as usize / 50).min(40);
        let bar: String = (0..bar_len).map(|_| '█').collect();
        println!("    Col {}: {:>4} {}", col, count, bar);
    }

    if notes_match && timing_match && key_match && timestamp_errors == 0 {
        println!("\n✓ {} PASSED", name);
    } else {
        println!("\n✗ {} FAILED", name);
    }
}

fn main() {
    println!("osu!mania Roundtrip Test");
    println!("========================\n");
    println!("Testing full decode -> encode -> decode cycle");

    test_map("4K Map", "assets/osu/mania_4k.osu");
    test_map("7K Map", "assets/osu/mania_7k.osu");

    println!("\n\n============================================================");
    println!("All tests complete!");
    println!("============================================================");
}
