use bevy::{
    color::Color,
    pbr::wireframe::{WireframeMaterial, WireframeTopology},
};
use pybevy_color::color::PyColor;
use pybevy_core::{AssetStorage, PyAsset, ValueStorage};
use pybevy_macros::pyasset;
use pyo3::prelude::*;

use crate::wireframe_topology::PyWireframeTopology;

#[pyasset(WireframeMaterial, bridge, not_loadable)]
#[pyclass(name = "WireframeMaterial", extends = PyAsset, skip_from_py_object)]
#[derive(Debug)]
pub struct PyWireframeMaterial {
    pub storage: AssetStorage<WireframeMaterial>,
}

#[pymethods]
impl PyWireframeMaterial {
    #[new]
    #[pyo3(signature = (
        color = Color::WHITE.into(),
        line_width = 1.0,
        topology = None
    ))]
    pub fn new(
        color: PyColor,
        line_width: f32,
        topology: Option<PyRef<'_, PyWireframeTopology>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let color = color.try_into()?;
        let material = WireframeMaterial {
            color,
            line_width,
            topology: topology
                .map(|topology| topology.0)
                .unwrap_or(WireframeTopology::Triangles),
        };
        Ok(Self::from_owned(material).into())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        let storage: ValueStorage<Color> = self
            .storage
            .borrow_field(|material| &material.color, |material| &mut material.color)?;
        PyColor::from_storage(storage, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = color.try_into()?;
        self.as_mut()?.color = color;
        Ok(())
    }

    #[getter]
    pub fn line_width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.line_width)
    }

    #[setter]
    pub fn set_line_width(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.line_width = value;
        Ok(())
    }

    #[getter]
    pub fn topology(&self, py: Python<'_>) -> PyResult<Py<PyWireframeTopology>> {
        Py::new(py, PyWireframeTopology::from_owned(self.as_ref()?.topology))
    }

    #[setter]
    pub fn set_topology(&mut self, value: PyRef<'_, PyWireframeTopology>) -> PyResult<()> {
        self.as_mut()?.topology = value.0;
        Ok(())
    }
}
