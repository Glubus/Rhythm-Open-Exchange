//! Example: Convert StepMania (.sm) to ROX format.
//!
//! Usage: cargo run --example sm_to_rox

use rhythm_open_exchange::codec::formats::sm::SmDecoder;
use rhythm_open_exchange::codec::{Decoder, Encoder, RoxCodec};

fn main() {
    // Load a sample SM file
    let sm_data = include_bytes!("../assets/stepmania/4k.sm");

    println!("=== SM to ROX Conversion ===\n");

    // Decode SM to RoxChart
    let chart = SmDecoder::decode(sm_data).expect("Failed to decode SM file");

    println!("Title: {}", chart.metadata.title);
    println!("Artist: {}", chart.metadata.artist);
    println!("Creator: {}", chart.metadata.creator);
    println!("Difficulty: {}", chart.metadata.difficulty_name);
    println!("Key count: {}K", chart.key_count);
    println!("Notes: {}", chart.notes.len());
    println!("Timing points: {}", chart.timing_points.len());

    // Encode to ROX format
    let rox_data = RoxCodec::encode(&chart).expect("Failed to encode to ROX");

    println!("\nROX file size: {} bytes", rox_data.len());
    println!(
        "Compression ratio: {:.1}x",
        sm_data.len() as f64 / rox_data.len() as f64
    );

    // Verify roundtrip
    let decoded = RoxCodec::decode(&rox_data).expect("Failed to decode ROX");
    assert_eq!(chart, decoded);
    println!("\n✓ Roundtrip verification passed!");
}
