# Hitsounds

Hitsounds are per-note audio samples (keysounds). They are used by formats like osu!mania custom samples.

## Structure

```rust
pub struct Hitsound {
    pub file: CompactString,       // relative path to audio sample
    pub volume: Option<u8>,        // 0–100, None = use default
}
```

Volume is clamped to 100 on construction — `Hitsound::with_volume("kick.wav", 150)` stores `Some(100)`.

## Constructors

```rust
Hitsound::new("kick.wav")                  // no volume override
Hitsound::with_volume("snare.wav", 80)     // volume = 80
```

## Usage in Charts

Notes reference hitsounds by index into `RoxChart.hitsounds`:

```rust
let hs_idx = chart.hitsounds.len() as u16;
chart.hitsounds.push(Hitsound::new("kick.wav"));
note.hitsound_index = Some(hs_idx);
```

The index is `Option<u16>`, allowing up to 65535 unique samples per chart.

## Format Support

| Format | Keysound Support |
|--------|-----------------|
| osu!mania | Custom sample per hit object (`extras` field) |
| Quaver | Custom audio per hit object |
| StepMania | No per-note keysounds |
| FNF | No per-note keysounds |
| Taiko | No per-note keysounds |
| ROX native | Full — stored in `hitsounds` array |
