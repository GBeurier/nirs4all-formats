//! Minimal C ABI scaffold.
//!
//! The ABI is intentionally tiny until the Rust data model has format readers
//! behind it. Additive symbols are tracked in docs once bindings consume them.

use std::ffi::{c_char, c_void, CString};

/// ABI version of the C surface, independent from the crate semver.
pub const N4IO_ABI_VERSION: &str = "0.1.0";

/// Return the C ABI version string.
#[no_mangle]
pub extern "C" fn n4io_abi_version() -> *mut c_char {
    CString::new(N4IO_ABI_VERSION)
        .expect("static version contains no nul")
        .into_raw()
}

/// Free strings returned by this ABI.
///
/// # Safety
///
/// `ptr` must either be null or a pointer previously returned by a
/// `nirs4all-io` C ABI function that transfers string ownership to the caller.
/// It must not be freed more than once.
#[no_mangle]
pub unsafe extern "C" fn n4io_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

/// Placeholder hook used by bindings to detect that the native core is loaded.
#[no_mangle]
pub extern "C" fn n4io_core_is_available() -> bool {
    true
}

/// Reserved opaque handle type for future records/collections.
#[repr(C)]
pub struct n4io_handle_t {
    _private: [u8; 0],
    _marker: std::marker::PhantomData<(*mut c_void, std::marker::PhantomPinned)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_current_alpha() {
        assert_eq!(N4IO_ABI_VERSION, "0.1.0");
    }
}
