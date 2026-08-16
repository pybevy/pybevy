use pyo3::{
    IntoPyObjectExt, PyTraverseError, PyVisit, exceptions::PyTypeError, prelude::*, types::PyType,
};

/// Per-system local state, persisted across system invocations.
///
/// `Local[SomeType]` default-constructs one value per system. Mutable values
/// expose their attributes directly; use `current` to read or replace the
/// complete value, including for immutable types.
#[pyclass(name = "Local")]
pub struct PyLocal {
    #[pyo3(get, name = "value_type")]
    pub(crate) ty: Py<PyType>,

    pub(crate) value: Py<PyAny>,
}

#[pymethods]
impl PyLocal {
    /// Report held Python objects to the cyclic GC.
    ///
    /// A Rust-held `Py` reference is invisible to the collector, and user
    /// scene objects reach back here through their defining module's dict, so
    /// without this the cycle is uncollectable and every hot reload leaks a
    /// whole generation. Traverse stays read-only and takes no locks.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.ty)?;
        visit.call(&self.value)
    }

    /// Create a `Local[SomeType]` with a default-constructed value.
    ///
    /// The type must be callable with no arguments (i.e. have a no-arg constructor).
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let ty = key.cast::<PyType>().map_err(|_| {
            PyTypeError::new_err(format!("Local[...] expects a type, got {}", key.get_type()))
        })?;
        let value = key.call0().map_err(|e| {
            PyTypeError::new_err(format!(
                "Local[{}]: type must be callable with no arguments to create a default value ({})",
                ty, e
            ))
        })?;
        Self {
            ty: ty.clone().unbind(),
            value: value.unbind(),
        }
        .into_py_any(cls.py())
    }

    #[new]
    pub fn new(value: Bound<'_, PyAny>) -> Self {
        Self {
            ty: value.get_type().into(),
            value: value.unbind(),
        }
    }

    /// Return the current per-system value.
    #[getter]
    pub fn current(&self, py: Python) -> Py<PyAny> {
        self.value.clone_ref(py)
    }

    /// Replace the current per-system value with another value of the same type.
    #[setter]
    pub fn set_current(&mut self, py: Python, value: Bound<'_, PyAny>) -> PyResult<()> {
        self.set(py, value)
    }

    /// Explicit alias for reading `current`.
    pub fn get(&self, py: Python) -> Py<PyAny> {
        self.current(py)
    }

    /// Explicit alias for replacing `current`.
    pub fn set(&mut self, py: Python, value: Bound<'_, PyAny>) -> PyResult<()> {
        if !value.get_type().is(self.ty.bind(py)) {
            return Err(PyTypeError::new_err(format!(
                "Expected type {}, but got {}",
                self.ty,
                value.get_type()
            )));
        }

        self.value = value.unbind();

        Ok(())
    }

    /// Proxy attribute access to the wrapped value
    pub fn __getattr__(&self, py: Python, name: &str) -> PyResult<Py<PyAny>> {
        self.value.bind(py).getattr(name).map(|v| v.unbind())
    }

    /// Proxy attribute setting to the wrapped value
    pub fn __setattr__(&mut self, py: Python, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        if name == "current" {
            return self.set(py, value);
        }
        self.value.bind(py).setattr(name, value)
    }
}
