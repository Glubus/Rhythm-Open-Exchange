# ROX Python Bindings

Python bindings for Rhythm Open Exchange using [PyO3](https://pyo3.rs/).

## Installation

### From source (development)

```bash
# Install maturin
pip install maturin

# Build and install in development mode
cd bindings/python
maturin develop
```

### Build wheel

```bash
maturin build --release
pip install target/wheels/rox-*.whl
```

## Usage

```python
import rox

# Decode a chart file
chart = rox.decode("song.osu")
print(f"{chart.artist} - {chart.title}")
print(f"Keys: {chart.key_count}K")
print(f"Notes: {chart.note_count}")
print(f"Duration: {chart.duration:.1f}s")

# Encode to a different format
rox.encode(chart, "output.qua")

# Convert directly
rox.convert("input.osu", "output.sm")

# Decode from bytes
with open("chart.osu", "rb") as f:
    data = f.read()
chart = rox.decode_bytes(data, "osu")

# Encode to bytes
osu_bytes = rox.encode_bytes(chart, "osu")
```

## API Reference

### Functions

- `decode(path: str) -> Chart` - Decode chart from file path
- `decode_bytes(data: bytes, format: str) -> Chart` - Decode chart from bytes
- `encode(chart: Chart, path: str)` - Encode chart to file
- `encode_bytes(chart: Chart, format: str) -> bytes` - Encode chart to bytes
- `convert(input: str, output: str)` - Convert between formats

### Chart Properties

- `title: str` - Song title
- `artist: str` - Song artist
- `creator: str` - Chart creator
- `difficulty: str` - Difficulty name
- `key_count: int` - Number of keys (4, 7, etc.)
- `note_count: int` - Number of notes
- `duration: float` - Duration in seconds
- `is_coop: bool` - Whether it's a coop chart
- `hash: str` - Short hash of the chart

### Supported Formats

- `rox` - ROX binary format
- `osu` - osu!mania
- `sm` - StepMania
- `qua` - Quaver
- `json` / `fnf` - Friday Night Funkin'
