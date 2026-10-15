use bevy::render::settings::{RenderCreation, WgpuSettings};
use pybevy_core::public_error::RENDER_CREATION_MANUAL_UNSUPPORTED;
use pybevy_macros::pyenum;
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::wgpu_settings::{PyWgpuSettings, wgpu_settings_repr};

#[pyenum(RenderCreation, manual)]
#[pyclass(
    name = "RenderCreation",
    module = "pybevy.render",
    subclass,
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRenderCreation {
    settings: WgpuSettings,
}

impl TryFrom<RenderCreation> for PyRenderCreation {
    type Error = PyErr;

    fn try_from(creation: RenderCreation) -> PyResult<Self> {
        match creation {
            RenderCreation::Automatic(settings) => Ok(Self {
                settings: *settings,
            }),
            RenderCreation::Manual(_) => {
                Err(PyValueError::new_err(RENDER_CREATION_MANUAL_UNSUPPORTED))
            }
        }
    }
}

impl From<&PyRenderCreation> for RenderCreation {
    fn from(creation: &PyRenderCreation) -> Self {
        RenderCreation::Automatic(Box::new(creation.settings.clone()))
    }
}

impl PyRenderCreation {
    pub fn automatic(settings: WgpuSettings) -> Self {
        Self { settings }
    }

    pub fn into_object(self, py: Python<'_>) -> PyResult<Py<Self>> {
        let value = Py::new(
            py,
            PyClassInitializer::from(self).add_subclass(PyRenderCreationAutomatic),
        )?;
        Ok(value.into_bound(py).into_super().unbind())
    }
}

#[pymethods]
impl PyRenderCreation {
    pub fn __repr__(&self) -> String {
        format!(
            "RenderCreation.Automatic({})",
            wgpu_settings_repr(&self.settings)
        )
    }
}

#[pyclass(
    name = "Automatic",
    module = "pybevy.render",
    extends = PyRenderCreation,
    frozen
)]
pub struct PyRenderCreationAutomatic;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyRenderCreationAutomatic {
    #[classattr]
    const __qualname__: &'static str = "RenderCreation.Automatic";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }

    #[new]
    pub fn new(value: PyRef<'_, PyWgpuSettings>) -> PyResult<PyClassInitializer<Self>> {
        let value = Box::new(WgpuSettings::try_from(&*value)?);
        let creation = PyRenderCreation::try_from(RenderCreation::Automatic(value))?;
        Ok(PyClassInitializer::from(creation).add_subclass(Self))
    }

    #[getter]
    pub fn value(slf: PyRef<'_, Self>) -> PyWgpuSettings {
        PyWgpuSettings::read_only_snapshot(&slf.as_super().settings)
    }
}

pub fn register_render_creation_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    module
        .getattr("RenderCreation")?
        .setattr("Automatic", py.get_type::<PyRenderCreationAutomatic>())?;
    Ok(())
}
