use bevy::{color::Color, image::Image, prelude::Handle, ui::widget::ImageNode};
use pybevy_color::color::PyColor;
use pybevy_core::{
    ComponentStorage, FromBorrowedStorage, PyComponent, PyHandle, ensure_asset_type,
    extract_handle_from_any,
};
use pybevy_macros::pycomponent;
use pybevy_math::rect::PyRect;
use pyo3::prelude::*;

use crate::{enums::PyVisualBox, node_image_mode::PyNodeImageMode};

#[pycomponent(ImageNode, bridge)]
#[pyclass(name = "ImageNode", extends = PyComponent)]
#[derive(Debug)]
pub struct PyImageNode {
    pub(crate) storage: ComponentStorage<ImageNode>,
}

#[pymethods]
impl PyImageNode {
    #[new]
    pub fn new(handle: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
        let py_handle = extract_handle_from_any(handle)?;

        ensure_asset_type::<Image>(&py_handle)?;

        let bevy_handle: Handle<Image> = py_handle.try_into()?;
        Ok(Self::from_owned(ImageNode::new(bevy_handle)).into())
    }

    #[staticmethod]
    pub fn solid_color(py: Python, color: PyColor) -> PyResult<Py<Self>> {
        let color = Color::try_from(color)?;
        let (obj, base) = Self::from_owned(ImageNode::solid_color(color));
        Py::new(py, (obj, base))
    }

    #[getter]
    pub fn image(&self) -> PyResult<PyHandle> {
        let handle = &self.as_ref()?.image;
        Ok(handle.into())
    }

    #[setter]
    pub fn set_image(&mut self, handle: &Bound<'_, PyAny>) -> PyResult<()> {
        let py_handle = extract_handle_from_any(handle)?;

        ensure_asset_type::<Image>(&py_handle)?;

        let bevy_handle: Handle<Image> = py_handle.try_into()?;
        self.as_mut()?.image = bevy_handle;
        Ok(())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |node| &node.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = Color::try_from(color)?;
        self.as_mut()?.color = color;
        Ok(())
    }

    #[getter]
    pub fn flip_x(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.flip_x)
    }

    #[setter]
    pub fn set_flip_x(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.flip_x = value;
        Ok(())
    }

    #[getter]
    pub fn flip_y(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.flip_y)
    }

    #[setter]
    pub fn set_flip_y(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.flip_y = value;
        Ok(())
    }

    #[getter]
    pub fn rect(&self) -> PyResult<Option<PyRect>> {
        Ok(self
            .storage
            .borrow_optional_field(|n| &n.rect)?
            .map(<PyRect as FromBorrowedStorage<_>>::from_borrowed))
    }

    #[setter]
    pub fn set_rect(&mut self, value: Option<PyRect>) -> PyResult<()> {
        self.as_mut()?.rect = value.map(TryInto::try_into).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn image_mode(&self) -> PyResult<PyNodeImageMode> {
        Ok(self.as_ref()?.image_mode.clone().into())
    }

    #[setter]
    pub fn set_image_mode(&mut self, value: PyNodeImageMode) -> PyResult<()> {
        self.as_mut()?.image_mode = value.into();
        Ok(())
    }

    #[getter]
    pub fn visual_box(&self) -> PyResult<PyVisualBox> {
        Ok(self.as_ref()?.visual_box.into())
    }

    #[setter]
    pub fn set_visual_box(&mut self, value: PyVisualBox) -> PyResult<()> {
        self.as_mut()?.visual_box = value.into();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("ImageNode(image={:?})", self.as_ref()?.image))
    }
}
