use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rox::codec::{Decoder, Encoder};
use rox_formats::{
    FnfDecoder, FnfEncoder, JroxDecoder, JroxEncoder, OsuDecoder, OsuEncoder, QuaDecoder,
    QuaEncoder, SmDecoder, SmEncoder, TaikoDecoder,
};
use rox_test_utils::get_test_asset;

// ---------------------------------------------------------------------------
// Decode benchmarks
// ---------------------------------------------------------------------------

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode");

    let osu_4k = get_test_asset("osu/mania_4k.osu");
    group.bench_function("osu/4K", |b| b.iter(|| OsuDecoder::decode(&osu_4k).unwrap()));

    let osu_7k = get_test_asset("osu/mania_7k.osu");
    group.bench_function("osu/7K", |b| b.iter(|| OsuDecoder::decode(&osu_7k).unwrap()));

    let taiko = get_test_asset("osu/taiko.osu");
    group.bench_function("taiko", |b| b.iter(|| TaikoDecoder::decode(&taiko).unwrap()));

    let sm = get_test_asset("stepmania/4k.sm");
    group.bench_function("sm/4K", |b| b.iter(|| SmDecoder::decode(&sm).unwrap()));

    let qua_4k = get_test_asset("quaver/4K.qua");
    group.bench_function("qua/4K", |b| b.iter(|| QuaDecoder::decode(&qua_4k).unwrap()));

    let qua_7k = get_test_asset("quaver/7K.qua");
    group.bench_function("qua/7K", |b| b.iter(|| QuaDecoder::decode(&qua_7k).unwrap()));

    let fnf = get_test_asset("fnf/test-song.json");
    group.bench_function("fnf", |b| b.iter(|| FnfDecoder::decode(&fnf).unwrap()));

    // JROX: binary format — benchmark via a pre-encoded chart
    let jrox_source = OsuDecoder::decode(&get_test_asset("osu/mania_4k.osu")).unwrap();
    let jrox_data = JroxEncoder::encode(&jrox_source).unwrap();
    group.bench_function("jrox", |b| b.iter(|| JroxDecoder::decode(&jrox_data).unwrap()));

    group.finish();
}

// ---------------------------------------------------------------------------
// Encode benchmarks
// ---------------------------------------------------------------------------

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode");

    let osu_chart = OsuDecoder::decode(&get_test_asset("osu/mania_4k.osu")).unwrap();
    group.bench_function("osu/4K", |b| b.iter(|| OsuEncoder::encode(&osu_chart).unwrap()));

    let sm_chart = SmDecoder::decode(&get_test_asset("stepmania/4k.sm")).unwrap();
    group.bench_function("sm/4K", |b| b.iter(|| SmEncoder::encode(&sm_chart).unwrap()));

    let qua_chart = QuaDecoder::decode(&get_test_asset("quaver/4K.qua")).unwrap();
    group.bench_function("qua/4K", |b| b.iter(|| QuaEncoder::encode(&qua_chart).unwrap()));

    let fnf_chart = FnfDecoder::decode(&get_test_asset("fnf/test-song.json")).unwrap();
    group.bench_function("fnf", |b| b.iter(|| FnfEncoder::encode(&fnf_chart).unwrap()));

    let jrox_chart = OsuDecoder::decode(&get_test_asset("osu/mania_4k.osu")).unwrap();
    group.bench_function("jrox", |b| b.iter(|| JroxEncoder::encode(&jrox_chart).unwrap()));

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
    group.bench_with_input(BenchmarkId::new("jrox", "4K"), &jrox_data, |b, d| {
        b.iter(|| JroxDecoder::decode(d).unwrap())
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_decode,
    bench_encode,
    bench_roundtrip,
    bench_format_comparison,
);
criterion_main!(benches);
