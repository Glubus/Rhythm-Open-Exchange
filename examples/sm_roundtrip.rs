use rhythm_open_exchange::codec::formats::sm::{SmDecoder, SmEncoder};
use rhythm_open_exchange::codec::{Decoder, Encoder};
use std::fs;
use std::path::Path;

fn main() -> rhythm_open_exchange::error::RoxResult<()> {
    // Setup paths
    let input_path = Path::new("assets/stepmania/test.sm");
    let output_dir = Path::new("output");
    let output_path = output_dir.join("test.sm");

    println!("Reading from: {:?}", input_path);
    let data = fs::read(input_path)?;

    println!("Decoding SM file...");
    let chart = SmDecoder::decode(&data)?;
    println!("Decoded chart: {}", chart.metadata.title);
    println!("  Artist: {}", chart.metadata.artist);
    println!("  Notes: {}", chart.notes.len());

    println!("Encoding to SM...");
    let encoded_data = SmEncoder::encode(&chart)?;

    // Ensure output directory exists
    if !output_dir.exists() {
        fs::create_dir_all(output_dir)?;
    }

    println!("Writing to: {:?}", output_path);
    fs::write(&output_path, encoded_data)?;

    println!("Done!");
    Ok(())
}
