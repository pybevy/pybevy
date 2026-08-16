use bevy::pbr::wireframe::WireframeLineWidth;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(WireframeLineWidth, bridge, view_fields = [width])]
#[pyclass(name = "WireframeLineWidth", extends = PyComponent)]
#[derive(Debug)]
pub struct PyWireframeLineWidth {
    pub(crate) storage: ComponentStorage<WireframeLineWidth>,
}

#[pymethods]
impl PyWireframeLineWidth {
    #[new]
    #[pyo3(signature = (width = 1.0))]
    pub fn new(width: f32) -> PyClassInitializer<Self> {
        Self::from_owned(WireframeLineWidth { width }).into()
    }

    #[getter]
    pub fn width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.width)
    }

    #[setter]
    pub fn set_width(&mut self, width: f32) -> PyResult<()> {
        self.as_mut()?.width = width;
        Ok(())
    }
}
