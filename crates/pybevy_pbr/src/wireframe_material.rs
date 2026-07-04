use bevy::{color::Color, pbr::wireframe::WireframeMaterial};
use pybevy_color::color::PyColor;
use pybevy_core::{AssetStorage, PyAsset};
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
        topology = PyWireframeTopology::Triangles
    ))]
    pub fn new(color: PyColor, line_width: f32, topology: PyWireframeTopology) -> PyClassInitializer<Self> {
        let material = WireframeMaterial {
            color: color.into(),
            line_width,
            topology: topology.into(),
        };
        Self::from_owned(material).into()
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
    pub fn topology(&self) -> PyResult<PyWireframeTopology> {
        Ok(self.as_ref()?.topology.into())
    }

    #[setter]
    pub fn set_topology(&mut self, value: PyWireframeTopology) -> PyResult<()> {
        self.as_mut()?.topology = value.into();
        Ok(())
    }
}
