use bevy::ui::ComputedNode;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pycomponent(ComputedNode, no_clone, bridge)]
#[pyclass(name = "ComputedNode", extends = PyComponent)]
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

    #[setter]
    pub fn set_size(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.size = value.into();
        Ok(())
    }

    #[getter]
    pub fn content_size(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|c| &c.content_size)?)
    }

    #[setter]
    pub fn set_content_size(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.content_size = value.into();
        Ok(())
    }

    #[getter]
    pub fn stack_index(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.stack_index)
    }

    #[setter]
    pub fn set_stack_index(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.stack_index = value;
        Ok(())
    }

    #[getter]
    pub fn unrounded_size(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|c| &c.unrounded_size)?)
    }

    #[setter]
    pub fn set_unrounded_size(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.unrounded_size = value.into();
        Ok(())
    }

    #[getter]
    pub fn outline_width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.outline_width)
    }

    #[setter]
    pub fn set_outline_width(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.outline_width = value;
        Ok(())
    }

    #[getter]
    pub fn outline_offset(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.outline_offset)
    }

    #[setter]
    pub fn set_outline_offset(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.outline_offset = value;
        Ok(())
    }

    #[getter]
    pub fn outlined_node_size(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.outlined_node_size().into())
    }

    #[getter]
    pub fn inverse_scale_factor(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.inverse_scale_factor)
    }

    #[setter]
    pub fn set_inverse_scale_factor(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.inverse_scale_factor = value;
        Ok(())
    }

    #[getter]
    pub fn scrollbar_size(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|c| &c.scrollbar_size)?)
    }

    #[setter]
    pub fn set_scrollbar_size(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.scrollbar_size = value.into();
        Ok(())
    }

    #[getter]
    pub fn scroll_position(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|c| &c.scroll_position)?)
    }

    #[setter]
    pub fn set_scroll_position(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.scroll_position = value.into();
        Ok(())
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_empty())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let inner = self.as_ref()?;
        Ok(format!(
            "ComputedNode(size={:?}, stack_index={})",
            inner.size, inner.stack_index
        ))
    }
}
