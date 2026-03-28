# rox-cli

> **Status: planned for v0.7.0 — not yet implemented.**

The `rox-cli` crate provides the `rox` binary for working with rhythm-game chart files from the command line. It requires the `std` feature and links against `rox-formats` and (optionally) `rox-analysis`.

---

## Commands

### `rox convert`

Convert a chart from one format to another. The input and output formats are inferred from file extensions.

```
rox convert <input> <output> [options]
```

| Flag | Description |
|------|-------------|
| `--re-arrange` | Enable `OsuDecodeOptions::re_arrange_bpm` when decoding `.osu` files. Shifts a misplaced first BPM point to satisfy validation. |

Examples:

```sh
# osu!mania → ROX native
rox convert chart.osu chart.rox

# StepMania → Quaver
rox convert chart.sm chart.qua

# osu with misaligned BPM
rox convert chart.osu chart.rox --re-arrange
```

---

### `rox info`

Print metadata and statistics about a chart file.

```
rox info <file> [options]
```

| Flag | Description |
|------|-------------|
| `--advanced` | Include full pattern analysis output (requires `rox-analysis` feature) |

Default output includes:

- Title, artist, creator, difficulty name
- Key count, `is_coop`
- Duration, note count, BPM range and mode
- NPS average
- Audio and background file paths

With `--advanced`, also includes:

- NPS density (16 segments)
- Polyphony breakdown
- Lane balance per column
- Stream/jack/roll detection summary
- `AnalysisResult` report

Examples:

```sh
rox info chart.rox
rox info chart.osu --advanced
```

---

### `rox validate`

Validate a chart against all ROX validation rules without decoding to a new format.

```
rox validate <file>
```

Exits with code 0 if valid, non-zero otherwise. Prints each violated rule to stderr.

Example:

```sh
rox validate chart.osu
# Output on failure:
# ERROR: BpmAfterFirstNote — first BPM timing point is after the first note
# Validation failed (1 error)
```

---

## Feature flags

| Feature | Effect |
|---------|--------|
| `analysis` | Links `rox-analysis`; enables `--advanced` flag on `rox info` |

Build without analysis (smaller binary):

```sh
cargo build -p rox-cli --no-default-features --features std
```

Build with analysis:

```sh
cargo build -p rox-cli --features std,analysis
```
