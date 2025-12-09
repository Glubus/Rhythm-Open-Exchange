//! Example: Generate a large .rox file with 32000 notes and 100 timing points.
//! Uses random timing between notes for realistic compression testing.
//!
//! Run with: `cargo run --example generate_large`

use rand::Rng;
use rhythm_open_exchange::{Decoder, Encoder, Metadata, Note, RoxChart, RoxCodec, TimingPoint};

fn main() {
    println!("Generating large ROX chart with random timing...");

    let mut rng = rand::rng();

    // Create a 7K chart
    let mut chart = RoxChart::new(7);

    // Set metadata
    chart.metadata = Metadata {
        title: "Generated Marathon Chart".into(),
        artist: "ROX Generator".into(),
        creator: "Example Script".into(),
        difficulty_name: "EXTREME".into(),
        difficulty_value: Some(10.0),
        audio_file: "marathon.ogg".into(),
        background_file: Some("bg.jpg".into()),
        preview_time_us: 60_000_000,
        preview_duration_us: 30_000_000,
        source: Some("Generated".into()),
        genre: Some("Speedcore".into()),
        language: None,
        tags: vec![
            "marathon".into(),
            "generated".into(),
            "32000notes".into(),
            "benchmark".into(),
        ],
        ..Default::default()
    };

    // Add 100 timing points with random BPM and SV values
    println!("Adding 100 timing points...");

    let chart_duration_us: i64 = 600_000_000; // 10 minutes in microseconds

    for i in 0..100 {
        // Random time position within the chart duration
        let time_us = (i as i64 * chart_duration_us / 100) + rng.random_range(0..50_000);

        if i % 2 == 0 {
            // Random BPM between 100 and 250
            let bpm = rng.random_range(100.0..250.0);
            chart.timing_points.push(TimingPoint::bpm(time_us, bpm));
        } else {
            // Random SV between 0.3 and 2.5
            let sv = rng.random_range(0.3..2.5);
            chart.timing_points.push(TimingPoint::sv(time_us, sv));
        }
    }

    // Add 32000 notes with random timing
    println!("Adding 32000 notes with random timing...");

    let note_count = 32000;
    let avg_interval = chart_duration_us / note_count as i64;
    let mut current_time: i64 = 0;

    for i in 0..note_count {
        // Random interval: base interval ± 50%
        let jitter = rng.random_range(-(avg_interval / 2)..(avg_interval / 2));
        current_time += avg_interval + jitter;
        current_time = current_time.max(0); // Ensure non-negative

        // Random column
        let column = rng.random_range(0..7) as u8;

        // Mix of note types with random durations
        let note = match i % 20 {
            0..=14 => Note::tap(current_time, column),
            15..=17 => {
                let duration = rng.random_range(50_000..500_000);
                Note::hold(current_time, duration, column)
            }
            18 => {
                let duration = rng.random_range(100_000..300_000);
                Note::burst(current_time, duration, column)
            }
            19 => Note::mine(current_time, rng.random_range(0..7) as u8),
            _ => Note::tap(current_time, column),
        };

        chart.notes.push(note);
    }

    // Sort notes by time (important for delta encoding)
    chart.notes.sort_by_key(|n| n.time_us);

    // Validate the chart
    println!("Validating chart...");
    chart.validate().expect("Chart validation failed!");

    // Print stats
    println!("\nChart Statistics:");
    println!("  Key count: {}K", chart.key_count);
    println!("  Notes: {}", chart.note_count());
    println!("  Timing points: {}", chart.timing_points.len());
    println!(
        "  Duration: {:.2} minutes",
        chart.duration_us() as f64 / 60_000_000.0
    );
    println!("  Hash: {}", chart.short_hash());

    // Encode to bytes
    println!("\nEncoding...");
    let encoded = RoxCodec::encode(&chart).expect("Encoding failed!");
    println!(
        "  Encoded size: {} bytes ({:.2} KB)",
        encoded.len(),
        encoded.len() as f64 / 1024.0
    );

    // Save to file
    let output_path = "large_chart.rox";
    RoxCodec::encode_to_path(&chart, output_path).expect("Failed to write file!");
    println!("  Saved to: {}", output_path);

    // Verify by decoding
    println!("\nVerifying...");
    let decoded = RoxCodec::decode(&encoded).expect("Decoding failed!");

    assert_eq!(decoded.notes.len(), note_count);
    assert_eq!(decoded.timing_points.len(), 100);
    assert_eq!(decoded.metadata.title, "Generated Marathon Chart");

    // Verify note data integrity
    for (i, (orig, dec)) in chart.notes.iter().zip(decoded.notes.iter()).enumerate() {
        assert_eq!(orig.time_us, dec.time_us, "Note {} time mismatch", i);
        assert_eq!(orig.column, dec.column, "Note {} column mismatch", i);
    }

    println!("  Verification passed!");
    println!("\nDone!");
}
