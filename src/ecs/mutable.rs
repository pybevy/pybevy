use pyo3::{
    prelude::*,
    types::{PyTuple, PyType},
};

/// Wrapper for mutable access to components and resources
/// Usage: Mut[Transform], Mut[Time], etc.
#[pyclass(name = "Mut")]
pub struct PyMut {
    #[pyo3(get, name = "inner_type")]
    ty: Py<PyType>,

    #[pyo3(get)]
    value: Py<PyAny>,
}

impl PyMut {
    pub fn new(value: Bound<'_, PyAny>) -> Self {
        Self {
            ty: value.get_type().into(),
            value: value.unbind(),
        }
    }
}

#[pymethods]
impl PyMut {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        // Import types.GenericAlias to create a proper generic type
        let py = cls.py();
        let types_mod = py.import("types")?;
        let generic_alias_cls = types_mod.getattr("GenericAlias")?;

        // Create GenericAlias(Mut, (key,))
        // This allows Query[Mut[Transform]] to receive a GenericAlias we can detect
        let args = PyTuple::new(py, [key])?;
        let generic = generic_alias_cls.call1((cls, args))?;

        Ok(generic.unbind())
    }

    /// Proxy attribute access to the wrapped value
    pub fn __getattr__(&self, py: Python, name: &str) -> PyResult<Py<PyAny>> {
        self.value.bind(py).getattr(name).map(|v| v.unbind())
    }

    /// Proxy attribute setting to the wrapped value
    pub fn __setattr__(&mut self, py: Python, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        self.value.bind(py).setattr(name, value)
    }

    /// Return the underlying value without shadowing wrapped `get` methods.
    pub fn unwrap(&self, py: Python) -> PyResult<Py<PyAny>> {
        Ok(self.value.clone_ref(py))
    }
}
