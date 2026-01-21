use std::path::PathBuf;

/// Get the path to the `assets` directory.
pub fn get_test_assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
}

/// Read a test asset file from the `assets` directory.
///
/// # Panics
///
/// Panics if the file cannot be read.
pub fn get_test_asset(path: &str) -> Vec<u8> {
    let path = get_test_assets_dir().join(path);
    std::fs::read(&path).unwrap_or_else(|e| panic!("Failed to read test asset {:?}: {}", path, e))
}
