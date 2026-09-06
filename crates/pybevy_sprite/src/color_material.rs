use bevy::{color::Color, math::Affine2, sprite_render::ColorMaterial};
use pybevy_color::color::PyColor;
use pybevy_core::{AssetStorage, PyAsset, PyHandle, ValueStorage, extract_handle_from_any};
use pybevy_macros::pyasset;
use pybevy_math::affine2::PyAffine2;
use pyo3::prelude::*;

use crate::alpha_mode_2d::PyAlphaMode2d;

#[pyasset(ColorMaterial, bridge)]
#[pyclass(name = "ColorMaterial", module = "pybevy.sprite", extends = PyAsset, skip_from_py_object)]
#[derive(Debug)]
pub struct PyColorMaterial {
    pub(crate) storage: AssetStorage<ColorMaterial>,
}

#[pymethods]
impl PyColorMaterial {
    #[new]
    #[pyo3(signature = (
        color = Color::WHITE.into(),
        alpha_mode = PyAlphaMode2d::Blend(),
        uv_transform = PyAffine2::IDENTITY,
        texture = None
    ))]
    pub fn new(
        color: PyColor,
        alpha_mode: PyAlphaMode2d,
        uv_transform: PyAffine2,
        texture: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let texture_handle = match texture {
            Some(handle_obj) => {
                let handle = extract_handle_from_any(handle_obj)?;
                Some((&handle).try_into()?)
            }
            None => None,
        };

        let color = color.try_into()?;
        Ok(Self::from_owned(ColorMaterial {
            color,
            alpha_mode: alpha_mode.into(),
            uv_transform: uv_transform.try_into()?,
            texture: texture_handle,
        })
        .into())
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

    #[getter]
    pub fn uv_transform(&self) -> PyResult<PyAffine2> {
        Ok(self.storage.borrow_field_as(
            |material| &material.uv_transform,
            |material| &mut material.uv_transform,
        )?)
    }

    #[setter]
    pub fn set_uv_transform(&mut self, transform: PyAffine2) -> PyResult<()> {
        let transform: Affine2 = transform.try_into()?;
        self.as_mut()?.uv_transform = transform;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let mat = self.as_ref()?;
        Ok(format!(
            "ColorMaterial(color={:?}, alpha_mode={:?}, uv_transform={:?}, texture={:?})",
            mat.color,
            mat.alpha_mode,
            mat.uv_transform,
            mat.texture.is_some()
        ))
    }
}
