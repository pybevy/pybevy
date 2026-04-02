use bevy::{color::Color, pbr::wireframe::WireframeMaterial};
use pybevy_color::PyColor;
use pybevy_core::{AssetStorage, PyAsset};
use pybevy_macros::asset_storage;
use pyo3::prelude::*;

#[asset_storage(WireframeMaterial, bridge, not_loadable)]
#[pyclass(name = "WireframeMaterial", extends = PyAsset)]
#[derive(Debug)]
pub struct PyWireframeMaterial {
    pub storage: AssetStorage<WireframeMaterial>,
}

#[pymethods]
impl PyWireframeMaterial {
    #[new]
    #[pyo3(signature = (color = Color::WHITE.into()))]
    pub fn new(color: PyColor) -> (Self, PyAsset) {
        let material = WireframeMaterial {
            color: color.into(),
        };
        Self::from_owned(material)
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.color = color.into();
        Ok(())
    }
}
