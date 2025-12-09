//! Example: Convert osu!taiko (.osu) to osu!mania (.osu) format via ROX.
//!
//! Usage: cargo run --example taiko_to_osu

use std::fs;
use std::path::Path;

use rhythm_open_exchange::codec::Encoder;
use rhythm_open_exchange::codec::formats::osu::OsuEncoder;
use rhythm_open_exchange::codec::formats::taiko::{TaikoDecoder, types::ColumnLayout};

fn main() {
    // Note: Since we don't have a real .osu taiko file in assets yet,
    // we'll use a hardcoded string for demonstration or a user provided file if available.
    // Ideally this would read from assets/taiko/test.osu

    // Simulating input data for now, or reading if it exists.
    let input_path = Path::new("assets/taiko/test.osu");
    let taiko_data = if input_path.exists() {
        fs::read(input_path).expect("Failed to read test.osu")
    } else {
        println!(
            "No test file found at {}, using synthetic data.",
            input_path.display()
        );
        // Simple synthetic Taiko map
        b"osu file format v14
[General]
Mode: 1
AudioFilename: audio.mp3

[Metadata]
Title:Synthetic Taiko
Artist:Rust
Creator:Rox
Version:Oni

[TimingPoints]
0,500,4,1,0,100,1,0

[HitObjects]
256,192,1000,1,0,0:0:0:0:
256,192,1250,1,2,0:0:0:0:
256,192,1500,1,4,0:0:0:0:
256,192,1750,1,8,0:0:0:0:
"
        .to_vec()
    };

    println!("=== Taiko to ROX to Osu!mania Conversion ===\n");

    // Decode Taiko to RoxChart (4K) using default DKKD layout
    // You can use decode_with_layout() to choose Dkdk or Kddk
    let chart = TaikoDecoder::decode_with_layout(&taiko_data, ColumnLayout::Dkkd)
        .expect("Failed to decode Taiko file");

    println!("Title: {}", chart.metadata.title);
    println!("Artist: {}", chart.metadata.artist);
    println!(
        "Chart Key Count: {}K (Converted from Taiko)",
        chart.key_count
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
