use crate::model::RoxChart;

/// Compute BLAKE3 hash of the chart (full content).
pub fn hash(chart: &RoxChart) -> String {
    let encoded = rkyv::to_bytes::<rkyv::rancor::Error>(chart).unwrap_or_default();
    blake3::hash(&encoded).to_hex().to_string()
}

/// Compute BLAKE3 hash of the chart's notes only.
pub fn notes_hash(chart: &RoxChart) -> String {
    let encoded = rkyv::to_bytes::<rkyv::rancor::Error>(&chart.notes).unwrap_or_default();
    blake3::hash(&encoded).to_hex().to_string()
}

/// Compute BLAKE3 hash of the chart's timing points only.
pub fn timings_hash(chart: &RoxChart) -> String {
    let encoded = rkyv::to_bytes::<rkyv::rancor::Error>(&chart.timing_points).unwrap_or_default();
    blake3::hash(&encoded).to_hex().to_string()
}
