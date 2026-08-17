use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rox::codec::{Decoder, Encoder};
use rox_formats::{
    FnfDecoder, FnfEncoder, JroxDecoder, JroxEncoder, McDecoder, McEncoder, OsuDecodeOptions,
    OsuDecoder, OsuEncoder, QuaDecoder, QuaEncoder, RoxNativeCodec, SmDecoder, SmEncoder,
    TaikoDecoder,
};
use rox_test_utils::get_test_asset;

// ---------------------------------------------------------------------------
// Decode benchmarks
// ---------------------------------------------------------------------------

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode");

    let osu_4k = get_test_asset("osu/mania_4k.osu");
    group.bench_function("osu/4K", |b| {
        b.iter(|| OsuDecoder::decode(&osu_4k).unwrap())
    });

    let osu_7k = get_test_asset("osu/mania_7k.osu");
    group.bench_function("osu/7K", |b| {
        b.iter(|| OsuDecoder::decode(&osu_7k).unwrap())
    });

    // Heavy asset: requires re_arrange_bpm to bypass BpmAfterFirstNote validation
    let osu_50k = get_test_asset("osu/mania_4K_50K_notes.osu");
    let opts = OsuDecodeOptions {
        re_arrange_bpm: true,
    };
    group.bench_function("osu/4K_50K_notes", |b| {
        b.iter(|| OsuDecoder::decode_with_options(&osu_50k, &opts).unwrap())
    });

    let taiko = get_test_asset("osu/taiko.osu");
    group.bench_function("taiko", |b| {
        b.iter(|| TaikoDecoder::decode(&taiko).unwrap())
    });

    let sm = get_test_asset("stepmania/4k.sm");
    group.bench_function("sm/4K", |b| b.iter(|| SmDecoder::decode(&sm).unwrap()));

    let qua_4k = get_test_asset("quaver/4K.qua");
    group.bench_function("qua/4K", |b| {
        b.iter(|| QuaDecoder::decode(&qua_4k).unwrap())
    });

    let qua_7k = get_test_asset("quaver/7K.qua");
    group.bench_function("qua/7K", |b| {
        b.iter(|| QuaDecoder::decode(&qua_7k).unwrap())
    });

    let fnf = get_test_asset("fnf/test-song.json");
    group.bench_function("fnf", |b| b.iter(|| FnfDecoder::decode(&fnf).unwrap()));

    let mc_source = OsuDecoder::decode(&get_test_asset("osu/mania_4k.osu")).unwrap();
    let mc_data = McEncoder::encode(&mc_source).unwrap();
    group.bench_function("mc/4K", |b| b.iter(|| McDecoder::decode(&mc_data).unwrap()));

    // JROX: binary format — benchmark via a pre-encoded chart
    let jrox_source = OsuDecoder::decode(&get_test_asset("osu/mania_4k.osu")).unwrap();
    let jrox_data = JroxEncoder::encode(&jrox_source).unwrap();
    group.bench_function("jrox", |b| {
        b.iter(|| JroxDecoder::decode(&jrox_data).unwrap())
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Encode benchmarks
// ---------------------------------------------------------------------------

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode");

    let osu_chart = OsuDecoder::decode(&get_test_asset("osu/mania_4k.osu")).unwrap();
    group.bench_function("osu/4K", |b| {
        b.iter(|| OsuEncoder::encode(&osu_chart).unwrap())
    });

    let sm_chart = SmDecoder::decode(&get_test_asset("stepmania/4k.sm")).unwrap();
    group.bench_function("sm/4K", |b| {
        b.iter(|| SmEncoder::encode(&sm_chart).unwrap())
    });

    let qua_chart = QuaDecoder::decode(&get_test_asset("quaver/4K.qua")).unwrap();
    group.bench_function("qua/4K", |b| {
        b.iter(|| QuaEncoder::encode(&qua_chart).unwrap())
    });

    let fnf_chart = FnfDecoder::decode(&get_test_asset("fnf/test-song.json")).unwrap();
    group.bench_function("fnf", |b| {
        b.iter(|| FnfEncoder::encode(&fnf_chart).unwrap())
    });

    let mc_chart = OsuDecoder::decode(&get_test_asset("osu/mania_4k.osu")).unwrap();
    group.bench_function("mc/4K", |b| {
        b.iter(|| McEncoder::encode(&mc_chart).unwrap())
    });

    let jrox_chart = OsuDecoder::decode(&get_test_asset("osu/mania_4k.osu")).unwrap();
    group.bench_function("jrox", |b| {
        b.iter(|| JroxEncoder::encode(&jrox_chart).unwrap())
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Roundtrip benchmarks (decode → encode → decode)
// ---------------------------------------------------------------------------

fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("roundtrip");

    let osu_data = get_test_asset("osu/mania_4k.osu");
    group.bench_function("osu/4K", |b| {
        b.iter(|| {
            let chart = OsuDecoder::decode(&osu_data).unwrap();
            let encoded = OsuEncoder::encode(&chart).unwrap();
            OsuDecoder::decode(&encoded).unwrap()
        })
    });

    let sm_data = get_test_asset("stepmania/4k.sm");
    group.bench_function("sm/4K", |b| {
        b.iter(|| {
            let chart = SmDecoder::decode(&sm_data).unwrap();
            let encoded = SmEncoder::encode(&chart).unwrap();
            SmDecoder::decode(&encoded).unwrap()
        })
    });

    let fnf_data = get_test_asset("fnf/test-song.json");
    group.bench_function("fnf", |b| {
        b.iter(|| {
            let chart = FnfDecoder::decode(&fnf_data).unwrap();
            let encoded = FnfEncoder::encode(&chart).unwrap();
            FnfDecoder::decode(&encoded).unwrap()
        })
    });

    let mc_source = OsuDecoder::decode(&get_test_asset("osu/mania_4k.osu")).unwrap();
    let mc_data = McEncoder::encode(&mc_source).unwrap();
    group.bench_function("mc/4K", |b| {
        b.iter(|| {
            let chart = McDecoder::decode(&mc_data).unwrap();
            let encoded = McEncoder::encode(&chart).unwrap();
            McDecoder::decode(&encoded).unwrap()
        })
    });

    let jrox_source = OsuDecoder::decode(&get_test_asset("osu/mania_4k.osu")).unwrap();
    let jrox_data = JroxEncoder::encode(&jrox_source).unwrap();
    group.bench_function("jrox", |b| {
        b.iter(|| {
            let chart = JroxDecoder::decode(&jrox_data).unwrap();
            let encoded = JroxEncoder::encode(&chart).unwrap();
            JroxDecoder::decode(&encoded).unwrap()
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Format comparison: same chart decoded by different formats
// ---------------------------------------------------------------------------

fn bench_format_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("format_comparison/decode");

    // Text formats on comparable 4K charts
    let osu_data = get_test_asset("osu/mania_4k.osu");
    let sm_data = get_test_asset("stepmania/4k.sm");
    let qua_data = get_test_asset("quaver/4K.qua");
    let fnf_data = get_test_asset("fnf/test-song.json");

    // Generated formats built from the osu chart
    let mc_source = OsuDecoder::decode(&osu_data).unwrap();
    let mc_data = McEncoder::encode(&mc_source).unwrap();

    // Binary format: jrox built from the osu chart
    let jrox_source = OsuDecoder::decode(&osu_data).unwrap();
    let jrox_data = JroxEncoder::encode(&jrox_source).unwrap();

    group.bench_with_input(BenchmarkId::new("osu", "4K"), &osu_data, |b, d| {
        b.iter(|| OsuDecoder::decode(d).unwrap())
    });
    group.bench_with_input(BenchmarkId::new("sm", "4K"), &sm_data, |b, d| {
        b.iter(|| SmDecoder::decode(d).unwrap())
    });
    group.bench_with_input(BenchmarkId::new("qua", "4K"), &qua_data, |b, d| {
        b.iter(|| QuaDecoder::decode(d).unwrap())
    });
    group.bench_with_input(BenchmarkId::new("fnf", "4K"), &fnf_data, |b, d| {
        b.iter(|| FnfDecoder::decode(d).unwrap())
    });
    group.bench_with_input(BenchmarkId::new("mc", "4K"), &mc_data, |b, d| {
        b.iter(|| McDecoder::decode(d).unwrap())
    });
    group.bench_with_input(BenchmarkId::new("jrox", "4K"), &jrox_data, |b, d| {
        b.iter(|| JroxDecoder::decode(d).unwrap())
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Heavy chart (50K notes) — decode comparison across formats
// ---------------------------------------------------------------------------

fn bench_heavy(c: &mut Criterion) {
    let osu_50k_data = get_test_asset("osu/mania_4K_50K_notes.osu");
    let opts = OsuDecodeOptions {
        re_arrange_bpm: true,
    };
    let chart_50k =
        OsuDecoder::decode_with_options(&osu_50k_data, &opts).expect("50K decode failed");

    let jrox_50k = JroxEncoder::encode(&chart_50k).expect("jrox encode failed");
    let rox_50k = RoxNativeCodec::encode(&chart_50k).expect("rox encode failed");

    eprintln!(
        "\n[heavy/50K] sizes — osu: {}KB  jrox: {}KB  rox(zstd): {}KB",
        osu_50k_data.len() / 1024,
        jrox_50k.len() / 1024,
        rox_50k.len() / 1024,
    );

    let mut group = c.benchmark_group("heavy/50K_notes");

    group.bench_function("osu/decode", |b| {
        b.iter(|| OsuDecoder::decode_with_options(&osu_50k_data, &opts).unwrap())
    });
    group.bench_function("jrox/decode", |b| {
        b.iter(|| JroxDecoder::decode(&jrox_50k).unwrap())
    });
    group.bench_function("rox/decode", |b| {
        b.iter(|| RoxNativeCodec::decode(&rox_50k).unwrap())
    });
    group.bench_function("osu/encode", |b| {
        b.iter(|| OsuEncoder::encode(&chart_50k).unwrap())
    });
    group.bench_function("jrox/encode", |b| {
        b.iter(|| JroxEncoder::encode(&chart_50k).unwrap())
    });
    group.bench_function("rox/encode", |b| {
        b.iter(|| RoxNativeCodec::encode(&chart_50k).unwrap())
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_decode,
    bench_encode,
    bench_roundtrip,
    bench_format_comparison,
    bench_heavy,
);
criterion_main!(benches);
