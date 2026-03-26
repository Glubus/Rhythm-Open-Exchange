pub struct OsuEncoder;

/// Convert a column index to an x-coordinate for the given key count.
#[must_use]
pub fn column_to_x(column: u8, key_count: u8) -> i32 {
    if key_count == 0 {
        return 0;
    }
    (i32::from(column) * 512 + 256) / i32::from(key_count)
}
