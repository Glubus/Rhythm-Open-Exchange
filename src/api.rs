//! FFI API for C# bindings.
//!
//! Provides C-compatible functions for chart encoding/decoding.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::slice;

use crate::codec::{
    InputFormat, OutputFormat, decode_with_format, encode_with_format, from_bytes, from_string,
};
use crate::model::RoxChart;

/// Opaque handle to a `RoxChart`.
pub type ChartHandle = *mut RoxChart;

/// Result of an FFI operation.
#[repr(C)]
pub struct FfiResult {
    /// Success flag (1 = success, 0 = failure).
    pub success: i32,
    /// Error message (null if success, must be freed with `rox_free_string`).
    pub error: *mut c_char,
}

/// Result containing bytes.
#[repr(C)]
pub struct FfiBytesResult {
    /// Success flag.
    pub success: i32,
    /// Error message (null if success).
    pub error: *mut c_char,
    /// Output data pointer (null if error).
    pub data: *mut u8,
    /// Output data length.
    pub len: usize,
}

// Helper to create error result
#[allow(dead_code)]
fn error_result(msg: &str) -> FfiResult {
    FfiResult {
        success: 0,
        error: CString::new(msg).unwrap_or_default().into_raw(),
    }
}

#[allow(dead_code)]
fn success_result() -> FfiResult {
    FfiResult {
        success: 1,
        error: ptr::null_mut(),
    }
}

fn bytes_error(msg: &str) -> FfiBytesResult {
    FfiBytesResult {
        success: 0,
        error: CString::new(msg).unwrap_or_default().into_raw(),
        data: ptr::null_mut(),
        len: 0,
    }
}

fn bytes_success(data: Vec<u8>) -> FfiBytesResult {
    let len = data.len();
    let mut boxed = data.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    FfiBytesResult {
        success: 1,
        error: ptr::null_mut(),
        data: ptr,
        len,
    }
}

/// Decode a chart from bytes with auto-detection.
///
/// Returns a chart handle on success, null on failure.
/// The chart must be freed with `rox_free_chart`.
///
/// # Safety
/// `data` must be a valid pointer to `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_decode_bytes(data: *const u8, len: usize) -> ChartHandle {
    if data.is_null() || len == 0 {
        return ptr::null_mut();
    }

    let slice = unsafe { slice::from_raw_parts(data, len) };
    match from_bytes(slice) {
        Ok(chart) => Box::into_raw(Box::new(chart)),
        Err(_) => ptr::null_mut(),
    }
}

/// Decode a chart from a string with auto-detection.
///
/// Returns a chart handle on success, null on failure.
/// The chart must be freed with `rox_free_chart`.
///
/// # Safety
/// `data` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_decode_string(data: *const c_char) -> ChartHandle {
    if data.is_null() {
        return ptr::null_mut();
    }

    let c_str = unsafe { CStr::from_ptr(data) };
    let Ok(rust_str) = c_str.to_str() else {
        return ptr::null_mut();
    };

    match from_string(rust_str) {
        Ok(chart) => Box::into_raw(Box::new(chart)),
        Err(_) => ptr::null_mut(),
    }
}

/// Decode a chart from bytes with a specific format.
///
/// Format: 0=Rox, 1=Osu, 2=Sm, 3=Qua, 4=Fnf
///
/// # Safety
/// `data` must be a valid pointer to `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_decode_with_format(
    data: *const u8,
    len: usize,
    format: i32,
) -> ChartHandle {
    if data.is_null() || len == 0 {
        return ptr::null_mut();
    }

    let slice = unsafe { slice::from_raw_parts(data, len) };
    let fmt = match format {
        0 => InputFormat::Rox,
        1 => InputFormat::Osu,
        2 => InputFormat::Sm,
        3 => InputFormat::Qua,
        4 => InputFormat::Fnf,
        _ => return ptr::null_mut(),
    };

    match decode_with_format(slice, fmt) {
        Ok(chart) => Box::into_raw(Box::new(chart)),
        Err(_) => ptr::null_mut(),
    }
}

