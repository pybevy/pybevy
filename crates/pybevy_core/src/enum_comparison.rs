//! Interpreter checks for uncalled native enum variants.

use pyo3::{
    IntoPyObjectExt, PyClass, PyTypeInfo, exceptions::PyTypeError, prelude::*, pyclass::CompareOp,
    types::PyType,
};

use crate::public_error;

pub fn compare_values<T: PyClass>(
    value: &T,
    other: &Bound<'_, PyAny>,
    op: CompareOp,
    equal: impl FnOnce(&T, &T) -> PyResult<bool>,
) -> PyResult<Py<PyAny>> {
    let py = other.py();
    if !matches!(op, CompareOp::Eq | CompareOp::Ne) {
        return Ok(py.NotImplemented());
    }
    reject_variant_type::<T>(other)?;
    let Ok(other) = other.extract::<PyRef<'_, T>>() else {
        return Ok(py.NotImplemented());
    };
    let result = equal(value, &other)?;
    (if matches!(op, CompareOp::Eq) {
        result
    } else {
        !result
    })
    .into_py_any(py)
}

pub fn reject_variant_type<T: PyTypeInfo>(other: &Bound<'_, PyAny>) -> PyResult<()> {
    let Ok(variant) = other.cast::<PyType>() else {
        return Ok(());
    };
    let base = other.py().get_type::<T>();
    if !variant.is(&base) && variant.is_subclass(&base)? {
        return Err(PyTypeError::new_err(
            public_error::enum_constructor_comparison(&variant.qualname()?.to_string_lossy()),
        ));
    }
    Ok(())
}
