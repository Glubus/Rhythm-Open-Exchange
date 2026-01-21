#[macro_export]
macro_rules! get_chart {
    ($ptr:expr) => {{
        if $ptr.is_null() {
            return std::ptr::null_mut();
        }
        unsafe { &*$ptr }
    }};
    ($ptr:expr, default: $def:expr) => {{
        if $ptr.is_null() {
            return $def;
        }
        unsafe { &*$ptr }
    }};
}

#[macro_export]
macro_rules! get_chart_mut {
    ($ptr:expr) => {{
        if $ptr.is_null() {
            return;
        }
        unsafe { &mut *$ptr }
    }};
}

#[macro_export]
macro_rules! impl_string_getter {
    ($name:ident, $field:ident) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            chart: $crate::types::ChartHandle,
        ) -> *mut std::os::raw::c_char {
            let chart_ref = $crate::get_chart!(chart);
            std::ffi::CString::new(chart_ref.metadata.$field.clone())
                .map(std::ffi::CString::into_raw)
                .unwrap_or(std::ptr::null_mut())
        }
    };
}

#[macro_export]
macro_rules! impl_string_setter {
    ($name:ident, $field:ident) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            chart: $crate::types::ChartHandle,
            value: *const std::os::raw::c_char,
        ) {
            if value.is_null() {
                return;
            }
            let chart_ref = $crate::get_chart_mut!(chart);
            if let Ok(s) = unsafe { std::ffi::CStr::from_ptr(value) }.to_str() {
                chart_ref.metadata.$field = s.to_string();
            }
        }
    };
}

#[macro_export]
macro_rules! impl_primitive_getter {
    // Case 1: Struct method (e.g. chart.key_count())
    ($name:ident, method $field:ident, $ret:ty, $conv:expr) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(chart: $crate::types::ChartHandle) -> $ret {
            let chart_ref = $crate::get_chart!(chart, default: 0 as $ret);
            $conv(chart_ref.$field())
        }
    };
    // Case 2: Metadata field (e.g. chart.metadata.key_count)
    ($name:ident, meta $field:ident, $ret:ty, $conv:expr, $def:expr) => {
         #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(chart: $crate::types::ChartHandle) -> $ret {
            let chart_ref = $crate::get_chart!(chart, default: $def);
            $conv(chart_ref.metadata.$field)
        }
    };
}

#[macro_export]
macro_rules! impl_primitive_setter {
    ($name:ident, meta $field:ident, $arg:ty, $conv:expr) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(chart: $crate::types::ChartHandle, value: $arg) {
            let chart_ref = $crate::get_chart_mut!(chart);
            chart_ref.metadata.$field = $conv(value);
        }
    };
    ($name:ident, meta $field:ident, $arg:ty) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(chart: $crate::types::ChartHandle, value: $arg) {
            let chart_ref = $crate::get_chart_mut!(chart);
            chart_ref.metadata.$field = value;
        }
    };
}