/// Encode a chart to bytes with a specific format.
///
/// Format: 0=Rox, 1=Osu, 2=Sm, 3=Qua, 4=Fnf
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_encode_with_format(chart: ChartHandle, format: i32) -> FfiBytesResult {
    if chart.is_null() {
        return bytes_error("Null chart handle");
    }

    let chart_ref = unsafe { &*chart };
    let fmt = match format {
        0 => OutputFormat::Rox,
        1 => OutputFormat::Osu,
        2 => OutputFormat::Sm,
        3 => OutputFormat::Qua,
        4 => OutputFormat::Fnf,
        _ => return bytes_error("Invalid format"),
    };

    match encode_with_format(chart_ref, fmt) {
        Ok(data) => bytes_success(data),
        Err(e) => bytes_error(&e.to_string()),
    }
}

/// Encode a chart to a string (for text-based formats).
///
/// Format: 1=Osu, 2=Sm, 3=Qua, 4=Fnf (Rox is binary, not supported)
///
/// Returns a null-terminated string that must be freed with `rox_free_string`.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_encode_to_string(chart: ChartHandle, format: i32) -> *mut c_char {
    if chart.is_null() {
        return ptr::null_mut();
    }

    let chart_ref = unsafe { &*chart };
    let fmt = match format {
        1 => OutputFormat::Osu,
        2 => OutputFormat::Sm,
        3 => OutputFormat::Qua,
        4 => OutputFormat::Fnf,
        _ => return ptr::null_mut(), // Rox is binary
    };

    match encode_with_format(chart_ref, fmt) {
        Ok(data) => {
            // Convert bytes to string
            match String::from_utf8(data) {
                Ok(s) => CString::new(s)
                    .map(CString::into_raw)
                    .unwrap_or(ptr::null_mut()),
                Err(_) => ptr::null_mut(),
            }
        }
        Err(_) => ptr::null_mut(),
    }
}

/// Get the title of a chart.
///
/// Returns a null-terminated string that must be freed with `rox_free_string`.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_title(chart: ChartHandle) -> *mut c_char {
    if chart.is_null() {
        return ptr::null_mut();
    }
    let chart_ref = unsafe { &*chart };
    CString::new(chart_ref.metadata.title.clone())
        .map(CString::into_raw)
        .unwrap_or(ptr::null_mut())
}

/// Get the artist of a chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_artist(chart: ChartHandle) -> *mut c_char {
    if chart.is_null() {
        return ptr::null_mut();
    }
    let chart_ref = unsafe { &*chart };
    CString::new(chart_ref.metadata.artist.clone())
        .map(CString::into_raw)
        .unwrap_or(ptr::null_mut())
}

/// Get the key count of a chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_key_count(chart: ChartHandle) -> i32 {
    if chart.is_null() {
        return 0;
    }
    let chart_ref = unsafe { &*chart };
    i32::from(chart_ref.key_count())
}

/// Get the note count of a chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_note_count(chart: ChartHandle) -> usize {
    if chart.is_null() {
        return 0;
    }
    let chart_ref = unsafe { &*chart };
    chart_ref.note_count()
}

/// Get the full hash of a chart (BLAKE3).
///
/// Returns a null-terminated hex string that must be freed with `rox_free_string`.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_hash(chart: ChartHandle) -> *mut c_char {
    if chart.is_null() {
        return ptr::null_mut();
    }
    let chart_ref = unsafe { &*chart };
    CString::new(chart_ref.hash())
        .map(CString::into_raw)
        .unwrap_or(ptr::null_mut())
}

/// Get the notes-only hash of a chart (BLAKE3).
/// This ignores metadata, timing points, and hitsounds.
///
/// Returns a null-terminated hex string that must be freed with `rox_free_string`.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_notes_hash(chart: ChartHandle) -> *mut c_char {
    if chart.is_null() {
        return ptr::null_mut();
    }
    let chart_ref = unsafe { &*chart };
    CString::new(chart_ref.notes_hash())
        .map(CString::into_raw)
        .unwrap_or(ptr::null_mut())
}

