use bevy::{color::Color, math::Affine2, sprite_render::ColorMaterial};
use pybevy_color::color::PyColor;
use pybevy_core::{AssetStorage, PyAsset, PyHandle, extract_handle_from_any};
use pybevy_macros::asset_storage;
use pyo3::prelude::*;

use crate::alpha_mode_2d::PyAlphaMode2d;

#[asset_storage(ColorMaterial, bridge)]
#[pyclass(name = "ColorMaterial", extends = PyAsset)]
#[derive(Debug)]
pub struct PyColorMaterial {
    pub(crate) storage: AssetStorage<ColorMaterial>,
}

#[pymethods]
impl PyColorMaterial {
    #[new]
    #[pyo3(signature = (
        color = Color::WHITE.into(),
        texture = None,
        alpha_mode = PyAlphaMode2d::default()
    ))]
    pub fn new(
        color: PyColor,
        texture: Option<&Bound<'_, PyAny>>,
        alpha_mode: PyAlphaMode2d,
    ) -> PyResult<(Self, PyAsset)> {
        let texture_handle = match texture {
            Some(handle_obj) => {
                let handle = extract_handle_from_any(handle_obj)?;
                Some((&handle).try_into()?)
            }
            None => None,
        };

        Ok(Self::from_owned(ColorMaterial {
            color: color.into(),
            texture: texture_handle,
            alpha_mode: alpha_mode.into(),
            uv_transform: Affine2::IDENTITY,
        }))
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
    pub fn texture(&self) -> PyResult<Option<PyHandle>> {
        Ok(self.as_ref()?.texture.as_ref().map(PyHandle::from))
    }

    #[setter]
    pub fn set_texture(&mut self, texture: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.as_mut()?.texture = match texture {
            Some(handle_obj) => {
                let handle = extract_handle_from_any(handle_obj)?;
                Some((&handle).try_into()?)
            }
            None => None,
        };
        Ok(())
    }

    #[getter]
    pub fn alpha_mode(&self, py: Python) -> PyResult<Py<PyAlphaMode2d>> {
        Py::new(py, PyAlphaMode2d::from(self.as_ref()?.alpha_mode))
    }

    #[setter]
    pub fn set_alpha_mode(&mut self, alpha_mode: PyAlphaMode2d) -> PyResult<()> {
        self.as_mut()?.alpha_mode = alpha_mode.into();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let mat = self.as_ref()?;
        Ok(format!(
            "ColorMaterial(color={:?}, texture={:?}, alpha_mode={:?})",
            mat.color,
            mat.texture.is_some(),
            mat.alpha_mode
        ))
    }
}
