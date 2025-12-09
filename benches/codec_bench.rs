//! Benchmarks for ROX codec encode/decode performance.

use criterion::{Criterion, criterion_group, criterion_main};
use rhythm_open_exchange::{Decoder, Encoder, Metadata, Note, RoxChart, RoxCodec, TimingPoint};
use std::hint::black_box;

/// Create a large chart for benchmarking (32000 notes, 100 timing points).
fn create_large_chart() -> RoxChart {
    let mut chart = RoxChart::new(7);

    chart.metadata = Metadata {
        title: "Benchmark Chart".into(),
        artist: "Benchmark".into(),
        creator: "Criterion".into(),
        difficulty_name: "EXTREME".into(),
        difficulty_value: Some(10.0),
        audio_file: "audio.ogg".into(),
        background_file: Some("bg.jpg".into()),
        preview_time_us: 60_000_000,
        ..Default::default()
    };

    // 100 timing points
    let chart_duration_us: i64 = 600_000_000;
    let timing_interval = chart_duration_us / 100;

    for i in 0..100 {
        let time_us = i * timing_interval;
        if i % 2 == 0 {
            chart
                .timing_points
                .push(TimingPoint::bpm(time_us, 120.0 + (i as f32 * 1.5)));
        } else {
            chart
                .timing_points
                .push(TimingPoint::sv(time_us, 0.5 + (i % 10) as f32 * 0.15));
        }
    }

    // 32000 notes
    let note_count = 32000;
    let note_interval = chart_duration_us / note_count as i64;

    for i in 0..note_count {
        let time_us = i as i64 * note_interval;
        let column = (i % 7) as u8;

        let note = match i % 20 {
            0..=14 => Note::tap(time_us, column),
            15..=17 => Note::hold(time_us, note_interval * 2, column),
            18 => Note::burst(time_us, note_interval * 3, column),
            19 => Note::mine(time_us, (column + 3) % 7),
            _ => Note::tap(time_us, column),
        };

        chart.notes.push(note);
    }

    chart
}

/// Create a medium chart for benchmarking (5000 notes, 20 timing points).
fn create_medium_chart() -> RoxChart {
    let mut chart = RoxChart::new(4);
    chart.timing_points.push(TimingPoint::bpm(0, 180.0));

    for i in 0..20 {
        chart
            .timing_points
            .push(TimingPoint::sv(i * 1_000_000, 1.0 + (i % 5) as f32 * 0.2));
    }

    for i in 0..5000 {
        chart.notes.push(Note::tap(i * 50_000, (i % 4) as u8));
    }

    chart
}

/// Create a small chart for benchmarking (500 notes, 5 timing points).
fn create_small_chart() -> RoxChart {
    let mut chart = RoxChart::new(4);
    chart.timing_points.push(TimingPoint::bpm(0, 120.0));

    for i in 0..5 {
        chart.timing_points.push(TimingPoint::sv(i * 500_000, 1.0));
    }

    for i in 0..500 {
        chart.notes.push(Note::tap(i * 100_000, (i % 4) as u8));
    }

    chart
}

fn bench_encode_large(c: &mut Criterion) {
    let chart = create_large_chart();

    c.bench_function("encode_32000_notes", |b| {
        b.iter(|| RoxCodec::encode(black_box(&chart)))
    });
}

fn bench_decode_large(c: &mut Criterion) {
    let chart = create_large_chart();
    let encoded = RoxCodec::encode(&chart).unwrap();

    c.bench_function("decode_32000_notes", |b| {
        b.iter(|| RoxCodec::decode(black_box(&encoded)))
    });
}

fn bench_encode_medium(c: &mut Criterion) {
    let chart = create_medium_chart();

    c.bench_function("encode_5000_notes", |b| {
        b.iter(|| RoxCodec::encode(black_box(&chart)))
    });
}

fn bench_decode_medium(c: &mut Criterion) {
    let chart = create_medium_chart();
    let encoded = RoxCodec::encode(&chart).unwrap();

    c.bench_function("decode_5000_notes", |b| {
        b.iter(|| RoxCodec::decode(black_box(&encoded)))
    });
}

fn bench_encode_small(c: &mut Criterion) {
    let chart = create_small_chart();

    c.bench_function("encode_500_notes", |b| {
        b.iter(|| RoxCodec::encode(black_box(&chart)))
    });
}

fn bench_decode_small(c: &mut Criterion) {
    let chart = create_small_chart();
    let encoded = RoxCodec::encode(&chart).unwrap();

    c.bench_function("decode_500_notes", |b| {
        b.iter(|| RoxCodec::decode(black_box(&encoded)))
    });
}

fn bench_roundtrip(c: &mut Criterion) {
    let chart = create_medium_chart();

    c.bench_function("roundtrip_5000_notes", |b| {
        b.iter(|| {
            let encoded = RoxCodec::encode(black_box(&chart)).unwrap();
            RoxCodec::decode(black_box(&encoded))
        })
    });
}

criterion_group!(
    benches,
    bench_encode_large,
    bench_decode_large,
    bench_encode_medium,
    bench_decode_medium,
    bench_encode_small,
    bench_decode_small,
    bench_roundtrip,
);

criterion_main!(benches);
