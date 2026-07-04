use bevy::core_pipeline::tonemapping::Tonemapping;
use pybevy_core::PyComponent;
use pybevy_macros::pywrap;
use pyo3::prelude::*;

#[pywrap(Tonemapping, bridge)]
#[pyclass(name = "Tonemapping", extends = PyComponent, frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub struct PyTonemapping(pub(crate) Tonemapping);

#[pymethods]
impl PyTonemapping {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        Self::from_owned(Tonemapping::TonyMcMapface).into()
    }

    #[classattr]
    #[pyo3(name = "NONE")]
    pub fn none(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::None))
    }

    #[classattr]
    #[pyo3(name = "REINHARD")]
    pub fn reinhard(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::Reinhard))
    }

    #[classattr]
    #[pyo3(name = "REINHARD_LUMINANCE")]
    pub fn reinhard_luminance(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::ReinhardLuminance))
    }

    #[classattr]
    #[pyo3(name = "ACES_FITTED")]
    pub fn aces_fitted(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::AcesFitted))
    }

    #[classattr]
    #[pyo3(name = "AGX")]
    pub fn agx(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::AgX))
    }

    #[classattr]
    #[pyo3(name = "SOMEWHAT_BORING_DISPLAY_TRANSFORM")]
    pub fn somewhat_boring_display_transform(py: Python) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(Tonemapping::SomewhatBoringDisplayTransform),
        )
    }

    #[classattr]
    #[pyo3(name = "TONY_MC_MAPFACE")]
    pub fn tony_mc_mapface(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::TonyMcMapface))
    }

    #[classattr]
    #[pyo3(name = "BLENDER_FILMIC")]
    pub fn blender_filmic(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::BlenderFilmic))
    }

    #[classattr]
    #[pyo3(name = "PBR_NEUTRAL")]
    pub fn pbr_neutral(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Tonemapping::KhronosPbrNeutral))
    }

    pub fn __repr__(&self) -> &'static str {
        match self.0 {
            Tonemapping::None => "Tonemapping.NONE",
            Tonemapping::Reinhard => "Tonemapping.REINHARD",
            Tonemapping::ReinhardLuminance => "Tonemapping.REINHARD_LUMINANCE",
            Tonemapping::AcesFitted => "Tonemapping.ACES_FITTED",
            Tonemapping::AgX => "Tonemapping.AGX",
            Tonemapping::SomewhatBoringDisplayTransform => {
                "Tonemapping.SOMEWHAT_BORING_DISPLAY_TRANSFORM"
            }
            Tonemapping::TonyMcMapface => "Tonemapping.TONY_MC_MAPFACE",
            Tonemapping::BlenderFilmic => "Tonemapping.BLENDER_FILMIC",
            Tonemapping::KhronosPbrNeutral => "Tonemapping.PBR_NEUTRAL",
        }
    }
}
