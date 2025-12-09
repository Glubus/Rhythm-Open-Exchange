# Hitsounds

Hitsounds (also called keysounds) are audio samples that play when notes are hit. This feature is essential for BMS and O2Jam style charts where each note plays a unique sound.

## Structure

```rust
pub struct Hitsound {
    pub file: String,
    pub volume: Option<u8>,
}
```

## Fields

| Field | Type | Description |
|-------|------|-------------|
| `file` | `String` | Relative path to the audio sample |
| `volume` | `Option<u8>` | Volume override (0-100, None = default) |

## Creating Hitsounds

### Default Volume

```rust
use rhythm_open_exchange::Hitsound;

let sound = Hitsound::new("sounds/kick.wav");

assert_eq!(sound.file, "sounds/kick.wav");
assert!(sound.volume.is_none());
```

### Custom Volume

```rust
let quiet = Hitsound::with_volume("sounds/hihat.wav", 60);
let loud = Hitsound::with_volume("sounds/crash.wav", 100);

assert_eq!(quiet.volume, Some(60));
```

### Volume Clamping

Volume is automatically clamped to 100:

```rust
let clamped = Hitsound::with_volume("loud.wav", 150);
assert_eq!(clamped.volume, Some(100)); // Clamped to 100
```

## Linking Notes to Hitsounds

Notes reference hitsounds by index:

```rust
use rhythm_open_exchange::{RoxChart, Note, Hitsound};

let mut chart = RoxChart::new(7);

// Define hitsound pool
chart.hitsounds.push(Hitsound::new("kick.wav"));     // Index 0
chart.hitsounds.push(Hitsound::new("snare.wav"));    // Index 1
chart.hitsounds.push(Hitsound::new("hihat.wav"));    // Index 2
chart.hitsounds.push(Hitsound::new("piano_c4.wav")); // Index 3

// Create notes with hitsounds
let mut kick_note = Note::tap(0, 0);
kick_note.hitsound_index = Some(0);

let mut snare_note = Note::tap(500_000, 3);
snare_note.hitsound_index = Some(1);

let mut piano_note = Note::tap(750_000, 5);
piano_note.hitsound_index = Some(3);

// Note without hitsound (uses default sample)
let plain_note = Note::tap(1_000_000, 2);

chart.notes.push(kick_note);
chart.notes.push(snare_note);
chart.notes.push(piano_note);
chart.notes.push(plain_note);
```

## Supported Formats

Common audio formats for hitsounds:

| Format | Extension | Notes |
|--------|-----------|-------|
| WAV | `.wav` | Lossless, instant playback |
| OGG | `.ogg` | Good compression, standard for games |
| MP3 | `.mp3` | Common but may have latency |
| FLAC | `.flac` | Lossless with compression |

WAV is recommended for optimal latency.

## Use Cases

### BMS Charts

BMS (Be-Music Source) charts are fully keysounded:

```rust
// Each note plays a unique sample
for i in 0..100 {
    chart.hitsounds.push(Hitsound::new(format!("sample_{:03}.wav", i)));
}

// Notes reference samples
for (i, time) in note_times.iter().enumerate() {
    let mut note = Note::tap(*time, columns[i]);
    note.hitsound_index = Some(sample_indices[i]);
    chart.notes.push(note);
}
```

### Drum Patterns

```rust
// Set up drum kit
chart.hitsounds.push(Hitsound::new("drums/kick.wav"));
chart.hitsounds.push(Hitsound::with_volume("drums/snare.wav", 90));
chart.hitsounds.push(Hitsound::with_volume("drums/hihat_closed.wav", 60));
chart.hitsounds.push(Hitsound::with_volume("drums/hihat_open.wav", 70));

const KICK: u16 = 0;
const SNARE: u16 = 1;
const HIHAT_C: u16 = 2;
const HIHAT_O: u16 = 3;
```

### Piano/Melody

```rust
// Piano samples per octave
let notes = ["c", "cs", "d", "ds", "e", "f", "fs", "g", "gs", "a", "as", "b"];
for octave in 3..6 {
    for note in &notes {
        chart.hitsounds.push(Hitsound::new(format!("piano/{}_{}.wav", note, octave)));
    }
}
```

## Format Mapping

### BMS

BMS defines samples in the header:

```
#WAV01 kick.wav
#WAV02 snare.wav
#WAV03 hihat.wav
```

Then references them in the chart:

```
#00111:01020301
```

### O2Jam

O2Jam uses `.ojm` files containing all samples in a single archive.

### osu!mania

osu! uses:
- Skin hitsounds (default)
- Custom hitsounds per timing section
- Storyboard samples

Keysounds are not natively supported but can be simulated with storyboard.

## Best Practices

1. **Organize samples** in subdirectories:
   ```
   sounds/
   ├── drums/
   │   ├── kick.wav
   │   └── snare.wav
   ├── piano/
   │   ├── c4.wav
   │   └── d4.wav
   └── fx/
       └── laser.wav
   ```

2. **Use consistent sample rates** (44100 Hz recommended)

3. **Keep samples short** for percussion (< 500ms)

4. **Normalize volumes** across samples for consistency

5. **Use volume field** for dynamic control rather than different sample files
