use bevy::render::render_resource::Extent3d;
use pyo3::prelude::*;

#[pyclass(name = "Extent3d", module = "pybevy.render", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyExtent3d {
    pub width: u32,
    pub height: u32,
    pub depth_or_array_layers: u32,
}

impl From<PyExtent3d> for Extent3d {
    fn from(extent: PyExtent3d) -> Self {
        Extent3d {
            width: extent.width,
            height: extent.height,
            depth_or_array_layers: extent.depth_or_array_layers,
        }
    }
}

impl From<Extent3d> for PyExtent3d {
    fn from(extent: Extent3d) -> Self {
        PyExtent3d {
            width: extent.width,
            height: extent.height,
            depth_or_array_layers: extent.depth_or_array_layers,
        }
    }
}

#[pymethods]
impl PyExtent3d {
    #[new]
    pub fn new(width: u32, height: u32, depth_or_array_layers: u32) -> Self {
        PyExtent3d {
            width,
            height,
            depth_or_array_layers,
        }
    }

    #[getter]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[getter]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[getter]
    pub fn depth_or_array_layers(&self) -> u32 {
        self.depth_or_array_layers
    }
}
