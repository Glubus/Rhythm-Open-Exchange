//! Example: Convert StepMania (.sm) to osu!mania (.osu) format.
//!
//! Usage: cargo run --example sm_to_osu

use std::fs;
use std::path::Path;

use rhythm_open_exchange::codec::formats::osu::OsuEncoder;
use rhythm_open_exchange::codec::formats::sm::SmDecoder;
use rhythm_open_exchange::codec::{Decoder, Encoder};

fn main() {
    // Load a sample SM file
    let sm_data = include_bytes!("../assets/stepmania/4k.sm");

    println!("=== SM to OSU Conversion ===\n");

    // Decode SM to RoxChart
    let chart = SmDecoder::decode(sm_data).expect("Failed to decode SM file");

    println!("Source: StepMania (.sm)");
    println!("Title: {}", chart.metadata.title);
    println!("Artist: {}", chart.metadata.artist);
    println!("Key count: {}K", chart.key_count);
    println!("Notes: {}", chart.notes.len());

    // Encode to osu! format
    let osu_data = OsuEncoder::encode(&chart).expect("Failed to encode to osu!");

    // Write to file
    let output_dir = Path::new("output");
    fs::create_dir_all(output_dir).expect("Failed to create output directory");

    let output_path = output_dir.join("output.osu");
    fs::write(&output_path, &osu_data).expect("Failed to write .osu file");

    println!("\n✓ Written to: {}", output_path.display());
    println!("  Size: {} bytes", osu_data.len());
}
