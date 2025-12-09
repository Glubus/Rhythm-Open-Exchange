//! Example: Full roundtrip .osu -> .rox -> .osu
//!
//! Run with: `cargo run --example osu_to_rox`

use rhythm_open_exchange::RoxCodec;
use rhythm_open_exchange::codec::formats::osu::{OsuDecoder, OsuEncoder};
use rhythm_open_exchange::codec::{Decoder, Encoder};
use std::fs;

fn convert(input_osu: &str, name: &str) {
    let output_rox = format!("output/{}.rox", name);
    let output_osu = format!("output/{}.osu", name);

    println!("\n=== {} ===", input_osu);

    // Step 1: Load .osu
    let chart = OsuDecoder::decode_from_path(input_osu).expect("Failed to decode .osu");
    println!(
        "Loaded: {} [{}]",
        chart.metadata.title, chart.metadata.difficulty_name
    );
    println!("  {}K | {} notes", chart.key_count, chart.notes.len());

    // Step 2: Save as .rox
    RoxCodec::encode_to_path(&chart, &output_rox).expect("Failed to write .rox");
    let rox_size = fs::metadata(&output_rox).unwrap().len();
    println!("Saved .rox: {} bytes -> {}", rox_size, output_rox);

    // Step 3: Load .rox back
    let chart2 = RoxCodec::decode_from_path(&output_rox).expect("Failed to decode .rox");
    assert_eq!(chart.notes.len(), chart2.notes.len());
    println!("Verified .rox roundtrip ✓");

    // Step 4: Save as .osu
    OsuEncoder::encode_to_path(&chart2, &output_osu).expect("Failed to write .osu");
    let osu_size = fs::metadata(&output_osu).unwrap().len();
    println!("Saved .osu: {} bytes -> {}", osu_size, output_osu);

    // Step 5: Load .osu again and verify
    let chart3 =
        OsuDecoder::decode_from_path(&output_osu).expect("Failed to decode re-encoded .osu");

    assert_eq!(chart.key_count, chart3.key_count, "Key count mismatch");
    assert_eq!(chart.notes.len(), chart3.notes.len(), "Note count mismatch");
    assert_eq!(
        chart.timing_points.len(),
        chart3.timing_points.len(),
        "Timing points mismatch"
    );

    println!("Verified full roundtrip: .osu -> .rox -> .osu ✓");
}

fn main() {
    // Create output directory if it doesn't exist
    fs::create_dir_all("output").expect("Failed to create output directory");

    println!("osu!mania Full Roundtrip Test");
    println!("==============================");
    println!(".osu -> .rox -> .osu with encode_to_path\n");

    convert("assets/osu/mania_4k.osu", "mania_4k");
    convert("assets/osu/mania_7k.osu", "mania_7k");

    println!("\n==============================");
    println!("All roundtrips passed!");
    println!("Output files saved to output/");
}
