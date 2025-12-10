//! ROX CLI - Chart format conversion tool.
//!
//! Usage:
//!   rox convert <input> <output>
//!   rox info <file>
//!   rox validate <file>
//!
//! Examples:
//!   rox convert song.osu song.qua
//!   rox convert chart.json output.osu
//!   rox info chart.rox

use std::path::PathBuf;
use std::process::ExitCode;

use rhythm_open_exchange::codec::{auto_decode, auto_encode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_help();
        return ExitCode::from(1);
    }

    match args[1].as_str() {
        "convert" => cmd_convert(&args[2..]),
        "info" => cmd_info(&args[2..]),
        "validate" => cmd_validate(&args[2..]),
        "help" | "-h" | "--help" => {
            print_help();
            ExitCode::SUCCESS
        }
        "version" | "-V" | "--version" => {
            println!("rox {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_help();
            ExitCode::from(1)
        }
    }
}

fn print_help() {
    println!(
        r#"ROX - Rhythm Open Exchange CLI

USAGE:
    rox <COMMAND> [OPTIONS]

COMMANDS:
    convert <input> <output>   Convert between chart formats
    info <file>                Display chart information
    validate <file>            Validate a chart file
    help                       Show this help message
    version                    Show version

SUPPORTED FORMATS:
    .rox   - ROX binary format
    .osu   - osu!mania
    .sm    - StepMania
    .qua   - Quaver
    .json  - Friday Night Funkin'

EXAMPLES:
    rox convert song.osu song.qua
    rox convert chart.json output.sm
    rox info chart.rox
    rox validate song.osu
"#
    );
}

fn cmd_convert(args: &[String]) -> ExitCode {
    if args.len() < 2 {
        eprintln!("Usage: rox convert <input> <output>");
        return ExitCode::from(1);
    }

    let input = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);

    println!("Converting: {} -> {}", input.display(), output.display());

    // Decode input
    let chart = match auto_decode(&input) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error decoding {}: {}", input.display(), e);
            return ExitCode::from(1);
        }
    };

    println!(
        "  Loaded: {} - {} [{} notes, {}K]",
        chart.metadata.artist,
        chart.metadata.title,
        chart.notes.len(),
        chart.key_count
    );

    // Encode output
    if let Err(e) = auto_encode(&chart, &output) {
        eprintln!("Error encoding {}: {}", output.display(), e);
        return ExitCode::from(1);
    }

    println!("  ✓ Saved to: {}", output.display());
    ExitCode::SUCCESS
}

fn cmd_info(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("Usage: rox info <file>");
        return ExitCode::from(1);
    }

    let path = PathBuf::from(&args[0]);

    let chart = match auto_decode(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            return ExitCode::from(1);
        }
    };

    println!("File: {}", path.display());
    println!();
    println!("=== Metadata ===");
    println!("  Title:      {}", chart.metadata.title);
    println!("  Artist:     {}", chart.metadata.artist);
    println!("  Creator:    {}", chart.metadata.creator);
    println!("  Difficulty: {}", chart.metadata.difficulty_name);
    if let Some(val) = chart.metadata.difficulty_value {
        println!("  Level:      {:.2}", val);
    }
    println!("  Audio:      {}", chart.metadata.audio_file);
    if chart.metadata.is_coop {
        println!(
            "  Mode:       {}K Coop ({}K + {}K)",
            chart.key_count,
            chart.key_count / 2,
            chart.key_count / 2
        );
    } else {
        println!("  Mode:       {}K", chart.key_count);
    }
    println!();
    println!("=== Statistics ===");
    println!("  Notes:         {}", chart.notes.len());
    println!("  Timing Points: {}", chart.timing_points.len());
    println!("  Hitsounds:     {}", chart.hitsounds.len());
    #[allow(clippy::cast_precision_loss)]
    let duration_s = chart.duration_us() as f64 / 1_000_000.0;
    println!("  Duration:      {:.1}s", duration_s);
    println!("  Hash:          {}", chart.short_hash());

    ExitCode::SUCCESS
}

fn cmd_validate(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("Usage: rox validate <file>");
        return ExitCode::from(1);
    }

    let path = PathBuf::from(&args[0]);

    let chart = match auto_decode(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error loading: {}", e);
            return ExitCode::from(1);
        }
    };

    match chart.validate() {
        Ok(()) => {
            println!("✓ {} is valid", path.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("✗ {} validation failed: {}", path.display(), e);
            ExitCode::from(1)
        }
    }
}
