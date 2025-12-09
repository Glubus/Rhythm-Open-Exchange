//! Tests for Taiko column layouts and alternation logic.

use rhythm_open_exchange::codec::formats::taiko::types::{AlternationState, ColumnLayout};

#[test]
fn test_dkkd_layout() {
    let mut state = AlternationState::new(ColumnLayout::Dkkd);

    // DKKD Pattern:
    // Don columns: 0, 3 (outer)
    // Kat columns: 1, 2 (inner)

    // Test Dons (should alternate 0 -> 3)
    assert_eq!(state.next_don_columns(false), vec![0]);
    assert_eq!(state.next_don_columns(false), vec![3]);
    assert_eq!(state.next_don_columns(false), vec![0]);

    // Test Kats (should alternate 1 -> 2)
    assert_eq!(state.next_kat_columns(false), vec![1]);
    assert_eq!(state.next_kat_columns(false), vec![2]);
    assert_eq!(state.next_kat_columns(false), vec![1]);

    // Test Big Don (should be both 0 and 3)
    assert_eq!(state.next_don_columns(true), vec![0, 3]);

    // Test Big Kat (should be both 1 and 2)
    assert_eq!(state.next_kat_columns(true), vec![1, 2]);
}

#[test]
fn test_dkdk_layout() {
    let mut state = AlternationState::new(ColumnLayout::Dkdk);

    // DKDK Pattern:
    // Don columns: 0, 2 (even)
    // Kat columns: 1, 3 (odd)

    // Test Dons (should alternate 0 -> 2)
    assert_eq!(state.next_don_columns(false), vec![0]);
    assert_eq!(state.next_don_columns(false), vec![2]);

    // Test Kats (should alternate 1 -> 3)
    assert_eq!(state.next_kat_columns(false), vec![1]);
    assert_eq!(state.next_kat_columns(false), vec![3]);
}

#[test]
fn test_kddk_layout() {
    let mut state = AlternationState::new(ColumnLayout::Kddk);

    // KDDK Pattern:
    // Don columns: 1, 2 (inner)
    // Kat columns: 0, 3 (outer)

    // Test Dons (should alternate 1 -> 2)
    assert_eq!(state.next_don_columns(false), vec![1]);
    assert_eq!(state.next_don_columns(false), vec![2]);

    // Test Kats (should alternate 0 -> 3)
    assert_eq!(state.next_kat_columns(false), vec![0]);
    assert_eq!(state.next_kat_columns(false), vec![3]);
}
