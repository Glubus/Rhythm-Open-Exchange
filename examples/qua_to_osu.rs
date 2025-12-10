//! Convert Quaver .qua files to osu!mania .osu format.
//!
//! ```bash
//! cargo run --example qua_to_osu
//! ```

use std::path::Path;

use rhythm_open_exchange::codec::{auto_decode, auto_encode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ensure output directory exists
    std::fs::create_dir_all("output")?;

    let assets = ["assets/quaver/4K.qua", "assets/quaver/7K.qua"];

    for asset in assets {
        println!("\n=== Converting {} ===", asset);

        // Decode .qua to RoxChart
        let chart = auto_decode(asset)?;
        println!(
            "  Title: {} - {}",
            chart.metadata.artist, chart.metadata.title
        );
        println!(
            "  Diff: {} [{}K, {} notes]",
            chart.metadata.difficulty_name,
            chart.key_count,
            chart.notes.len()
        );

        // Generate output filename
        let input_stem = Path::new(asset)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let output_path = format!("output/{}.osu", input_stem);

        // Encode to .osu
        auto_encode(&chart, &output_path)?;
        println!("  ✓ Saved to: {}", output_path);
    }

    println!("\n✓ Done! Check the output/ folder.");
    Ok(())
}
