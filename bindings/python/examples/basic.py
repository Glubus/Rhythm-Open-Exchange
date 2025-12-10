#!/usr/bin/env python3
"""
Example: Basic usage of the ROX Python bindings.

Run from the bindings/python directory:
    .venv/Scripts/python examples/basic.py
"""

import rox

def main():
    # Decode a Quaver chart
    print("=== Decoding Quaver chart ===")
    chart = rox.decode("../../assets/quaver/4K.qua")
    
    print(f"Title:      {chart.title}")
    print(f"Artist:     {chart.artist}")
    print(f"Creator:    {chart.creator}")
    print(f"Difficulty: {chart.difficulty}")
    print(f"Keys:       {chart.key_count}K")
    print(f"Notes:      {chart.note_count}")
    print(f"Duration:   {chart.duration:.1f}s")
    print(f"Hash:       {chart.hash}")
    print(f"Coop:       {chart.is_coop}")
    print()
    
    # Convert to osu! format
    print("=== Converting to osu! ===")
    rox.encode(chart, "output_from_python.osu")
    print("Saved: output_from_python.osu")
    print()
    
    # Decode FNF chart
    print("=== Decoding FNF chart ===")
    fnf_chart = rox.decode("../../assets/fnf/test-song.json")
    print(f"Title: {fnf_chart.title}, {fnf_chart.key_count}K, {fnf_chart.note_count} notes")
    print()
    
    # Direct conversion
    print("=== Direct conversion ===")
    rox.convert("../../assets/quaver/7K.qua", "output_7k.osu")
    print("Converted 7K.qua -> output_7k.osu")

if __name__ == "__main__":
    main()
