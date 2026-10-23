# Performance Optimizations

## Benchmarks (v0.7.0)

Measured with Criterion on a 50K-note osu!mania chart (approx. real-world stress test):

| Operation | Format | Time | File Size |
|-----------|--------|------|-----------|
| Decode | osu!mania text | ~170ms | ~3MB |
| Decode | ROX native (rkyv+zstd) | ~21ms | ~133KB |
| Encode | osu!mania text | — | — |
| Encode | ROX native | ~9ms | — |

ROX native is **~8× faster** to decode and **~23× smaller** than osu! text for a 50K-note chart.

## Key Techniques

### rkyv Zero-Copy Serialization

`RoxChart` and all model types derive `rkyv::Archive`. Deserialization is zero-copy — no allocations for fields when reading. This is why ROX native decode is so fast.

```rust
// SAFETY: data was produced by RoxNativeCodec::encode_inner
let chart = unsafe { rkyv::from_bytes_unchecked::<RoxChart, RkyvError>(&decompressed) }?;
```

### zstd Compression (Level 3)

Level 3 balances speed and ratio. The ROX file is `ROX\0` magic + zstd-compressed rkyv bytes.

### Delta Timestamp Encoding

Notes are stored as delta timestamps instead of absolute values before compression. This creates long runs of small integers that zstd compresses very efficiently.

```
absolute:  [1000, 2000, 3000, 3100, 3200]
delta:     [1000,  999, 1000,  100,  100]  ← better for compression
```

### CompactString

All string fields in `Metadata` use `compact_str::CompactString` — strings ≤ 24 bytes are stored inline with no heap allocation. Most metadata fields (title, artist, etc.) benefit from this.

## Running Benchmarks

```bash
cargo bench -p rox-formats
```

Results are written to `target/criterion/` as HTML reports.

## Benchmark Groups

| Group | Description |
|-------|-------------|
| `decode` | All formats, standard assets |
| `encode` | All formats, standard assets |
| `roundtrip` | decode → encode → decode |
| `format_comparison/decode` | Side-by-side on equivalent 4K charts |
| `heavy/50K_notes` | Decode + encode on 50K-note chart |
