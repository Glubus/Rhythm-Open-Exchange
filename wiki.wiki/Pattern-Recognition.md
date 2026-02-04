# Pattern Recognition

## Overview

ROX implements advanced pattern recognition logic directly ported from the [Quattern](https://github.com/Leinadix/Quattern) library (originally C#). This allows for VSRG-centric analysis of charts, identifying patterns such as streams, jacks, jumps, and technical hybrids.

## Technical Decisions

### 1. Porting Quattern

**Decision**: We chose to port Quattern logic to Rust rather than using FFI or creating a new algorithm from scratch.

**Why**:
- **Proven Accuracy**: Quattern is battle-tested in the osu!mania community and provides results that players trust.
- **Performance**: A native Rust port avoids the overhead of managing a .NET runtime side-by-side with Rust, especially for WASM targets.
- **Ownership**: Having the logic in Rust allows us to expose it cleanly to all our bindings (Python, C#, WASM) via UniFFI.

### 2. QuadTree Architecture

**Decision**: The core analysis uses a QuadTree structure to represent note distribution over time and columns.

**Why**:
- **Spatial Efficiency**: Rhythm game charts are "sparse matrices". A QuadTree allows us to skip empty regions efficiently.
- **Recursive Merging**: Patterns are self-similar. A "stream" is composed of smaller trills. A QuadTree allows us to define merge rules that propagate classifications from the leaves (2x2 atomics) up to the root.

### 3. Classification Taxonomy

**Decision**: adhering to standard VSRG terminology (Jumpstream, Handstream, Chordjack).

**Why**: To ensure the output is meaningful to the end-users (players and mappers) without requiring translation.


### 4. JSON Output Standardization

**Decision**: The analysis output is flattened into a linear timeline of pattern entries, discarding the hierarchical tree structure in the final output.

**Why**: 
- **Consumer Simplicity**: Frontends and API consumers expect a list of events ("Stream from X to Y") rather than a complex QuadTree.
- **Interoperability**: Matches the expected format for pattern density graphs and UI visualization.
- **Fields**: Each entry includes `start_time`, `end_time`, `duration`, `pattern_type`, `avg/min/max_bpm`, and `note_count`.

## Usage

### CLI
You can run the analysis via the CLI using the `-aa` flag:
```bash
rox info chart.osu -aa
```

### Rust API
```rust
use rhythm_open_exchange::analysis::RoxAnalysis;

let result = chart.pattern_analysis();
for entry in result.timeline.entries {
    println!("{}: {}", entry.time_start_us, entry.pattern);
}
```
