use pyo3::{
    PyTraverseError, PyVisit,
    prelude::*,
    types::{PyTuple, PyType},
};

/// Wrapper for mutable access to components and resources
/// Usage: Mut[Transform], Mut[Time], etc.
#[pyclass(name = "Mut", module = "pybevy.ecs")]
pub struct PyMut {
    ty: Py<PyType>,
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
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.ty)?;
        visit.call(&self.value)
    }

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

    pub fn __len__(&self, py: Python<'_>) -> PyResult<usize> {
        self.value.bind(py).len()
    }

    pub fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.value.bind(py).try_iter()?.into_any().unbind())
    }

    /// Return the underlying value, as Bevy's `Mut::into_inner` does.
    #[allow(
        clippy::wrong_self_convention,
        reason = "Python extraction keeps the wrapper reusable"
    )]
    pub fn into_inner(&self, py: Python) -> PyResult<Py<PyAny>> {
        Ok(self.value.clone_ref(py))
    }
}