/// Get the short hash of a chart (first 16 hex chars).
///
/// Returns a null-terminated string that must be freed with `rox_free_string`.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_short_hash(chart: ChartHandle) -> *mut c_char {
    if chart.is_null() {
        return ptr::null_mut();
    }
    let chart_ref = unsafe { &*chart };
    CString::new(chart_ref.short_hash())
        .map(CString::into_raw)
        .unwrap_or(ptr::null_mut())
}

/// Get the creator of a chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_creator(chart: ChartHandle) -> *mut c_char {
    if chart.is_null() {
        return ptr::null_mut();
    }
    let chart_ref = unsafe { &*chart };
    CString::new(chart_ref.metadata.creator.clone())
        .map(CString::into_raw)
        .unwrap_or(ptr::null_mut())
}

/// Get the difficulty name of a chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_difficulty(chart: ChartHandle) -> *mut c_char {
    if chart.is_null() {
        return ptr::null_mut();
    }
    let chart_ref = unsafe { &*chart };
    CString::new(chart_ref.metadata.difficulty_name.clone())
        .map(CString::into_raw)
        .unwrap_or(ptr::null_mut())
}

/// Get the duration of a chart in microseconds.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_duration_us(chart: ChartHandle) -> i64 {
    if chart.is_null() {
        return 0;
    }
    let chart_ref = unsafe { &*chart };
    chart_ref.duration_us()
}

/// Set the title of a chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
/// `title` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_set_title(chart: ChartHandle, title: *const c_char) {
    if chart.is_null() || title.is_null() {
        return;
    }
    let chart_ref = unsafe { &mut *chart };
    if let Ok(s) = unsafe { CStr::from_ptr(title) }.to_str() {
        chart_ref.metadata.title = s.to_string();
    }
}

/// Set the artist of a chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
/// `artist` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_set_artist(chart: ChartHandle, artist: *const c_char) {
    if chart.is_null() || artist.is_null() {
        return;
    }
    let chart_ref = unsafe { &mut *chart };
    if let Ok(s) = unsafe { CStr::from_ptr(artist) }.to_str() {
        chart_ref.metadata.artist = s.to_string();
    }
}

/// Set the creator of a chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
/// `creator` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_set_creator(chart: ChartHandle, creator: *const c_char) {
    if chart.is_null() || creator.is_null() {
        return;
    }
    let chart_ref = unsafe { &mut *chart };
    if let Ok(s) = unsafe { CStr::from_ptr(creator) }.to_str() {
        chart_ref.metadata.creator = s.to_string();
    }
}

/// Set the difficulty name of a chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
/// `difficulty` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_set_difficulty(chart: ChartHandle, difficulty: *const c_char) {
    if chart.is_null() || difficulty.is_null() {
        return;
    }
    let chart_ref = unsafe { &mut *chart };
    if let Ok(s) = unsafe { CStr::from_ptr(difficulty) }.to_str() {
        chart_ref.metadata.difficulty_name = s.to_string();
    }
}

/// Set the key count of a chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_set_key_count(chart: ChartHandle, key_count: u8) {
    if chart.is_null() {
        return;
    }
    let chart_ref = unsafe { &mut *chart };
    chart_ref.metadata.key_count = key_count;
}

/// Get whether the chart is a coop chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_is_coop(chart: ChartHandle) -> i32 {
    if chart.is_null() {
        return 0;
    }
    let chart_ref = unsafe { &*chart };
    if chart_ref.metadata.is_coop { 1 } else { 0 }
}

/// Set whether the chart is a coop chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_set_coop(chart: ChartHandle, is_coop: i32) {
    if chart.is_null() {
        return;
    }
    let chart_ref = unsafe { &mut *chart };
    chart_ref.metadata.is_coop = is_coop != 0;
}

// ============================================================================
// Note manipulation
// ============================================================================

