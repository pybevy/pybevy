use pybevy_core::public_error::local_value_type_mismatch;
use pyo3::{
    IntoPyObjectExt, PyTraverseError, PyVisit,
    exceptions::PyTypeError,
    prelude::*,
    types::{PyList, PyType},
};

/// Per-system local state, persisted across system invocations.
///
/// `Local[SomeType]` default-constructs one value per system. Mutable values
/// expose their attributes directly; use `current` to read or replace the
/// complete value, including for immutable types.
///
/// Every hot reload, partial or full, rebuilds the system and constructs the
/// local again. Use a `@resource` for state that must survive one.
#[pyclass(name = "Local", module = "pybevy.ecs", frozen)]
pub struct PyLocal {
    pub(crate) ty: Py<PyType>,

    value_slot: Py<PyList>,

    default_constructed: bool,
}

impl PyLocal {
    fn check_value(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        Self::check_type(self.ty.bind(py), value)
    }

    fn check_type(ty: &Bound<'_, PyType>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        if !value.get_type().is(ty) {
            return Err(PyTypeError::new_err(local_value_type_mismatch(
                ty,
                value.get_type(),
            )));
        }
        Ok(())
    }

    pub(crate) fn fresh_for_registration(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = if self.default_constructed {
            self.ty.bind(py).call0()?.unbind()
        } else {
            let current = self.current(py)?;
            py.import("copy")?
                .call_method1("deepcopy", (current.bind(py),))?
                .unbind()
        };
        self.check_value(py, value.bind(py))?;

        Self {
            ty: self.ty.clone_ref(py),
            value_slot: PyList::new(py, [value])?.unbind(),
            default_constructed: self.default_constructed,
        }
        .into_py_any(py)
    }
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
        visit.call(&self.value_slot)
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
        Self::check_type(ty, &value)?;
        Self {
            ty: ty.clone().unbind(),
            value_slot: PyList::new(cls.py(), [value])?.unbind(),
            default_constructed: true,
        }
        .into_py_any(cls.py())
    }

    #[new]
    pub fn new(value: Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            ty: value.get_type().into(),
            value_slot: PyList::new(value.py(), [value])?.unbind(),
            default_constructed: false,
        })
    }

    /// Return the current per-system value.
    #[getter]
    pub fn current(&self, py: Python) -> PyResult<Py<PyAny>> {
        self.value_slot.bind(py).get_item(0).map(Bound::unbind)
    }

    /// Replace the current per-system value with another value of the same type.
    #[setter]
    pub fn set_current(&self, py: Python, value: Bound<'_, PyAny>) -> PyResult<()> {
        self.check_value(py, &value)?;
        // Keep finalizers outside the container's replacement operation.
        let previous = self.current(py)?;
        self.value_slot.bind(py).set_item(0, value)?;
        drop(previous);
        Ok(())
    }

    /// Proxy attribute access to the wrapped value
    pub fn __getattr__(&self, py: Python, name: &str) -> PyResult<Py<PyAny>> {
        self.current(py)?.bind(py).getattr(name).map(|v| v.unbind())
    }

    /// Proxy attribute setting to the wrapped value
    pub fn __setattr__(&self, py: Python, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        if name == "current" {
            return self.set_current(py, value);
        }
        self.current(py)?.bind(py).setattr(name, value)
    }
}
