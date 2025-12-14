//! Example: Convert osu!taiko (.osu) to osu!mania (.osu) format via ROX.
//!
//! Usage: cargo run --example taiko_to_osu

use std::fs;
use std::path::Path;

use rhythm_open_exchange::codec::Encoder;
use rhythm_open_exchange::codec::formats::osu::OsuEncoder;
use rhythm_open_exchange::codec::formats::taiko::{TaikoDecoder, types::ColumnLayout};

fn main() {
    // Use the real Taiko file from assets
    let input_path = Path::new("assets/osu/taiko.osu");
    if !input_path.exists() {
        panic!("Please ensure assets/osu/taiko.osu exists before running this example.");
    }

    let taiko_data = fs::read(input_path).expect("Failed to read taiko.osu");

    println!("=== Taiko to ROX to Osu!mania Conversion ===\n");

    // Decode Taiko to RoxChart (4K) using default DKKD layout
    // You can use decode_with_layout() to choose Dkdk or Kddk
    let chart = TaikoDecoder::decode_with_layout(&taiko_data, ColumnLayout::Dkkd)
        .expect("Failed to decode Taiko file");

    println!("Title: {}", chart.metadata.title);
    println!("Artist: {}", chart.metadata.artist);
    println!(
        "Chart Key Count: {}K (Converted from Taiko)",
        chart.key_count()
    );
    println!("Notes: {}", chart.notes.len());

    // Encode to osu!mania format
    let osu_data = OsuEncoder::encode(&chart).expect("Failed to encode to osu!");

    // Write to file
    let output_dir = Path::new("output");
    fs::create_dir_all(output_dir).expect("Failed to create output directory");

    let output_path = output_dir.join("taiko_converted.osu");
    fs::write(&output_path, &osu_data).expect("Failed to write .osu file");

    println!("\n✓ Written to: {}", output_path.display());
    println!("  Size: {} bytes", osu_data.len());
}
