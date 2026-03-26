use crate::error::{RoxError, RoxResult};
use crate::model::RoxChart;

#[cfg(not(feature = "std"))]
use alloc::vec;

/// Runs all validators in sequence. First error wins.
///
/// Order: metadata → timing points → notes sorted → note columns → note overlaps → note durations
///
/// # Errors
///
/// Returns the first validation error found.
pub fn validate(chart: &RoxChart) -> RoxResult<()> {
    validate_metadata(chart)?;
    validate_timing_points(chart)?;
    validate_notes_sorted(chart)?;
    validate_note_columns(chart)?;
    validate_note_overlaps(chart)?;
    validate_note_durations(chart)?;
    Ok(())
}

fn validate_metadata(chart: &RoxChart) -> RoxResult<()> {
    if chart.key_count == 0 {
        return Err(RoxError::InvalidKeyCount);
    }
    if chart.metadata.is_coop && !chart.key_count.is_multiple_of(2) {
        return Err(RoxError::InvalidCoopKeyCount(chart.key_count));
    }
    Ok(())
}

fn validate_timing_points(chart: &RoxChart) -> RoxResult<()> {
    check_timing_points_sorted(chart)?;
    if chart.notes.is_empty() {
        return Ok(());
    }
    check_bpm_before_first_note(chart)
}

fn check_timing_points_sorted(chart: &RoxChart) -> RoxResult<()> {
    let mut prev_time = i64::MIN;
    for tp in &chart.timing_points {
        if tp.time_us() < prev_time {
            return Err(RoxError::TimingPointsNotSorted {
                prev_time_us: prev_time,
                time_us: tp.time_us(),
            });
        }
        prev_time = tp.time_us();
    }
    Ok(())
}

fn check_bpm_before_first_note(chart: &RoxChart) -> RoxResult<()> {
    if !chart.timing_points.iter().any(crate::model::TimingPoint::is_bpm) {
        return Err(RoxError::NoBpmTimingPoint);
    }
    let first_note_time = chart.notes[0].time_us;
    let first_bpm_time = chart
        .timing_points
        .iter()
        .find(|tp| tp.is_bpm())
        .map_or(i64::MIN, crate::model::TimingPoint::time_us);
    if first_bpm_time > first_note_time {
        return Err(RoxError::BpmAfterFirstNote {
            bpm_time_us: first_bpm_time,
            note_time_us: first_note_time,
        });
    }
    Ok(())
}

fn validate_notes_sorted(chart: &RoxChart) -> RoxResult<()> {
    let mut prev_time = i64::MIN;
    for note in &chart.notes {
        if note.time_us < prev_time {
            return Err(RoxError::NotesNotSorted {
                prev_time_us: prev_time,
                time_us: note.time_us,
            });
        }
        prev_time = note.time_us;
    }
    Ok(())
}

fn validate_note_columns(chart: &RoxChart) -> RoxResult<()> {
    for note in &chart.notes {
        if note.column >= chart.key_count {
            return Err(RoxError::InvalidColumn {
                column: note.column,
                key_count: chart.key_count,
            });
        }
    }
    Ok(())
}

fn validate_note_overlaps(chart: &RoxChart) -> RoxResult<()> {
    let mut last_end = vec![i64::MIN; chart.key_count as usize];
    for note in &chart.notes {
        let col = note.column as usize;
        if note.time_us < last_end[col] {
            return Err(RoxError::OverlappingNotes {
                column: note.column,
                time_us: note.time_us,
            });
        }
        last_end[col] = note.end_time_us();
    }
    Ok(())
}

