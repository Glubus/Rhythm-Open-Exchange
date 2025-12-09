//! Benchmarks for Format Converters (Osu, Taiko, SM) using real assets.

use criterion::{Criterion, criterion_group, criterion_main};
use rhythm_open_exchange::codec::formats::osu::OsuDecoder;
use rhythm_open_exchange::codec::formats::osu::OsuEncoder;
use rhythm_open_exchange::codec::formats::sm::decoder::SmDecoder;
use rhythm_open_exchange::codec::formats::sm::encoder::SmEncoder;
use rhythm_open_exchange::codec::formats::taiko::{TaikoDecoder, types::ColumnLayout};
use rhythm_open_exchange::codec::{Decoder, Encoder, RoxCodec};
use std::fs;
use std::hint::black_box;
use std::path::Path;

fn read_asset(path: &str) -> Vec<u8> {
    let p = Path::new("assets").join(path);
    fs::read(&p).unwrap_or_else(|_| panic!("Failed to read asset: {}", p.display()))
}

fn bench_osu_mania(c: &mut Criterion) {
    let data = read_asset("osu/mania_4k.osu");
    let chart = OsuDecoder::decode(&data).unwrap();

    let mut group = c.benchmark_group("OsuMania");

    group.bench_function("decode_4k", |b| {
        b.iter(|| OsuDecoder::decode(black_box(&data)))
    });

    group.bench_function("encode_4k", |b| {
        b.iter(|| OsuEncoder::encode(black_box(&chart)))
    });

    group.finish();
}

fn bench_taiko(c: &mut Criterion) {
    let data = read_asset("osu/taiko.osu");

    let mut group = c.benchmark_group("OsuTaiko");

    group.bench_function("decode", |b| {
        b.iter(|| TaikoDecoder::decode_with_layout(black_box(&data), ColumnLayout::Dkkd))
    });

    group.finish();
}

fn bench_stepmania(c: &mut Criterion) {
    let data = read_asset("stepmania/4k.sm");
    let chart = SmDecoder::decode(&data).unwrap();

    let mut group = c.benchmark_group("StepMania");

    group.bench_function("decode", |b| b.iter(|| SmDecoder::decode(black_box(&data))));

    group.bench_function("encode", |b| {
        b.iter(|| SmEncoder::encode(black_box(&chart)))
    });

    group.finish();
}

fn bench_limit_large(c: &mut Criterion) {
    let data = read_asset("osu/mania_4K_50K_notes.osu");
    let chart = OsuDecoder::decode(&data).unwrap();
    let rox_data = RoxCodec::encode(&chart).unwrap();

    let mut group = c.benchmark_group("Large_50K");
    // Set sample size lower for large benchmarks to avoid long runtime
    group.sample_size(10);

    group.bench_function("osu_decode", |b| {
        b.iter(|| OsuDecoder::decode(black_box(&data)))
    });

    group.bench_function("rox_encode", |b| {
        b.iter(|| RoxCodec::encode(black_box(&chart)))
    });

    group.bench_function("rox_decode", |b| {
        b.iter(|| RoxCodec::decode(black_box(&rox_data)))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_osu_mania,
    bench_taiko,
    bench_stepmania,
    bench_limit_large
);
criterion_main!(benches);
