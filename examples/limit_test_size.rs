//! Limit Test: Size Comparison for Large Map (50k notes).
//!
//! usage: cargo run --example limit_test_size

use rhythm_open_exchange::codec::formats::osu::OsuDecoder;
use rhythm_open_exchange::codec::{Decoder, Encoder, RoxCodec};
use std::fs;
use std::path::Path;
use std::time::Instant;

fn main() {
    let input_path = Path::new("assets/osu/mania_4K_50K_notes.osu");

    if !input_path.exists() {
        panic!(
            "Please ensure assets/osu/mania_4K_50K_notes.osu exists before running this example."
        );
    }

    println!("=== ROX Limit Test: Size Comparison (50k Notes) ===\n");

    // 1. Read Osu File
    let start_read = Instant::now();
    let osu_data = fs::read(input_path).expect("Failed to read osu file");
    let osu_size = osu_data.len();
    println!("Reading .osu file: {:.2?}", start_read.elapsed());
    println!(
        "Original Size (.osu): {} bytes ({:.2} MB)",
        osu_size,
        osu_size as f64 / 1024.0 / 1024.0
    );

    // 2. Parse Osu
    let start_parse = Instant::now();
    let chart = OsuDecoder::decode(&osu_data).expect("Failed to decode osu file");
    println!("Parsing .osu file: {:.2?}", start_parse.elapsed());
    println!("Chart Notes: {}", chart.notes.len());

    // 3. Encode to ROX
    let start_encode = Instant::now();
    let rox_data = RoxCodec::encode(&chart).expect("Failed to encode to ROX");
    let rox_size = rox_data.len();
    println!("Encoding to .rox: {:.2?}", start_encode.elapsed());

    // 4. File IO Round-trip
    let output_dir = Path::new("output");
    fs::create_dir_all(output_dir).expect("Failed to create output dir");

    let rox_path = output_dir.join("limit_test_50k.rox");
    fs::write(&rox_path, &rox_data).expect("Failed to write rox file");
    println!("\nWritten to: {}", rox_path.display());

    // Reload
    let reload_start = Instant::now();
    let reloaded_data = fs::read(&rox_path).expect("Failed to read back rox file");
    let reloaded_chart = RoxCodec::decode(&reloaded_data).expect("Failed to decode rox file");
    println!("Reloading .rox file: {:.2?}", reload_start.elapsed());

    // Encode back to OSU
    let osu_encode_start = Instant::now();
    let osu_out_data =
        rhythm_open_exchange::codec::formats::osu::OsuEncoder::encode(&reloaded_chart)
            .expect("Failed to encode to osu");
    println!("Encoding back to .osu: {:.2?}", osu_encode_start.elapsed());

    let osu_out_path = output_dir.join("limit_test_50k_output.osu");
    fs::write(&osu_out_path, &osu_out_data).expect("Failed to write output osu file");
    println!("Written to: {}", osu_out_path.display());

    // 5. Results
    println!("\n=== Results ===");
    println!(
        "Original Osu Size: {} bytes ({:.2} MB)",
        osu_size,
        osu_size as f64 / 1024.0 / 1024.0
    );
    println!(
        "ROX Size:          {} bytes ({:.2} MB)",
        rox_size,
        rox_size as f64 / 1024.0 / 1024.0
    );

    let ratio = osu_size as f64 / rox_size as f64;
    let reduction = (1.0 - (rox_size as f64 / osu_size as f64)) * 100.0;

    println!("Compression Ratio: {:.2}x", ratio);
    println!("Size Reduction: {:.2}%", reduction);

    if rox_size < osu_size {
        println!("\n✅ SUCCESS: ROX is smaller!");
    } else {
        println!("\n⚠️ WARNING: ROX is larger!");
    }
}
