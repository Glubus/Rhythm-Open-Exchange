//! Benchmark different compression levels to find optimal speed/size balance.

use rand::Rng;
use rhythm_open_exchange::{Encoder, Metadata, Note, RoxChart, RoxCodec, TimingPoint};
use std::time::Instant;

fn create_realistic_chart() -> RoxChart {
    let mut rng = rand::rng();
    let mut chart = RoxChart::new(7);

    chart.metadata = Metadata {
        title: "Test Chart".into(),
        artist: "Artist".into(),
        creator: "Mapper".into(),
        difficulty_name: "Hard".into(),
        audio_file: "audio.ogg".into(),
        ..Default::default()
    };

    // Add timing points
    for i in 0..50 {
        let time = i * 6_000_000;
        if i % 2 == 0 {
            chart
                .timing_points
                .push(TimingPoint::bpm(time, rng.random_range(120.0..200.0)));
        } else {
            chart
                .timing_points
                .push(TimingPoint::sv(time, rng.random_range(0.5..2.0)));
        }
    }

    // Add 5000 notes with random timing (realistic chart size)
    let mut current_time: i64 = 0;
    for _ in 0..5000 {
        current_time += rng.random_range(10_000..100_000);
        let column = rng.random_range(0..7) as u8;
        chart.notes.push(Note::tap(current_time, column));
    }

    chart.notes.sort_by_key(|n| n.time_us);
    chart
}

fn main() {
    println!("Compression Level Benchmark\n");
    println!("Testing with 5000-note chart (realistic size)\n");

    let chart = create_realistic_chart();

    // Test encoding speed
    let iterations = 100;

    let start = Instant::now();
    let mut last_size = 0;
    for _ in 0..iterations {
        let encoded = RoxCodec::encode(&chart).unwrap();
        last_size = encoded.len();
    }
    let elapsed = start.elapsed();

    let avg_ms = elapsed.as_secs_f64() * 1000.0 / iterations as f64;

    println!("Results:");
    println!("  Iterations: {}", iterations);
    println!("  Total time: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    println!("  Average encode time: {:.3}ms", avg_ms);
    println!(
        "  Encoded size: {} bytes ({:.2} KB)",
        last_size,
        last_size as f64 / 1024.0
    );

    // Estimate batch processing
    println!("\nBatch processing estimates:");
    println!("  1000 charts: {:.2}s", avg_ms * 1000.0 / 1000.0);
    println!("  10000 charts: {:.2}s", avg_ms * 10000.0 / 1000.0);

    // Compare with no compression baseline
    println!("\nNote: Current compression level is set in src/codec/rox.rs");
}
