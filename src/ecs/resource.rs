pub use pybevy_core::PyResource;
use pyo3::{
    exceptions::PyTypeError,
    prelude::*,
    types::{PyTuple, PyType},
};

/// Read-only access to a Bevy resource
#[pyclass(name = "Res")]
pub struct PyRes {
    #[pyo3(get, name = "resource_type")]
    ty: Py<PyType>,

    value: Py<PyAny>,
}

impl PyRes {
    pub fn new(value: Bound<'_, PyAny>) -> Self {
        Self {
            ty: value.get_type().into(),
            value: value.unbind(),
        }
    }
}

#[pymethods]
impl PyRes {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        // Create a GenericAlias like types.GenericAlias(Res, (key,))
        // This allows the system parameter parser to detect Res[T] properly
        let py = cls.py();
        let types_mod = py.import("types")?;
        let generic_alias_cls = types_mod.getattr("GenericAlias")?;

        // Create GenericAlias(Res, (key,))
        let args = PyTuple::new(py, [key])?;
        let generic = generic_alias_cls.call1((cls, args))?;

        Ok(generic.unbind())
    }

    /// Proxy attribute access to the wrapped resource (read-only)
    pub fn __getattr__(&self, py: Python, name: &str) -> PyResult<Py<PyAny>> {
        self.value.bind(py).getattr(name).map(|v| v.unbind())
    }

    /// Prevent attribute setting on read-only resource
    pub fn __setattr__(&self, _py: Python, name: &str, _value: Bound<'_, PyAny>) -> PyResult<()> {
        Err(PyTypeError::new_err(format!(
            "Cannot set attribute '{}' on read-only Res - use ResMut instead",
            name
        )))
    }
}

/// Mutable access to a Bevy resource
#[pyclass(name = "ResMut")]
pub struct PyResMut {
    #[pyo3(get, name = "resource_type")]
    ty: Py<PyType>,

    value: Py<PyAny>,
}

impl PyResMut {
    pub fn new(value: Bound<'_, PyAny>) -> Self {
        Self {
            ty: value.get_type().into(),
            value: value.unbind(),
        }
    }
}

#[pymethods]
impl PyResMut {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        // Create a GenericAlias like types.GenericAlias(ResMut, (key,))
        // This allows the system parameter parser to detect ResMut[T] properly
        let py = cls.py();
        let types_mod = py.import("types")?;
        let generic_alias_cls = types_mod.getattr("GenericAlias")?;

        // Create GenericAlias(ResMut, (key,))
        let args = PyTuple::new(py, [key])?;
        let generic = generic_alias_cls.call1((cls, args))?;

        Ok(generic.unbind())
    }

    /// Proxy attribute access to the wrapped resource
    pub fn __getattr__(&self, py: Python, name: &str) -> PyResult<Py<PyAny>> {
        self.value.bind(py).getattr(name).map(|v| v.unbind())
    }

    /// Proxy attribute setting to the wrapped resource
    pub fn __setattr__(&mut self, py: Python, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        self.value.bind(py).setattr(name, value)
    }
}
