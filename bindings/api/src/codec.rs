use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr;
use std::slice;

use crate::types::{ChartHandle, FfiBytesResult, bytes_error, bytes_success};
use rhythm_open_exchange::codec::{
    InputFormat, OutputFormat, decode_with_format, encode_with_format, from_bytes, from_string,
};

/// Decode a chart from bytes with auto-detection.
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
        #[cfg(feature = "compression")]
        0 => InputFormat::Rox,
        #[cfg(not(feature = "compression"))]
        0 => return ptr::null_mut(),
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
/// # Safety
/// `chart` must be a valid chart handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rox_encode_with_format(chart: ChartHandle, format: i32) -> FfiBytesResult {
    if chart.is_null() {
        return bytes_error("Null chart handle");
    }

    let chart_ref = unsafe { &*chart };
    let fmt = match format {
        #[cfg(feature = "compression")]
        0 => OutputFormat::Rox,
        #[cfg(not(feature = "compression"))]
        0 => return bytes_error("Rox format requires 'compression' feature"),
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

/// Encode a chart to a string.
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
        _ => return ptr::null_mut(),
    };

    match encode_with_format(chart_ref, fmt) {
        Ok(data) => match String::from_utf8(data) {
            Ok(s) => std::ffi::CString::new(s)
                .map(std::ffi::CString::into_raw)
                .unwrap_or(ptr::null_mut()),
            Err(_) => ptr::null_mut(),
        },
        Err(_) => ptr::null_mut(),
    }
}
