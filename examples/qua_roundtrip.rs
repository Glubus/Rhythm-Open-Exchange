//! Quick roundtrip test for Quaver format.
//!
//! ```bash
//! cargo run --example qua_roundtrip
//! ```

use rhythm_open_exchange::codec::{auto_decode, auto_encode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let assets = ["assets/quaver/4K.qua", "assets/quaver/7K.qua"];

    for asset in assets {
        println!("\n=== Testing {} ===", asset);

        // Decode .qua to RoxChart
        let chart = auto_decode(asset)?;
        println!(
            "Decoded: {} - {} [{} notes, {}K]",
            chart.metadata.title,
            chart.metadata.difficulty_name,
            chart.note_count(),
            chart.key_count()
        );
        println!(
            "  Timing points: {} BPM, {} SV",
            chart
                .timing_points
                .iter()
                .filter(|tp| !tp.is_inherited)
                .count(),
            chart
                .timing_points
                .iter()
                .filter(|tp| tp.is_inherited)
                .count()
        );

        // Encode back to .qua
        let output_path = format!("target/{}_roundtrip.qua", chart.key_count());
        auto_encode(&chart, &output_path)?;
        println!("Encoded to: {}", output_path);

        // Re-decode to verify
        let chart2 = auto_decode(&output_path)?;
        assert_eq!(
            chart.note_count(),
            chart2.note_count(),
            "Note count mismatch"
        );
        assert_eq!(chart.key_count(), chart2.key_count(), "Key count mismatch");
        println!("  ✓ Roundtrip verified ({} notes)", chart2.note_count());
    }

    println!("\n✓ All tests passed!");
    Ok(())
}
