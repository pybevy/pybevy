use pybevy_core::{
    LogicalTypeId,
    public_error::{ASSET_BRIDGE_NOT_FOUND, invalid_asset_type},
    registry::global_registry,
};
use pyo3::{exceptions::PyTypeError, ffi::PyTypeObject, prelude::*, types::PyType};

/// Parameter type for Assets[T] subscript notation.
///
/// Stores the Python type pointer for the asset type, enabling O(1) lookup
/// in the global AssetBridge registry. When `Assets[HologramMaterial]` is used
/// with a `@material`-decorated class, `wrapper_class` stores the original
/// Python class pointer so `get_mut()` can auto-wrap results.
#[pyclass(name = "AssetTypeParam", frozen, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyAssetTypeParam {
    type_ptr: *const PyTypeObject,
    /// If set, the original `@material` class for auto-wrapping in `get_mut()`.
    wrapper_class: Option<*const PyTypeObject>,
    logical_type_id: Option<LogicalTypeId>,
    logical_type_name: Option<String>,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for PyAssetTypeParam {}
unsafe impl Sync for PyAssetTypeParam {}

impl PyAssetTypeParam {
    /// Create from a Python type object, validating it's a registered asset type.
    pub fn try_from_py_type(asset_type: &Bound<'_, PyType>) -> PyResult<Self> {
        let type_ptr = asset_type.as_type_ptr();
        if global_registry::contains_asset_py_type(type_ptr) {
            Ok(Self {
                type_ptr,
                wrapper_class: None,
                logical_type_id: None,
                logical_type_name: None,
            })
        } else {
            Err(PyTypeError::new_err(invalid_asset_type(asset_type)))
        }
    }

    /// Create with a redirect: `actual_type` is the real asset type (e.g. ShaderMaterial),
    /// `wrapper` is the `@material`-decorated class (e.g. HologramMaterial).
    pub fn with_redirect(
        actual_type: &Bound<'_, PyType>,
        wrapper: &Bound<'_, PyType>,
    ) -> PyResult<Self> {
        let type_ptr = actual_type.as_type_ptr();
        if global_registry::contains_asset_py_type(type_ptr) {
            let logical_type_id = wrapper
                .getattr("__pybevy_logical_type_id__")?
                .extract::<u64>()?;
            let logical_type_name = wrapper.qualname()?.extract::<String>()?;
            Ok(Self {
                type_ptr,
                wrapper_class: Some(wrapper.as_type_ptr()),
                logical_type_id: Some(LogicalTypeId::new(logical_type_id)),
                logical_type_name: Some(logical_type_name),
            })
        } else {
            Err(PyTypeError::new_err(invalid_asset_type(actual_type)))
        }
    }

    /// Get the raw type pointer for bridge lookup.
    pub fn type_ptr(&self) -> *const PyTypeObject {
        self.type_ptr
    }

    /// Get the optional wrapper class pointer (for `@material` redirects).
    pub fn wrapper_class(&self) -> Option<*const PyTypeObject> {
        self.wrapper_class
    }

    pub fn logical_type_id(&self) -> Option<LogicalTypeId> {
        self.logical_type_id
    }

    pub fn logical_type_name(&self) -> Option<&str> {
        self.logical_type_name.as_deref()
    }
}

#[pymethods]
impl PyAssetTypeParam {
    /// Get the Python type class for this asset type parameter.
    pub fn asset_type_class(&self) -> PyResult<Py<PyType>> {
        if let Some(bridge) = global_registry::get_asset_bridge_by_py_type(self.type_ptr) {
            Ok(Python::attach(|py| bridge.py_type(py).unbind()))
        } else {
            Err(PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))
        }
    }
}
