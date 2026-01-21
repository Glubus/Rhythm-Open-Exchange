use crate::types::{ChartHandle, FfiNote};
use rhythm_open_exchange::model::{Note, NoteType};

/// Add a tap note.
/// # Safety
/// `chart` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_add_tap(chart: ChartHandle, time_us: i64, column: u8) {
    let chart_ref = get_chart_mut!(chart);
    chart_ref.notes.push(Note::tap(time_us, column));
}

/// Add a hold note.
/// # Safety
/// `chart` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_add_hold(
    chart: ChartHandle,
    time_us: i64,
    duration_us: i64,
    column: u8,
) {
    let chart_ref = get_chart_mut!(chart);
    chart_ref
        .notes
        .push(Note::hold(time_us, duration_us, column));
}

/// Add a burst note.
/// # Safety
/// `chart` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_add_burst(
    chart: ChartHandle,
    time_us: i64,
    duration_us: i64,
    column: u8,
) {
    let chart_ref = get_chart_mut!(chart);
    chart_ref
        .notes
        .push(Note::burst(time_us, duration_us, column));
}

/// Add a mine note.
/// # Safety
/// `chart` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_add_mine(chart: ChartHandle, time_us: i64, column: u8) {
    let chart_ref = get_chart_mut!(chart);
    chart_ref.notes.push(Note::mine(time_us, column));
}

/// Remove a note.
/// # Safety
/// `chart` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_remove_note(chart: ChartHandle, index: usize) -> i32 {
    if chart.is_null() {
        return 0;
    }
    let chart_ref = unsafe { &mut *chart };
    if index < chart_ref.notes.len() {
        chart_ref.notes.remove(index);
        1
    } else {
        0
    }
}

/// Clear all notes.
/// # Safety
/// `chart` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_clear_notes(chart: ChartHandle) {
    let chart_ref = get_chart_mut!(chart);
    chart_ref.notes.clear();
}

/// Get a note.
/// # Safety
/// `chart` and `out` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_get_note(
    chart: ChartHandle,
    index: usize,
    out: *mut FfiNote,
) -> i32 {
    if chart.is_null() || out.is_null() {
        return 0;
    }
    let chart_ref = unsafe { &*chart };
    if index >= chart_ref.notes.len() {
        return 0;
    }

    let note = &chart_ref.notes[index];
    let (ntype, dur) = match note.note_type {
        NoteType::Tap => (0, 0),
        NoteType::Hold { duration_us } => (1, duration_us),
        NoteType::Burst { duration_us } => (2, duration_us),
        NoteType::Mine => (3, 0),
    };

    unsafe {
        (*out).time_us = note.time_us;
        (*out).column = note.column;
        (*out).note_type = ntype;
        (*out).duration_us = dur;
    }
    1
}

/// Sort notes.
/// # Safety
/// `chart` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_sort_notes(chart: ChartHandle) {
    let chart_ref = get_chart_mut!(chart);
    chart_ref.notes.sort_by_key(|n| n.time_us);
}
