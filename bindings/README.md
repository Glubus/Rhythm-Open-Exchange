# ROX Bindings

Language bindings for the Rhythm Open Exchange library.

## Available Bindings

### Python (`ffi/python/`) ✅ Working

Python bindings generated via UniFFI.

```bash
cd bindings/ffi/python
python -m venv .venv
source .venv/bin/activate  # Or activate depending on OS
pip install maturin
maturin develop
```

```python
import rox

chart = rox.decode("song.osu")
print(f"{chart.title} by {chart.artist}")
rox.encode(chart, "output.qua")
```

### JavaScript/WASM (`wasm/`) ✅ Working

WebAssembly bindings using [wasm-bindgen](https://rustwasm.github.io/).

### C# / .NET (`ffi/csharp/`) ✅ Working

C# bindings generated via UniFFI.

> **Note**: WASM uses SHA256 instead of BLAKE3 and no compression (pure Rust).

```bash
cd bindings/wasm
wasm-pack build --target nodejs  # or --target web
```

```javascript
import { decode, encode, version } from './pkg/rox_wasm.js';

const chart = decode(fileBytes, 'osu');
console.log(`${chart.title} by ${chart.artist}`);
const output = encode(chart, 'qua');
```

## Building

See individual binding directories for build instructions and examples.
