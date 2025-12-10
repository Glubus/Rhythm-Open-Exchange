//! Convert Friday Night Funkin' .json charts to osu!mania .osu format.
//!
//! ```bash
//! cargo run --example fnf_to_osu
//! ```

use rhythm_open_exchange::codec::auto_encode;
use rhythm_open_exchange::codec::formats::FnfDecoder;
use rhythm_open_exchange::codec::formats::fnf::FnfSide;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ensure output directory exists
    std::fs::create_dir_all("output")?;

    let asset = "assets/fnf/test-song.json";
    println!("=== Converting {} ===\n", asset);

    let data = std::fs::read(asset)?;

    // Decode player side only (4K)
    let player_chart = FnfDecoder::decode_with_side(&data, FnfSide::Player)?;
    println!("Player only: {} notes (4K)", player_chart.notes.len());
    auto_encode(&player_chart, "output/fnf_player.osu")?;
    println!("  ✓ Saved to: output/fnf_player.osu");

    // Decode opponent side only (4K)
    let opponent_chart = FnfDecoder::decode_with_side(&data, FnfSide::Opponent)?;
    println!("Opponent only: {} notes (4K)", opponent_chart.notes.len());
    auto_encode(&opponent_chart, "output/fnf_opponent.osu")?;
    println!("  ✓ Saved to: output/fnf_opponent.osu");

    // Decode both sides (8K coop)
    let both_chart = FnfDecoder::decode_with_side(&data, FnfSide::Both)?;
    println!("Both sides: {} notes (8K coop)", both_chart.notes.len());
    auto_encode(&both_chart, "output/fnf_coop.osu")?;
    println!("  ✓ Saved to: output/fnf_coop.osu");

    // Also export back to FNF JSON
    auto_encode(&player_chart, "output/fnf_roundtrip.json")?;
    println!("\n  ✓ Roundtrip to: output/fnf_roundtrip.json");

    println!("\n✓ Done! Check the output/ folder.");
    Ok(())
}
