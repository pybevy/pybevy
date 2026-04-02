use bevy::pbr::ScatteringMedium;
use pybevy_core::{AssetStorage, PyAsset};
use pybevy_macros::asset_storage;
use pyo3::prelude::*;

#[asset_storage(ScatteringMedium, bridge, not_loadable)]
#[pyclass(name = "ScatteringMedium", extends = PyAsset)]
pub struct PyScatteringMedium {
    pub(crate) storage: AssetStorage<ScatteringMedium>,
}

#[pymethods]
impl PyScatteringMedium {
    #[new]
    pub fn new() -> (Self, PyAsset) {
        Self::from_owned(ScatteringMedium::default())
    }
    #[staticmethod]
    #[pyo3(signature = (falloff_resolution = 256, phase_resolution = 256))]
    pub fn earthlike(falloff_resolution: u32, phase_resolution: u32) -> PyResult<Py<Self>> {
        Python::attach(|py| {
            Py::new(
                py,
                Self::from_owned(ScatteringMedium::earthlike(
                    falloff_resolution,
                    phase_resolution,
                )),
            )
        })
    }
}
