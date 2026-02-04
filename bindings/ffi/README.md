# Rox FFI Bindings

This crate provides the core implementation of the Rhythm Open Exchange logic, exposed via [UniFFI](https://github.com/mozilla/uniffi-rs) to other languages (Python, C#, Swift, etc.).

## Architecture

The system works in two layers:

1.  **The Native Layer (Rust)**:
    *   This crate compiles into a standard "Shared Library":
        *   `librox_ffi.so` (Linux)
        *   `rox_ffi.dll` (Windows)
        *   `librox_ffi.dylib` (macOS)
    *   This binary contains all the optimized logic (parsing, analysis, algorithms).
    *   It exposes a C-compatible interface (C ABI).

2.  **The Adapter Layer (Target Language)**:
    *   For each language, `uniffi-bindgen` generates a **Wrapper File** (Source Code).
    *   **C#**: Generates `rox_ffi.cs`. It uses `[DllImport]` to load the native library and calls functions dynamically.
    *   **Python**: Generates `rox_ffi.py`. It uses `ctypes` to load the native library.
    *   **Swift**: Generates `rox_ffi.swift` and a C header.

## Build Flow (CI/CD)

1.  **Rust Build**: We compile the Rust code to native binaries for all platforms (Linux, Win, Mac).
2.  **Generate Wrappers**: We run `uniffi-bindgen` to create the `.cs`, `.py`, etc. from the Rust definition.
3.  **Package**:
    *   **NuGet (C#)**: We bundle the `.cs` wrapper (compiled into a .NET DLL) + the Native Binaries inside a standard `.nupkg`.
    *   **PyPI (Python)**: We bundle the `.py` wrapper + the Native Binary inside a standard Wheel.

## Runtime

When you use the library:
1.  Your code calls the Wrapper (e.g. `chart.nps()`).
2.  The Wrapper handles type conversion (C# String -> Rust String).
3.  The Wrapper calls the Native Library via FFI.
4.  The Native Library executes the logic and returns the result.
