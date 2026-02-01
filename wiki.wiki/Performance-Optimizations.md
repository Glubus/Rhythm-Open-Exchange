# Performance Optimizations

This document details the optimization techniques applied to the Rhythm Open Exchange (ROX) project to achieve high-performance parsing and serialization.

## 1. `.osu` Parser Optimization

Targeting the loading speed of large `.osu` maps (50k+ notes).

### Techniques

1.  **Zero-Copy I/O (`memmap2`)**:
    -   Mapped file content directly into memory instead of reading into a heap-allocated buffer.
    -   Reduced initial load latency and memory pressure.

2.  **SIMD Line Iteration (`memchr`)**:
    -   Used AVX2/SSE2 instructions to find newline bytes.
    -   Replaced standard `str::lines()` iterator.

3.  **SIMD Integer Parsing (`atoi`)**:
    -   Parsed integers directly from byte slices without UTF-8 validation redundancy.
    -   Massive speedup for `[HitObjects]` section.

4.  **Vector Pre-allocation**:
    -   Estimated vector capacity based on file size (~40 bytes per object).
    -   Eliminated costly reallocations during parsing.

### Results (50k Notes)

-   **Load Time**: Reduced from **14.6ms** to **6.2ms** (~58% faster).
-   **Allocation Count**: Drastically reduced.

## 2. ROX Format Optimization

Targeting the binary serialization and deserialization speed.

### Techniques

1.  **Compact String (`compact_str`)**:
    -   Replaced `String` with `CompactString` for Metadata and Hitsounds.
    -   Small strings (<24 bytes) are stored inline, avoiding heap allocations.

2.  **Data Layout Optimization**:
    -   Reordered `Note` struct fields to minimize padding.
    -   Reduced `Note` size from **40 bytes** to **32 bytes** (20% reduction).
    -   Improved cache locality.

### Results (50k Notes)

-   **File Size**: **66.7 KB** (vs 1.55 MB for .osu)
-   **Decode Speed**: **~0.82 ms**
-   **Encode Speed**: **~1.69 ms**
-   **Throughput**: **~61 Million Notes/sec**

## 3. General Best Practices

-   **Strict Limits**: Enforced `MAX_FILE_SIZE` (100MB) across all parsers to prevent DoS.
-   **Observability**: Added `tracing` for detailed performance breakdown and error logging.
