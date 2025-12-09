//! Example: Convert StepMania (.sm) to osu!mania (.osu) format.
//!
//! Usage: cargo run --example sm_to_osu

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
    let osu_content = String::from_utf8_lossy(&osu_data);

    println!("\n=== Output (.osu) Preview ===\n");
    
    // Print first 50 lines
    for line in osu_content.lines().take(50) {
        println!("{}", line);
    }
    
    println!("\n... ({} bytes total)", osu_data.len());
}
