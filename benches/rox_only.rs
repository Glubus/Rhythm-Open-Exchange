use criterion::{criterion_group, criterion_main, Criterion};
use rhythm_open_exchange::codec::formats::osu::OsuDecoder;
use rhythm_open_exchange::codec::{Decoder, Encoder, RoxCodec};
use std::fs;
use std::hint::black_box;
use std::path::Path;

fn read_asset(path: &str) -> Vec<u8> {
    let p = Path::new("assets").join(path);
    fs::read(&p).unwrap_or_else(|_| panic!("Failed to read asset: {}", p.display()))
}

fn bench_rox_50k(c: &mut Criterion) {
    // Load the 50k map once
    let data = read_asset("osu/mania_4K_50K_notes.osu");
    // Decode from OSU to get the Chart object
    let chart = OsuDecoder::decode(&data).expect("Failed to decode OSU map");
    // Encode to ROX to get the binary data
    let rox_data = RoxCodec::encode(&chart).expect("Failed to encode ROX data");

    println!("ROX Data Size: {} bytes", rox_data.len());

    let mut group = c.benchmark_group("ROX_50K");
    group.sample_size(20); // Sufficient for stable results on large map

    group.bench_function("encode", |b| {
        b.iter(|| RoxCodec::encode(black_box(&chart)))
    });

    group.bench_function("decode", |b| {
        b.iter(|| RoxCodec::decode(black_box(&rox_data)))
    });

    group.finish();
}

criterion_group!(benches, bench_rox_50k);
criterion_main!(benches);
