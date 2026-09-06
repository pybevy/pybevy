use bevy::ecs::resource::IsResource;
pub use pybevy_core::PyResource;
/// Resource-entity guards are shared with the control plane, which cannot
/// depend on this crate.
pub(crate) use pybevy_core::custom_resource::{
    hierarchy_contains_resource_entity, is_resource_entity,
};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::{
    PyTraverseError, PyTypeInfo, PyVisit,
    exceptions::PyTypeError,
    prelude::*,
    types::{PyList, PyTuple, PyType},
};

use crate::ecs::component::PyComponentId;

#[pycomponent(IsResource, no_clone, no_insert, bridge)]
#[pyclass(name = "IsResource", module = "pybevy.ecs", extends = PyComponent, frozen)]
pub struct PyIsResource {
    storage: ComponentStorage<IsResource>,
}

#[pymethods]
impl PyIsResource {
    #[getter]
    pub fn resource_component_id(&self) -> PyResult<PyComponentId> {
        Ok(PyComponentId(self.as_ref()?.resource_component_id()))
    }
}

/// Descriptor returned by `Res[T]` or `ResMut[T]`.
///
/// Provides `__origin__` and `__args__` for annotation parser compatibility,
/// and `__repr__` using `T.__name__` (not `__qualname__`) for clean display.
#[pyclass(name = "ResParam", frozen)]
pub struct PyResParam {
    mutable: bool,
    type_obj: Py<PyAny>,
    type_name: String,
}

#[pymethods]
impl PyResParam {
    /// Report held Python objects to the cyclic GC.
    ///
    /// A Rust-held `Py` reference is invisible to the collector, and user
    /// scene objects reach back here through their defining module's dict, so
    /// without this the cycle is uncollectable and every hot reload leaks a
    /// whole generation. Traverse stays read-only and takes no locks.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.type_obj)
    }

    #[getter]
    pub fn __origin__(&self, py: Python) -> Py<PyAny> {
        if self.mutable {
            PyResMut::type_object(py).into_any().unbind()
        } else {
            PyRes::type_object(py).into_any().unbind()
        }
    }

    #[getter]
    pub fn __args__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, [self.type_obj.bind(py)])
    }

    pub fn __repr__(&self) -> String {
        if self.mutable {
            format!("ResMut[{}]", self.type_name)
        } else {
            format!("Res[{}]", self.type_name)
        }
    }
}

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
    /// Report held Python objects to the cyclic GC; see docs/safety.md.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.ty)?;
        visit.call(&self.value)
    }

    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        _cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = _cls.py();
        let type_name = key
            .getattr("__name__")
            .and_then(|n| n.extract::<String>())
            .unwrap_or_else(|_| key.repr().map(|r| r.to_string()).unwrap_or_default());
        Py::new(
            py,
            PyResParam {
                mutable: false,
                type_obj: key.clone().unbind(),
                type_name,
            },
        )
        .map(|p| p.into_any())
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

    /// Expose wrapped resource attrs to dir() so REPL/IDE introspection works.
    pub fn __dir__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let inner = self.value.bind(py);
        let mut names = py
            .import("builtins")?
            .getattr("dir")?
            .call1((inner,))?
            .extract::<Vec<String>>()?;
        names.push("resource_type".into());
        PyList::new(py, names)
    }

    pub fn __repr__(&self, py: Python) -> PyResult<String> {
        Ok(format!("Res({})", self.value.bind(py).repr()?))
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
    /// Report held Python objects to the cyclic GC; see docs/safety.md.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.ty)?;
        visit.call(&self.value)
    }

    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        _cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = _cls.py();
        let type_name = key
            .getattr("__name__")
            .and_then(|n| n.extract::<String>())
            .unwrap_or_else(|_| key.repr().map(|r| r.to_string()).unwrap_or_default());
        Py::new(
            py,
            PyResParam {
                mutable: true,
                type_obj: key.clone().unbind(),
                type_name,
            },
        )
        .map(|p| p.into_any())
    }

    /// Proxy attribute access to the wrapped resource
    pub fn __getattr__(&self, py: Python, name: &str) -> PyResult<Py<PyAny>> {
        self.value.bind(py).getattr(name).map(|v| v.unbind())
    }

    /// Proxy attribute setting to the wrapped resource
    pub fn __setattr__(&mut self, py: Python, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        self.value.bind(py).setattr(name, value)
    }

    /// Expose wrapped resource attrs to dir() so REPL/IDE introspection works.
    pub fn __dir__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let inner = self.value.bind(py);
        let mut names = py
            .import("builtins")?
            .getattr("dir")?
            .call1((inner,))?
            .extract::<Vec<String>>()?;
        names.push("resource_type".into());
        PyList::new(py, names)
    }

    pub fn __repr__(&self, py: Python) -> PyResult<String> {
        Ok(format!("ResMut({})", self.value.bind(py).repr()?))
    }
}
