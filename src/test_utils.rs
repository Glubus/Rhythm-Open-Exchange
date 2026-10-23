use std::path::PathBuf;

/// Returns the raw bytes of a test asset file.
///
/// Assets live at the workspace root under `assets/`.
/// Pass a relative path like `"osu/mania_7k.osu"`.
///
/// # Panics
///
/// Panics if the file cannot be read.
pub fn get_test_asset(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read asset '{name}': {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("osu/mania_7k.osu")]
    #[case("stepmania/4k.sm")]
    #[case("quaver/4K.qua")]
    #[case("fnf/test-song.json")]
    fn test_get_test_asset_returns_non_empty(#[case] name: &str) {
        let data = get_test_asset(name);
        assert!(!data.is_empty(), "asset '{name}' should not be empty");
    }
}
