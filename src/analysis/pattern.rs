use crate::model::{Note, RoxChart};
use std::collections::HashMap;

/// Calculate chord density (polyphony) distribution.
///
/// Returns a map where:
/// - Key = Chord size (1 = Single, 2 = Jump, 3 = Hand, 4 = Quad...)
/// - Value = Count of occurrences
pub fn polyphony(chart: &RoxChart) -> HashMap<u32, u32> {
    if chart.notes.is_empty() {
        return HashMap::new();
    }

    let mut distribution = HashMap::new();
    let mut current_time: Option<i64> = None;
    let mut current_cluster_size = 0;

    let mut refs: Vec<&Note> = chart.notes.iter().collect();
    refs.sort_by_key(|n| n.time_us);

    for note in refs {
        if note.is_mine() {
            continue;
        }

        if Some(note.time_us) != current_time {
            // New cluster, commit previous one
            if current_cluster_size > 0 {
                *distribution.entry(current_cluster_size).or_insert(0) += 1;
            }
            current_time = Some(note.time_us);
            current_cluster_size = 0;
        }
        current_cluster_size += 1;
    }

    // Commit last cluster
    if current_cluster_size > 0 {
        *distribution.entry(current_cluster_size).or_insert(0) += 1;
    }

    distribution
}

/// Calculate lane usage balance.
///
/// Returns a vector of size `key_count` where index is column and value is note count.
pub fn lane_balance(chart: &RoxChart) -> Vec<u32> {
    let mut counts = vec![0; chart.key_count() as usize];

    for note in &chart.notes {
        if note.is_mine() {
            continue;
        }
        let col = note.column as usize;
        if col < counts.len() {
            counts[col] += 1;
        }
    }

    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Note;

    #[test]
    fn test_lane_balance() {
        let mut chart = RoxChart::new(4);
        chart.notes.push(Note::tap(0, 0));
        chart.notes.push(Note::tap(100, 0));
        chart.notes.push(Note::tap(200, 3));

        let balance = lane_balance(&chart);
        assert_eq!(balance, vec![2, 0, 0, 1]);
    }

    #[test]
    fn test_polyphony() {
        let mut chart = RoxChart::new(4);

        // Single at 0
        chart.notes.push(Note::tap(0, 0));

        // Jump at 100
        chart.notes.push(Note::tap(100, 0));
        chart.notes.push(Note::tap(100, 1));

        // Hand at 200
        chart.notes.push(Note::tap(200, 0));
        chart.notes.push(Note::tap(200, 1));
        chart.notes.push(Note::tap(200, 2));

        // Another Single at 300
        chart.notes.push(Note::tap(300, 3));

        let dist = polyphony(&chart);

        assert_eq!(dist.get(&1), Some(&2)); // 2 Singles
        assert_eq!(dist.get(&2), Some(&1)); // 1 Jump
        assert_eq!(dist.get(&3), Some(&1)); // 1 Hand
        assert_eq!(dist.get(&4), None);
    }
}
