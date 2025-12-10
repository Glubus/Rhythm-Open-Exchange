# ROX Bindings

Language bindings for the Rhythm Open Exchange library.

## Available Bindings

### Python (`python/`)

Python bindings using [PyO3](https://pyo3.rs/) and [maturin](https://www.maturin.rs/).

```bash
cd bindings/python
maturin develop
```

```python
import rox

chart = rox.decode("song.osu")
print(f"{chart.title} by {chart.artist}")
rox.encode(chart, "output.qua")
```

### JavaScript/WASM (`wasm/`)

WebAssembly bindings using [wasm-bindgen](https://rustwasm.github.io/).

```bash
cd bindings/wasm
wasm-pack build --target web
```

```javascript
import init, { decode, encode } from './pkg/rox_wasm.js';

await init();
const chart = decode(fileBytes);
console.log(`${chart.title} by ${chart.artist}`);
```

## Building

See individual binding directories for build instructions.
