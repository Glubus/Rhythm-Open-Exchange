use crate::types::ChartHandle;
use rhythm_open_exchange::model::RoxChart;
use std::ffi::CString;
use std::os::raw::c_char;
use std::slice;

/// Create a new chart.
#[unsafe(no_mangle)]
pub extern "C" fn rox_chart_new(key_count: u8) -> ChartHandle {
    Box::into_raw(Box::new(RoxChart::new(key_count)))
}

/// Free a chart.
/// # Safety
/// `chart` must be a valid handle or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_free_chart(chart: ChartHandle) {
    if !chart.is_null() {
        unsafe { drop(Box::from_raw(chart)) };
    }
}

/// Free a string.
/// # Safety
/// `s` must be valid or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)) };
    }
}

/// Free bytes.
/// # Safety
/// `data` must be valid or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_free_bytes(data: *mut u8, len: usize) {
    if !data.is_null() && len > 0 {
        let slice = unsafe { slice::from_raw_parts_mut(data, len) };
        unsafe { drop(Box::from_raw(std::ptr::from_mut::<[u8]>(slice))) };
    }
}
