use bevy::{image::Image, prelude::Handle, ui::widget::ImageNode};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::pycomponent;
use pybevy_math::rect::PyRect;
use pyo3::{exceptions::PyTypeError, prelude::*};

use crate::node_image_mode::PyNodeImageMode;

#[pycomponent(ImageNode, bridge)]
#[pyclass(name = "ImageNode", extends = PyComponent)]
#[derive(Clone, Debug)]
pub struct PyImageNode {
    pub(crate) storage: ComponentStorage<ImageNode>,
}

#[pymethods]
impl PyImageNode {
    #[new]
    pub fn new(handle: &Bound<'_, PyAny>) -> PyResult<(Self, PyComponent)> {
        let py_handle = extract_handle_from_any(handle)?;

        if let Some(name) = py_handle.asset_type_name()
            && name != "Image"
        {
            return Err(PyTypeError::new_err(format!(
                "AssetType `{}` does not match expected type `Image`",
                name
            )));
        }

        let bevy_handle: Handle<Image> = py_handle.try_into()?;
        Ok(Self::from_owned(ImageNode::new(bevy_handle)))
    }

    #[staticmethod]
    pub fn solid_color(py: Python, color: PyColor) -> PyResult<Py<Self>> {
        let (obj, base) = Self::from_owned(ImageNode::solid_color(color.into()));
        Py::new(py, (obj, base))
    }

    #[getter]
    pub fn texture(&self) -> PyResult<PyHandle> {
        let handle = &self.as_ref()?.image;
        Ok(handle.into())
    }

    #[setter]
    pub fn set_texture(&mut self, handle: &Bound<'_, PyAny>) -> PyResult<()> {
        let py_handle = extract_handle_from_any(handle)?;

        if let Some(name) = py_handle.asset_type_name()
            && name != "Image"
        {
            return Err(PyTypeError::new_err(format!(
                "AssetType `{}` does not match expected type `Image`",
                name
            )));
        }

        let bevy_handle: Handle<Image> = py_handle.try_into()?;
        self.as_mut()?.image = bevy_handle;
        Ok(())
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
        Ok(self.as_ref()?.rect.map(|r| r.into()))
    }

    #[setter]
    pub fn set_rect(&mut self, value: Option<PyRect>) -> PyResult<()> {
        self.as_mut()?.rect = value.map(|r| r.into());
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

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("ImageNode(texture={:?})", self.as_ref()?.image))
    }
}
