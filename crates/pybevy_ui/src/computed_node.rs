use bevy::ui::ComputedNode;
use pybevy_core::{ComponentStorage, PyComponent, computed_owned};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pycomponent(ComputedNode, no_clone, bridge)]
#[pyclass(name = "ComputedNode", module = "pybevy.ui", extends = PyComponent)]
#[derive(Debug)]
pub struct PyComputedNode {
    pub(crate) storage: ComponentStorage<ComputedNode>,
}

#[pymethods]
impl PyComputedNode {
    #[getter]
    pub fn size(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|c| &c.size)?)
    }

    #[getter]
    pub fn content_size(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|c| &c.content_size)?)
    }

    #[getter]
    pub fn unrounded_size(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|c| &c.unrounded_size)?)
    }

    #[getter]
    pub fn outline_width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.outline_width)
    }

    #[getter]
    pub fn outline_offset(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.outline_offset)
    }

    #[getter]
    pub fn outlined_node_size(&self) -> PyResult<PyVec2> {
        Ok(computed_owned(self.as_ref()?.outlined_node_size().into()))
    }

    #[getter]
    pub fn inverse_scale_factor(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.inverse_scale_factor)
    }

    #[getter]
    pub fn scrollbar_size(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|c| &c.scrollbar_size)?)
    }

    #[getter]
    pub fn scroll_position(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|c| &c.scroll_position)?)
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_empty())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let inner = self.as_ref()?;
        Ok(format!("ComputedNode(size={:?})", inner.size))
    }
}