fn validate_note_durations(chart: &RoxChart) -> RoxResult<()> {
    for note in &chart.notes {
        if (note.is_hold() || note.is_burst()) && note.duration_us() <= 0 {
            return Err(RoxError::InvalidHoldDuration {
                time_us: note.time_us,
                duration_us: note.duration_us(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Note, TimingPoint};
    use rstest::rstest;

    // --- validate_metadata ---
    #[rstest]
    #[case(0, false, true)]   // key_count=0 → error
    #[case(4, false, false)]  // valid
    #[case(3, true, true)]    // coop + odd key_count → error
    #[case(4, true, false)]   // coop + even key_count → ok
    #[case(8, true, false)]   // coop + 8 → ok
    fn test_validate_metadata(
        #[case] key_count: u8,
        #[case] is_coop: bool,
        #[case] should_fail: bool,
    ) {
        let mut chart = RoxChart::new(key_count);
        chart.metadata.is_coop = is_coop;
        assert_eq!(
            validate_metadata(&chart).is_err(),
            should_fail,
            "key_count={key_count} coop={is_coop}"
        );
    }

    // --- validate_timing_points ---
    #[rstest]
    #[case(vec![], vec![], false)]
    #[case(vec![TimingPoint::bpm(0, 120.0)], vec![Note::tap(0, 0)], false)]
    #[case(vec![TimingPoint::bpm(1_000_000, 120.0)], vec![Note::tap(0, 0)], true)]
    #[case(vec![TimingPoint::sv(0, 1.5)], vec![Note::tap(0, 0)], true)]
    #[case(vec![TimingPoint::bpm(1_000_000, 120.0), TimingPoint::bpm(0, 90.0)], vec![], true)]
    fn test_validate_timing_points(
        #[case] timing_points: Vec<TimingPoint>,
        #[case] notes: Vec<Note>,
        #[case] should_fail: bool,
    ) {
        let mut chart = RoxChart::new(4);
        chart.timing_points = timing_points;
        chart.notes = notes;
        assert_eq!(validate_timing_points(&chart).is_err(), should_fail);
    }

    // --- validate_notes_sorted ---
    #[rstest]
    #[case(vec![Note::tap(0, 0), Note::tap(1_000_000, 0)], false)]
    #[case(vec![Note::tap(1_000_000, 0), Note::tap(0, 0)], true)]
    fn test_validate_notes_sorted(#[case] notes: Vec<Note>, #[case] should_fail: bool) {
        let mut chart = RoxChart::new(4);
        chart.notes = notes;
        assert_eq!(validate_notes_sorted(&chart).is_err(), should_fail);
    }

    // --- validate_note_columns ---
    #[rstest]
    #[case(vec![Note::tap(0, 3)], 4, false)]
    #[case(vec![Note::tap(0, 4)], 4, true)]
    #[case(vec![Note::tap(0, 0)], 1, false)]
    fn test_validate_note_columns(
        #[case] notes: Vec<Note>,
        #[case] key_count: u8,
        #[case] should_fail: bool,
    ) {
        let mut chart = RoxChart::new(key_count);
        chart.notes = notes;
        assert_eq!(validate_note_columns(&chart).is_err(), should_fail);
    }

    // --- validate_note_overlaps ---
    #[rstest]
    #[case(vec![Note::hold(0, 500_000, 0), Note::tap(300_000, 0)], true)]
    #[case(vec![Note::hold(0, 500_000, 0), Note::tap(500_000, 0)], false)]
    #[case(vec![Note::hold(0, 500_000, 0), Note::tap(300_000, 1)], false)]
    fn test_validate_note_overlaps(#[case] notes: Vec<Note>, #[case] should_fail: bool) {
        let mut chart = RoxChart::new(4);
        chart.notes = notes;
        assert_eq!(validate_note_overlaps(&chart).is_err(), should_fail);
    }

    // --- validate_note_durations ---
    #[rstest]
    #[case(Note::hold(0, 0, 0), true)]
    #[case(Note::hold(0, -1, 0), true)]
    #[case(Note::hold(0, 1, 0), false)]
    #[case(Note::burst(0, 0, 0), true)]
    #[case(Note::tap(0, 0), false)]
    #[case(Note::mine(0, 0), false)]
    fn test_validate_note_durations(#[case] note: Note, #[case] should_fail: bool) {
        let mut chart = RoxChart::new(4);
        chart.notes = vec![note];
        assert_eq!(validate_note_durations(&chart).is_err(), should_fail);
    }

    // --- full validate() orchestrator ---
    #[rstest]
    fn test_validate_empty_chart_ok() {
        assert!(RoxChart::new(4).validate().is_ok());
    }

    #[rstest]
    fn test_validate_full_valid_chart() {
        let mut chart = RoxChart::new(4);
        chart.timing_points.push(TimingPoint::bpm(0, 120.0));
        chart.notes.push(Note::tap(0, 0));
        chart.notes.push(Note::tap(0, 3));
        assert!(chart.validate().is_ok());
    }

    #[rstest]
    fn test_validate_zero_key_count_fails() {
        assert!(RoxChart::new(0).validate().is_err());
    }

    #[rstest]
    fn test_validate_invalid_column_fails() {
        let mut chart = RoxChart::new(4);
        chart.notes.push(Note::tap(0, 4));
        assert!(chart.validate().is_err());
    }
}
