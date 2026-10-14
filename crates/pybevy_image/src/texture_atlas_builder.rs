use bevy::{
    asset::{AssetId, Handle},
    image::{Image, TextureAtlasBuilder, TextureFormatPixelInfo},
    math::UVec2,
    render::render_resource::TextureFormat,
};
use pybevy_core::handle::PyHandle;
use pybevy_math::uvec2::PyUVec2;
use pybevy_render::texture_format::PyTextureFormat;
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{
    image::{PyImage, reject_aspect_dependent_format},
    texture_atlas_layout::PyTextureAtlasLayout,
    texture_atlas_sources::PyTextureAtlasSources,
};

const MAX_ATLAS_PIXELS: u64 = 1 << 26;
const MAX_ATLAS_BYTES: u64 = 1 << 28;

fn check_atlas_pixels(size: UVec2) -> PyResult<()> {
    let pixels = u64::from(size.x) * u64::from(size.y);
    if pixels > MAX_ATLAS_PIXELS {
        return Err(PyValueError::new_err(format!(
            "atlas size {size:?} would contain {pixels} pixels; maximum is {MAX_ATLAS_PIXELS}"
        )));
    }
    Ok(())
}

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
        check_atlas_pixels(size)?;
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
        check_atlas_pixels(size)?;
        slf.max_size = size;
        Ok(slf)
    }

    pub fn format(
        mut slf: PyRefMut<'_, Self>,
        format: PyTextureFormat,
    ) -> PyResult<PyRefMut<'_, Self>> {
        // Bevy's pixel_size() unwraps the texel size of these formats in build().
        let format = format.into();
        reject_aspect_dependent_format(format)?;
        slf.format = format;
        Ok(slf)
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
        let bytes_per_pixel =
            self.format
                .pixel_size()
                .map_err(|error| PyValueError::new_err(error.to_string()))? as u64;
        for size in [self.initial_size, self.max_size] {
            let bytes = u64::from(size.x) * u64::from(size.y) * bytes_per_pixel;
            if bytes > MAX_ATLAS_BYTES {
                return Err(PyValueError::new_err(format!(
                    "atlas size {size:?} would allocate {bytes} bytes; maximum is {MAX_ATLAS_BYTES}"
                )));
            }
        }
        for (_, image) in &self.textures {
            if image.width().checked_add(self.padding.x).is_none()
                || image.height().checked_add(self.padding.y).is_none()
            {
                return Err(PyValueError::new_err(
                    "atlas source size plus padding overflows u32",
                ));
            }
        }
        let converted = self
            .textures
            .iter()
            .enumerate()
            .map(|(index, (_, image))| {
                if !self.auto_format_conversion || image.texture_descriptor.format == self.format {
                    return Ok(None);
                }
                image
                    .convert(self.format)
                    .filter(|converted| converted.texture_descriptor.format == self.format)
                    .map(Some)
                    .ok_or_else(|| {
                        PyValueError::new_err(format!(
                            "atlas source {index} cannot be converted from {:?} to {:?}",
                            image.texture_descriptor.format, self.format
                        ))
                    })
            })
            .collect::<PyResult<Vec<_>>>()?;
        let mut builder = TextureAtlasBuilder::default();
        builder
            .initial_size(self.initial_size)
            .max_size(self.max_size)
            .format(self.format)
            .auto_format_conversion(self.auto_format_conversion)
            .padding(self.padding);
        for ((image_id, image), converted_image) in self.textures.iter().zip(&converted) {
            builder.add_texture(*image_id, converted_image.as_ref().unwrap_or(image));
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
