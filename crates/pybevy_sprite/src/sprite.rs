use bevy::{
    asset::AssetId,
    image::Image,
    sprite::{Sprite, SpriteImageMode},
};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent, PyHandle, extract_handle_from_any};
use pybevy_image::texture_atlas::PyTextureAtlas;
use pybevy_macros::pycomponent;
use pybevy_math::{rect::PyRect, vec2::PyVec2};
use pyo3::prelude::*;

use crate::sprite_image_mode::PySpriteImageMode;

#[pycomponent(Sprite, bridge, view_fields = [flip_x, flip_y], batch_only_fields = [color])]
#[pyclass(name = "Sprite", extends = PyComponent)]
#[derive(Debug)]
pub struct PySprite {
    pub(crate) storage: ComponentStorage<Sprite>,
}

#[pymethods]
impl PySprite {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        image,
        color = PyColor::default(),
        flip_x = false,
        flip_y = false,
        custom_size = None,
        rect = None,
        texture_atlas = None,
        image_mode = PySpriteImageMode::default()
    ))]
    pub fn new(
        image: &Bound<'_, PyAny>,
        color: PyColor,
        flip_x: bool,
        flip_y: bool,
        custom_size: Option<(f32, f32)>,
        rect: Option<PyRect>,
        texture_atlas: Option<PyTextureAtlas>,
        image_mode: PySpriteImageMode,
    ) -> PyResult<PyClassInitializer<Self>> {
        let py_handle = extract_handle_from_any(image)?;
        let image_handle = py_handle.try_into()?;
        let texture_atlas_handle = texture_atlas.map(|ta| ta.try_into()).transpose()?;
        let custom_size_vec = custom_size.map(From::from);
        let rect_bevy = rect.map(|r| r.into());

        Ok(Self::from_owned(Sprite {
            image: image_handle,
            color: color.into(),
            flip_x,
            flip_y,
            custom_size: custom_size_vec,
            rect: rect_bevy,
            texture_atlas: texture_atlas_handle,
            image_mode: image_mode.into(),
        })
        .into())
    }

    #[staticmethod]
    pub fn from_image(image: &Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        Python::attach(|py| {
            let py_handle = extract_handle_from_any(image)?;
            let image_handle = py_handle.try_into()?;
            Py::new(
                py,
                Self::from_owned(Sprite {
                    image: image_handle,
                    color: PyColor::default().into(),
                    flip_x: false,
                    flip_y: false,
                    custom_size: None,
                    rect: None,
                    texture_atlas: None,
                    image_mode: SpriteImageMode::Auto,
                }),
            )
        })
    }

    #[staticmethod]
    pub fn from_atlas_image(image: &Bound<'_, PyAny>, atlas: PyTextureAtlas) -> PyResult<Py<Self>> {
        Python::attach(|py| {
            let py_handle = extract_handle_from_any(image)?;
            let image_handle = py_handle.try_into()?;
            let atlas_handle = atlas.try_into()?;
            Py::new(
                py,
                Self::from_owned(Sprite {
                    image: image_handle,
                    color: PyColor::default().into(),
                    flip_x: false,
                    flip_y: false,
                    custom_size: None,
                    rect: None,
                    texture_atlas: Some(atlas_handle),
                    image_mode: SpriteImageMode::Auto,
                }),
            )
        })
    }

    #[staticmethod]
    pub fn from_color(color: PyColor, size: PyVec2) -> PyResult<Py<Self>> {
        let size_vec: bevy::math::Vec2 = size.into();
        Python::attach(|py| {
            Py::new(
                py,
                Self::from_owned(Sprite {
                    image: Default::default(),
                    color: color.into(),
                    flip_x: false,
                    flip_y: false,
                    custom_size: Some(size_vec),
                    rect: None,
                    texture_atlas: None,
                    image_mode: SpriteImageMode::Auto,
                }),
            )
        })
    }

    #[staticmethod]
    pub fn sized(custom_size: PyVec2) -> PyResult<Py<Self>> {
        let size_vec: bevy::math::Vec2 = custom_size.into();
        Python::attach(|py| {
            Py::new(
                py,
                Self::from_owned(Sprite {
                    image: Default::default(),
                    color: PyColor::default().into(),
                    flip_x: false,
                    flip_y: false,
                    custom_size: Some(size_vec),
                    rect: None,
                    texture_atlas: None,
                    image_mode: SpriteImageMode::Auto,
                }),
            )
        })
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
    pub fn image(&self) -> PyResult<PyHandle> {
        Ok(PyHandle::from(&self.as_ref()?.image))
    }

    #[setter]
    pub fn set_image(&mut self, image: &Bound<'_, PyAny>) -> PyResult<()> {
        let py_handle = extract_handle_from_any(image)?;
        self.as_mut()?.image = py_handle.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn image_mode(&self) -> PyResult<PySpriteImageMode> {
        Ok(PySpriteImageMode::from(self.as_ref()?.image_mode.clone()))
    }

    #[setter]
    pub fn set_image_mode(&mut self, image_mode: PySpriteImageMode) -> PyResult<()> {
        self.as_mut()?.image_mode = SpriteImageMode::from(image_mode);
        Ok(())
    }

    #[getter]
    pub fn rect(&self) -> PyResult<Option<PyRect>> {
        Ok(self.as_ref()?.rect.map(|r| r.into()))
    }

    #[setter]
    pub fn set_rect(&mut self, rect: Option<PyRect>) -> PyResult<()> {
        self.as_mut()?.rect = rect.map(|r| r.into());
        Ok(())
    }

    #[getter]
    pub fn texture_atlas(&self) -> PyResult<Option<PyTextureAtlas>> {
        Ok(self
            .storage
            .borrow_optional_field(|s| &s.texture_atlas)?
            .map(PyTextureAtlas::from_borrowed_storage))
    }

    #[setter]
    pub fn set_texture_atlas(&mut self, texture_atlas: Option<PyTextureAtlas>) -> PyResult<()> {
        self.as_mut()?.texture_atlas = texture_atlas.map(|ta| ta.try_into()).transpose()?;
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
    pub fn custom_size(&self) -> PyResult<Option<(f32, f32)>> {
        Ok(self.as_ref()?.custom_size.map(|v| (v.x, v.y)))
    }

    #[setter]
    pub fn set_custom_size(&mut self, value: Option<(f32, f32)>) -> PyResult<()> {
        self.as_mut()?.custom_size = value.map(From::from);
        Ok(())
    }

    pub fn as_asset_id(&self) -> PyResult<PyHandle> {
        let asset_id: AssetId<Image> = self.as_ref()?.image.id();
        Ok(PyHandle::from(&asset_id))
    }
}
