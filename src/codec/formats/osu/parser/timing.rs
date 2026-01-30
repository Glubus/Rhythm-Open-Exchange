use super::super::types::OsuTimingPoint;

#[must_use]
pub fn parse_timing_point(line: &str) -> Option<OsuTimingPoint> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 8 {
        return None;
    }

    Some(OsuTimingPoint {
        time: parts[0].parse().ok()?,
        beat_length: parts[1].parse().ok()?,
        meter: parts[2].parse().unwrap_or(4),
        sample_set: parts[3].parse().unwrap_or(0),
        sample_index: parts[4].parse().unwrap_or(0),
        volume: parts[5].parse().unwrap_or(100),
        uninherited: parts[6] == "1",
        effects: parts[7].parse().unwrap_or(0),
    })
}
