use rhythm_open_exchange::codec::{auto_decode, auto_encode};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This example assumes you have an osu file in assets
    let asset_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let osu_path = asset_dir.join("osu/mania_7k.osu");

    if !osu_path.exists() {
        println!("Asset not found: {:?}", osu_path);
        return Ok(());
    }

    println!("Decoding osu! chart...");
    let chart = auto_decode(&osu_path)?;

    println!("Chart loaded:");
    println!("  Title: {}", chart.metadata.title);
    println!("  Artist: {}", chart.metadata.artist);
    println!("  Key Count: {}", chart.key_count());
    println!("  Notes: {}", chart.note_count());

    // Encode to ROX bytes
    println!("\nEncoding to ROX format...");
    let _ = auto_encode(&chart, "output.rox")?;
    println!("Encoded to output.rox");

    // Convert directly (simulated)
    println!("\nConverting directly osu -> rox...");
    // In a real scenario, this would write to disk if the API supported file-to-file,
    // but auto_convert currently takes paths.
    // Let's assume we want to convert "input.osu" to "output.sm"
    // auto_convert(osu_path, "output.sm")?; // This writes to disk

    Ok(())
}
