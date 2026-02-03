use super::super::types::OsuTimingPoint;

#[must_use]
pub fn parse_timing_point(line: &str) -> Option<OsuTimingPoint> {
    let mut parts = line.split(',');

    Some(OsuTimingPoint {
        time: parts.next()?.parse().ok()?,
        beat_length: parts.next()?.parse().ok()?,
        meter: parts.next().and_then(|s| s.parse().ok()).unwrap_or(4),
        sample_set: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
        sample_index: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
        volume: parts.next().and_then(|s| s.parse().ok()).unwrap_or(100),
        uninherited: parts.next().is_some_and(|s| s == "1"),
        effects: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
    })
}
