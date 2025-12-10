/**
 * Example: Basic usage of the ROX WASM bindings with Node.js.
 * 
 * Build first:
 *   cd bindings/wasm
 *   wasm-pack build --target nodejs
 * 
 * Then run:
 *   node examples/basic.mjs
 */

import { readFileSync, writeFileSync } from 'fs';
import { decode, encode, convert, version } from '../pkg/rox_wasm.js';

console.log(`ROX WASM v${version()}`);
console.log("");

// Decode a Quaver chart
console.log("=== Decoding Quaver chart ===");
const quaData = readFileSync("../../assets/quaver/4K.qua");
const chart = decode(quaData, "qua");

console.log(`Title:      ${chart.title}`);
console.log(`Artist:     ${chart.artist}`);
console.log(`Creator:    ${chart.creator}`);
console.log(`Difficulty: ${chart.difficulty}`);
console.log(`Keys:       ${chart.key_count}K`);
console.log(`Notes:      ${chart.note_count}`);
console.log(`Duration:   ${chart.duration.toFixed(1)}s`);
console.log(`Hash:       ${chart.hash}`);
console.log(`Coop:       ${chart.is_coop}`);
console.log("");

// Encode to osu! format
console.log("=== Converting to osu! ===");
const osuBytes = encode(chart, "osu");
writeFileSync("output_from_node.osu", Buffer.from(osuBytes));
console.log("Saved: output_from_node.osu");
console.log("");

// Decode FNF chart
console.log("=== Decoding FNF chart ===");
const fnfData = readFileSync("../../assets/fnf/test-song.json");
const fnfChart = decode(fnfData, "json");
console.log(`Title: ${fnfChart.title}, ${fnfChart.key_count}K, ${fnfChart.note_count} notes`);
console.log("");

// Direct conversion
console.log("=== Direct conversion ===");
const qua7kData = readFileSync("../../assets/quaver/7K.qua");
const smBytes = convert(qua7kData, "qua", "sm");
writeFileSync("output_7k.sm", Buffer.from(smBytes));
console.log("Converted 7K.qua -> output_7k.sm");
