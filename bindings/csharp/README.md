# RhythmOpenExchange

C# bindings for [rhythm-open-exchange](https://github.com/Glubus/rhythm-open-exchange) - A universal format converter for VSRG (Vertical Scrolling Rhythm Games).

## Supported Formats

| Format | Extension | Read | Write |
|--------|-----------|------|-------|
| ROX (native binary) | `.rox` | ✅ | ✅ |
| osu!mania | `.osu` | ✅ | ✅ |
| StepMania | `.sm` | ✅ | ✅ |
| Quaver | `.qua` | ✅ | ✅ |
| Friday Night Funkin' | `.json` | ✅ | ✅ |

## Installation

```bash
dotnet add package RhythmOpenExchange
```

## Usage

```csharp
using RhythmOpenExchange;

// Read a chart file
byte[] osuData = File.ReadAllBytes("chart.osu");
using var chart = RoxChart.FromBytes(osuData);

if (chart != null)
{
    Console.WriteLine($"Title: {chart.Title}");
    Console.WriteLine($"Artist: {chart.Artist}");
    Console.WriteLine($"Keys: {chart.KeyCount}K");
    Console.WriteLine($"Notes: {chart.NoteCount}");

    // Convert to StepMania format
    string? smContent = chart.ToString(RoxFormat.Sm);
    if (smContent != null)
    {
        File.WriteAllText("chart.sm", smContent);
    }
}
```

### Decode from string (text formats)

```csharp
string osuContent = File.ReadAllText("chart.osu");
using var chart = RoxChart.FromString(osuContent);
```

### Encode to binary format

```csharp
byte[]? roxData = chart.ToBytes(RoxFormat.Rox);
if (roxData != null)
{
    File.WriteAllBytes("chart.rox", roxData);
}
```

## Native Library

This package requires the native `rhythm_open_exchange` library:

- Windows: `rhythm_open_exchange.dll`
- Linux: `librhythm_open_exchange.so`
- macOS: `librhythm_open_exchange.dylib`

The native libraries are included in the NuGet package for common platforms.

## Building from Source

```bash
# Build the Rust library
cargo build --release

# The DLL/SO/DYLIB will be in target/release/
```

## License

MIT
