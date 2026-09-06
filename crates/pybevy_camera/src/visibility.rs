use bevy::camera::visibility::Visibility;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::visibility_batch::PyVisibilityBatch;

#[pycomponent(Visibility, bridge)]
#[pyclass(name = "Visibility", module = "pybevy.camera", extends = PyComponent, eq)]
#[derive(Debug)]
pub struct PyVisibility {
    pub(crate) storage: ComponentStorage<Visibility>,
}

impl PartialEq for PyVisibility {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

#[pymethods]
impl PyVisibility {
    #[staticmethod]
    #[pyo3(name = "Inherited")]
    pub fn inherited(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, (Visibility::Inherited.into(), PyComponent))
    }

    #[staticmethod]
    #[pyo3(name = "Visible")]
    pub fn visible(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, (Visibility::Visible.into(), PyComponent))
    }

    #[staticmethod]
    #[pyo3(name = "Hidden")]
    pub fn hidden(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, (Visibility::Hidden.into(), PyComponent))
    }

    #[staticmethod]
    pub fn from_numpy(py: Python, visibility: Py<PyAny>) -> PyResult<Py<PyAny>> {
        // Accept real NumPy, the bounded `pybevy.array` array (via `__array__`),
        // and (nested) lists of truthy values, matching every other from_numpy.
        let array = py.import("numpy")?.call_method1("asarray", (visibility,))?;
        let batch = PyVisibilityBatch::new(array.unbind());
        Ok(Py::new(py, batch)?.into_any())
    }

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (Visibility::Inherited.into(), PyComponent).into()
    }

    pub fn toggle_inherited_visible(&mut self) -> PyResult<()> {
        self.as_mut()?.toggle_inherited_visible();
        Ok(())
    }

    pub fn toggle_inherited_hidden(&mut self) -> PyResult<()> {
        self.as_mut()?.toggle_inherited_hidden();
        Ok(())
    }

    pub fn toggle_visible_hidden(&mut self) -> PyResult<()> {
        self.as_mut()?.toggle_visible_hidden();
        Ok(())
    }

    pub fn set_visible(&mut self) -> PyResult<()> {
        *self.as_mut()? = Visibility::Visible;
        Ok(())
    }

    pub fn set_hidden(&mut self) -> PyResult<()> {
        *self.as_mut()? = Visibility::Hidden;
        Ok(())
    }

    pub fn set_inherited(&mut self) -> PyResult<()> {
        *self.as_mut()? = Visibility::Inherited;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("Visibility.{:?}", self.as_ref()?))
    }
}
