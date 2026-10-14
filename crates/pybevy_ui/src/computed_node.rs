use bevy::{
    text::{EmSize, RemSize},
    ui::ComputedNode,
};
use pybevy_core::{
    ComponentStorage, PyComponent, computed_owned,
    domain::{FINITE, check_for},
    repr::ReprLiteral,
};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pycomponent(ComputedNode, bridge, domains = [outline_width => [finite], outline_offset => [finite], inverse_scale_factor => [finite]])]
#[pyclass(name = "ComputedNode", module = "pybevy.ui", extends = PyComponent)]
#[derive(Debug)]
pub struct PyComputedNode {
    pub(crate) storage: ComponentStorage<ComputedNode>,
}

#[pymethods]
impl PyComputedNode {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        Self::from_owned(ComputedNode::default()).into()
    }

    #[getter]
    pub fn size(&self) -> PyResult<PyVec2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|c| &c.size, |c| &mut c.size)?)
    }

    #[setter]
    pub fn set_size(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.size = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn content_size(&self) -> PyResult<PyVec2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|c| &c.content_size, |c| &mut c.content_size)?)
    }

    #[setter]
    pub fn set_content_size(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.content_size = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn unrounded_size(&self) -> PyResult<PyVec2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|c| &c.unrounded_size, |c| &mut c.unrounded_size)?)
    }

    #[setter]
    pub fn set_unrounded_size(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.unrounded_size = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn outline_width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.outline_width)
    }

    #[setter]
    pub fn set_outline_width(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_outline_width(value)?;
        self.as_mut()?.outline_width = value;
        Ok(())
    }

    #[getter]
    pub fn outline_offset(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.outline_offset)
    }

    #[setter]
    pub fn set_outline_offset(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_outline_offset(value)?;
        self.as_mut()?.outline_offset = value;
        Ok(())
    }

    #[getter]
    pub fn outlined_node_size(&self) -> PyResult<PyVec2> {
        Ok(computed_owned(self.as_ref()?.outlined_node_size().into()))
    }

    #[getter]
    pub fn inverse_scale_factor(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.inverse_scale_factor)
    }

    #[setter]
    pub fn set_inverse_scale_factor(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_inverse_scale_factor(value)?;
        self.as_mut()?.inverse_scale_factor = value;
        Ok(())
    }

    #[getter]
    pub fn em_size(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.em_size.0)
    }

    #[setter]
    pub fn set_em_size(&mut self, value: f32) -> PyResult<()> {
        let value = check_for::<Self, _>("em_size", value, FINITE)?;
        self.as_mut()?.em_size = EmSize(value);
        Ok(())
    }

    #[getter]
    pub fn rem_size(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.rem_size.0)
    }

    #[setter]
    pub fn set_rem_size(&mut self, value: f32) -> PyResult<()> {
        let value = check_for::<Self, _>("rem_size", value, FINITE)?;
        self.as_mut()?.rem_size = RemSize(value);
        Ok(())
    }

    #[getter]
    pub fn scrollbar_size(&self) -> PyResult<PyVec2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|c| &c.scrollbar_size, |c| &mut c.scrollbar_size)?)
    }

    #[setter]
    pub fn set_scrollbar_size(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.scrollbar_size = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn scroll_position(&self) -> PyResult<PyVec2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|c| &c.scroll_position, |c| &mut c.scroll_position)?)
    }

    #[setter]
    pub fn set_scroll_position(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.scroll_position = value.try_into()?;
        Ok(())
    }
    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_empty())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let inner = self.as_ref()?;
        Ok(format!("ComputedNode(size={})", inner.size.repr_literal()))
    }
}
