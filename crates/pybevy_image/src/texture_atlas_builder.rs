use bevy::{
    asset::{AssetId, Handle},
    image::{Image, TextureAtlasBuilder},
    math::UVec2,
    render::render_resource::TextureFormat,
};
use pybevy_core::handle::PyHandle;
use pybevy_math::uvec2::PyUVec2;
use pybevy_render::texture_format::PyTextureFormat;
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{
    image::PyImage, texture_atlas_layout::PyTextureAtlasLayout,
    texture_atlas_sources::PyTextureAtlasSources,
};

#[pyclass(
    name = "TextureAtlasBuilder",
    module = "pybevy.image",
    skip_from_py_object
)]
#[derive(Debug)]
pub struct PyTextureAtlasBuilder {
    textures: Vec<(Option<AssetId<Image>>, Image)>,
    initial_size: UVec2,
    max_size: UVec2,
    format: TextureFormat,
    auto_format_conversion: bool,
    padding: UVec2,
}

impl Default for PyTextureAtlasBuilder {
    fn default() -> Self {
        Self {
            textures: Vec::new(),
            initial_size: UVec2::splat(256),
            max_size: UVec2::splat(2048),
            format: TextureFormat::Rgba8UnormSrgb,
            auto_format_conversion: true,
            padding: UVec2::ZERO,
        }
    }
}

#[pymethods]
impl PyTextureAtlasBuilder {
    #[new]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn initial_size<'py>(
        mut slf: PyRefMut<'py, Self>,
        size: PyUVec2,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let size: UVec2 = size.try_into()?;
        if size.x == 0 || size.y == 0 {
            return Err(PyValueError::new_err("atlas initial size must be nonzero"));
        }
        slf.initial_size = size;
        Ok(slf)
    }

    pub fn max_size<'py>(
        mut slf: PyRefMut<'py, Self>,
        size: PyUVec2,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let size: UVec2 = size.try_into()?;
        if size.x == 0 || size.y == 0 {
            return Err(PyValueError::new_err("atlas maximum size must be nonzero"));
        }
        slf.max_size = size;
        Ok(slf)
    }

    pub fn format(mut slf: PyRefMut<'_, Self>, format: PyTextureFormat) -> PyRefMut<'_, Self> {
        slf.format = format.into();
        slf
    }

    pub fn auto_format_conversion(
        mut slf: PyRefMut<'_, Self>,
        auto_format_conversion: bool,
    ) -> PyRefMut<'_, Self> {
        slf.auto_format_conversion = auto_format_conversion;
        slf
    }

    pub fn padding<'py>(
        mut slf: PyRefMut<'py, Self>,
        padding: PyUVec2,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.padding = padding.try_into()?;
        Ok(slf)
    }

    pub fn add_texture<'py>(
        mut slf: PyRefMut<'py, Self>,
        image_id: Option<PyHandle>,
        texture: PyRef<'py, PyImage>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let image_id = image_id
            .map(|handle| TryInto::<Handle<Image>>::try_into(handle).map(|handle| handle.id()))
            .transpose()?;
        let image = (*texture).as_ref()?.clone();
        slf.textures.push((image_id, image));
        Ok(slf)
    }

    pub fn build(
        &self,
        py: Python<'_>,
    ) -> PyResult<(
        Py<PyTextureAtlasLayout>,
        Py<PyTextureAtlasSources>,
        Py<PyImage>,
    )> {
        let mut builder = TextureAtlasBuilder::default();
        builder
            .initial_size(self.initial_size)
            .max_size(self.max_size)
            .format(self.format)
            .auto_format_conversion(self.auto_format_conversion)
            .padding(self.padding);
        for (image_id, image) in &self.textures {
            builder.add_texture(*image_id, image);
        }
        let (layout, sources, image) = builder
            .build()
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        Ok((
            Py::new(py, PyTextureAtlasLayout::from_owned(layout))?,
            Py::new(py, PyTextureAtlasSources::from(sources))?,
            Py::new(py, PyImage::from_owned(image))?,
        ))
    }
}
