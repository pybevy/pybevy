//! Utility functions for working with Python types.

use pyo3::{ffi::PyTypeObject, prelude::*, types::PyType};

/// Extract the name of a Python type from a PyTypeObject pointer.
///
/// This is commonly used for error messages when working with custom
/// components or resources. Returns "Unknown" if the type name cannot
/// be extracted.
///
/// # Arguments
/// * `py` - Python GIL token
/// * `type_ptr` - Pointer to the Python type object
///
/// # Returns
/// The type name as a String, or "Unknown" if extraction fails
///
/// # Safety
/// The type_ptr must be a valid PyTypeObject pointer that remains
/// valid for the duration of this call.
pub fn get_python_type_name(py: Python, type_ptr: *const PyTypeObject) -> String {
    unsafe {
        let type_obj = pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject);
        type_obj
            .cast::<PyType>()
            .ok()
            .and_then(|t| t.name().ok())
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Unknown".to_string())
    }
}
