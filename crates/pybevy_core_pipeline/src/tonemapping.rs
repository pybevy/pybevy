use bevy::core_pipeline::tonemapping::Tonemapping;
use pybevy_core::PyComponent;
use pybevy_macros::pywrap;
use pyo3::prelude::*;

#[pywrap(Tonemapping, bridge)]
#[pyclass(name = "Tonemapping", module = "pybevy.core_pipeline", extends = PyComponent, eq, hash, frozen, skip_from_py_object)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyTonemapping(pub(crate) Tonemapping);

#[pymethods]
impl PyTonemapping {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        Self::from_owned(Tonemapping::TonyMcMapface).into()
    }

    #[staticmethod]
    #[pyo3(name = "None_")]
    pub fn none(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::None))
    }

    #[staticmethod]
    #[pyo3(name = "Reinhard")]
    pub fn reinhard(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::Reinhard))
    }

    #[staticmethod]
    #[pyo3(name = "ReinhardLuminance")]
    pub fn reinhard_luminance(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::ReinhardLuminance))
    }

    #[staticmethod]
    #[pyo3(name = "AcesFitted")]
    pub fn aces_fitted(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::AcesFitted))
    }

    #[staticmethod]
    #[pyo3(name = "AgX")]
    pub fn agx(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::AgX))
    }

    #[staticmethod]
    #[pyo3(name = "SomewhatBoringDisplayTransform")]
    pub fn somewhat_boring_display_transform(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(Tonemapping::SomewhatBoringDisplayTransform),
        )
    }

    #[staticmethod]
    #[pyo3(name = "TonyMcMapface")]
    pub fn tony_mc_mapface(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::TonyMcMapface))
    }

    #[staticmethod]
    #[pyo3(name = "BlenderFilmic")]
    pub fn blender_filmic(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::BlenderFilmic))
    }

    #[staticmethod]
    #[pyo3(name = "PbrNeutral")]
    pub fn pbr_neutral(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::KhronosPbrNeutral))
    }

    pub fn is_enabled(&self) -> bool {
        self.0 != Tonemapping::None
    }

    pub fn __repr__(&self) -> &'static str {
        match self.0 {
            Tonemapping::None => "Tonemapping.None_",
            Tonemapping::Reinhard => "Tonemapping.Reinhard",
            Tonemapping::ReinhardLuminance => "Tonemapping.ReinhardLuminance",
            Tonemapping::AcesFitted => "Tonemapping.AcesFitted",
            Tonemapping::AgX => "Tonemapping.AgX",
            Tonemapping::SomewhatBoringDisplayTransform => {
                "Tonemapping.SomewhatBoringDisplayTransform"
            }
            Tonemapping::TonyMcMapface => "Tonemapping.TonyMcMapface",
            Tonemapping::BlenderFilmic => "Tonemapping.BlenderFilmic",
            Tonemapping::KhronosPbrNeutral => "Tonemapping.PbrNeutral",
        }
    }
}