/// FFI representation of a note.
#[repr(C)]
pub struct FfiNote {
    /// Time in microseconds.
    pub time_us: i64,
    /// Column index.
    pub column: u8,
    /// Note type: 0=Tap, 1=Hold, 2=Burst, 3=Mine.
    pub note_type: u8,
    /// Duration in microseconds (for Hold/Burst, 0 otherwise).
    pub duration_us: i64,
}

/// Add a tap note to the chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_add_tap(chart: ChartHandle, time_us: i64, column: u8) {
    if chart.is_null() {
        return;
    }
    let chart_ref = unsafe { &mut *chart };
    chart_ref
        .notes
        .push(crate::model::Note::tap(time_us, column));
}

/// Add a hold note to the chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_add_hold(
    chart: ChartHandle,
    time_us: i64,
    duration_us: i64,
    column: u8,
) {
    if chart.is_null() {
        return;
    }
    let chart_ref = unsafe { &mut *chart };
    chart_ref
        .notes
        .push(crate::model::Note::hold(time_us, duration_us, column));
}

/// Add a burst/roll note to the chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_add_burst(
    chart: ChartHandle,
    time_us: i64,
    duration_us: i64,
    column: u8,
) {
    if chart.is_null() {
        return;
    }
    let chart_ref = unsafe { &mut *chart };
    chart_ref
        .notes
        .push(crate::model::Note::burst(time_us, duration_us, column));
}

/// Add a mine note to the chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_add_mine(chart: ChartHandle, time_us: i64, column: u8) {
    if chart.is_null() {
        return;
    }
    let chart_ref = unsafe { &mut *chart };
    chart_ref
        .notes
        .push(crate::model::Note::mine(time_us, column));
}

/// Remove a note at the specified index.
///
/// # Safety
/// `chart` must be a valid chart handle.
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

/// Clear all notes from the chart.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_clear_notes(chart: ChartHandle) {
    if chart.is_null() {
        return;
    }
    let chart_ref = unsafe { &mut *chart };
    chart_ref.notes.clear();
}

/// Get a note at the specified index.
/// Returns 1 on success, 0 on failure.
///
/// # Safety
/// `chart` must be a valid chart handle.
/// `out` must be a valid pointer to an FfiNote.
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
    let (note_type, duration_us) = match note.note_type {
        crate::model::NoteType::Tap => (0, 0),
        crate::model::NoteType::Hold { duration_us } => (1, duration_us),
        crate::model::NoteType::Burst { duration_us } => (2, duration_us),
        crate::model::NoteType::Mine => (3, 0),
    };
    unsafe {
        (*out).time_us = note.time_us;
        (*out).column = note.column;
        (*out).note_type = note_type;
        (*out).duration_us = duration_us;
    }
    1
}

/// Sort notes by time.
///
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_sort_notes(chart: ChartHandle) {
    if chart.is_null() {
        return;
    }
    let chart_ref = unsafe { &mut *chart };
    chart_ref.notes.sort_by_key(|n| n.time_us);
}

/// Create a new empty chart with the given key count.
///
/// Returns a chart handle that must be freed with `rox_free_chart`.
#[unsafe(no_mangle)]
pub extern "C" fn rox_chart_new(key_count: u8) -> ChartHandle {
    Box::into_raw(Box::new(crate::model::RoxChart::new(key_count)))
}

/// Free a chart handle.
///
/// # Safety
/// `chart` must be a valid chart handle or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_free_chart(chart: ChartHandle) {
    if !chart.is_null() {
        unsafe { drop(Box::from_raw(chart)) };
    }
}

/// Free a string returned by FFI functions.
///
/// # Safety
/// `s` must be a string returned by this library or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)) };
    }
}

/// Free bytes returned by FFI functions.
///
/// # Safety
/// `data` must be a pointer returned by this library or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_free_bytes(data: *mut u8, len: usize) {
    if !data.is_null() && len > 0 {
        let slice = unsafe { slice::from_raw_parts_mut(data, len) };
        unsafe { drop(Box::from_raw(std::ptr::from_mut::<[u8]>(slice))) };
    }
}
