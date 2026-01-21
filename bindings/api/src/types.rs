use rhythm_open_exchange::model::RoxChart;
use std::ffi::CString;
use std::os::raw::c_char;

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

// Internal helpers
pub(crate) fn bytes_error(msg: &str) -> FfiBytesResult {
    FfiBytesResult {
        success: 0,
        error: CString::new(msg).unwrap_or_default().into_raw(),
        data: std::ptr::null_mut(),
        len: 0,
    }
}

pub(crate) fn bytes_success(data: Vec<u8>) -> FfiBytesResult {
    let len = data.len();
    let mut boxed = data.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    FfiBytesResult {
        success: 1,
        error: std::ptr::null_mut(),
        data: ptr,
        len,
    }
}
