# ROX WASM Bindings

WebAssembly bindings for Rhythm Open Exchange using [wasm-bindgen](https://rustwasm.github.io/wasm-bindgen/).

## Building

```bash
# Install wasm-pack
cargo install wasm-pack

# Build for web
cd bindings/wasm
wasm-pack build --target web

# Build for Node.js
wasm-pack build --target nodejs
```

## Usage (Web)

```html
<script type="module">
import init, { decode, encode, convert, version } from './pkg/rox_wasm.js';

async function main() {
    await init();
    
    console.log(`ROX WASM v${version()}`);
    
    // Decode from file input
    const fileInput = document.getElementById('file');
    fileInput.onchange = async (e) => {
        const file = e.target.files[0];
        const buffer = await file.arrayBuffer();
        const data = new Uint8Array(buffer);
        
        // Detect format from extension
        const ext = file.name.split('.').pop();
        const chart = decode(data, ext);
        
        console.log(`${chart.artist} - ${chart.title}`);
        console.log(`${chart.key_count}K, ${chart.note_count} notes`);
        
        // Convert to another format
        const osuBytes = encode(chart, 'osu');
        downloadFile('converted.osu', osuBytes);
    };
}

function downloadFile(name, data) {
    const blob = new Blob([data], { type: 'application/octet-stream' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = name;
    a.click();
}

main();
</script>
```

## Usage (Node.js)

```javascript
const { decode, encode, convert, version } = require('./pkg/rox_wasm.js');
const fs = require('fs');

console.log(`ROX WASM v${version()}`);

// Read and decode
const data = fs.readFileSync('song.osu');
const chart = decode(data, 'osu');

console.log(`${chart.artist} - ${chart.title}`);
console.log(`${chart.key_count}K, ${chart.note_count} notes`);

// Convert
const quaBytes = encode(chart, 'qua');
fs.writeFileSync('output.qua', Buffer.from(quaBytes));

// Or convert directly
const smBytes = convert(data, 'osu', 'sm');
fs.writeFileSync('output.sm', Buffer.from(smBytes));
```

## API Reference

### Functions

- `decode(data: Uint8Array, format: string): Chart`
- `encode(chart: Chart, format: string): Uint8Array`
- `convert(data: Uint8Array, fromFormat: string, toFormat: string): Uint8Array`
- `version(): string`

### Chart Properties

- `title: string`
- `artist: string`
- `creator: string`
- `difficulty: string`
- `key_count: number`
- `note_count: number`
- `duration: number` (seconds)
- `is_coop: boolean`
- `hash: string`
- `audio_file: string`

### Supported Formats

- `rox` - ROX binary format
- `osu` - osu!mania
- `sm` - StepMania  
- `qua` - Quaver
- `json` / `fnf` - Friday Night Funkin'
