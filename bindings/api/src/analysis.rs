use crate::types::ChartHandle;
use rhythm_open_exchange::analysis::RoxAnalysis;
use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr;

/// Get the full hash.
/// # Safety
/// `chart` must be valid.
#[unsafe(no_mangle)]
#[cfg(feature = "analysis")]
pub unsafe extern "C" fn rox_chart_hash(chart: ChartHandle) -> *mut c_char {
    let chart_ref = get_chart!(chart);
    CString::new(chart_ref.hash())
        .map(CString::into_raw)
        .unwrap_or(ptr::null_mut())
}

#[cfg(not(feature = "analysis"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_hash(_chart: ChartHandle) -> *mut c_char {
    ptr::null_mut()
}

/// Get notes-only hash.
/// # Safety
/// `chart` must be valid.
#[unsafe(no_mangle)]
#[cfg(feature = "analysis")]
pub unsafe extern "C" fn rox_chart_notes_hash(chart: ChartHandle) -> *mut c_char {
    let chart_ref = get_chart!(chart);
    CString::new(chart_ref.notes_hash())
        .map(CString::into_raw)
        .unwrap_or(ptr::null_mut())
}

#[cfg(not(feature = "analysis"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_notes_hash(_chart: ChartHandle) -> *mut c_char {
    ptr::null_mut()
}

/// Get short hash.
/// # Safety
/// `chart` must be valid.
#[unsafe(no_mangle)]
#[cfg(feature = "analysis")]
pub unsafe extern "C" fn rox_chart_short_hash(chart: ChartHandle) -> *mut c_char {
    let chart_ref = get_chart!(chart);
    CString::new(chart_ref.short_hash())
        .map(CString::into_raw)
        .unwrap_or(ptr::null_mut())
}

#[cfg(not(feature = "analysis"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_chart_short_hash(_chart: ChartHandle) -> *mut c_char {
    ptr::null_mut()
}
