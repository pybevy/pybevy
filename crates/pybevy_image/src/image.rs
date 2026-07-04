use std::io::Cursor;

use bevy::{
    asset::RenderAssetUsages,
    image::{Image, TextureFormatPixelInfo},
    render::render_resource::{Extent3d, TextureFormat, TextureUsages},
};
use image::{ImageFormat as RustImageFormat, codecs::jpeg::JpegEncoder};
use numpy::{
    PyArray1, PyArrayMethods, PyReadonlyArray1,
    ndarray::{ArrayView1, ArrayViewMut1},
};
use pybevy_color::color::PyColor;
use pybevy_core::{AssetStorage, PyAsset};
use pybevy_macros::pyasset;
use pybevy_math::{uvec2::PyUVec2, uvec3::PyUVec3, vec2::PyVec2};
use pybevy_wgpu::{
    extent3d::PyExtent3d, texture_dimension::PyTextureDimension, texture_format::PyTextureFormat,
};
use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
};

use crate::{image_format::PyImageFormat, loader_settings::PyImageSampler};

// Convert PyImageFormat to image crate's ImageFormat
fn py_format_to_rust(format: PyImageFormat) -> RustImageFormat {
    match format {
        PyImageFormat::Bmp => RustImageFormat::Bmp,
        PyImageFormat::Dds => RustImageFormat::Dds,
        PyImageFormat::Farbfeld => RustImageFormat::Farbfeld,
        PyImageFormat::Gif => RustImageFormat::Gif,
        PyImageFormat::OpenExr => RustImageFormat::OpenExr,
        PyImageFormat::Hdr => RustImageFormat::Hdr,
        PyImageFormat::Ico => RustImageFormat::Ico,
        PyImageFormat::Jpeg => RustImageFormat::Jpeg,
        PyImageFormat::Ktx2 => RustImageFormat::OpenExr, // KTX2 has no image crate equivalent
        PyImageFormat::Png => RustImageFormat::Png,
        PyImageFormat::Pnm => RustImageFormat::Pnm,
        PyImageFormat::Qoi => RustImageFormat::Qoi,
        PyImageFormat::Tga => RustImageFormat::Tga,
        PyImageFormat::Tiff => RustImageFormat::Tiff,
        PyImageFormat::WebP => RustImageFormat::WebP,
    }
}

#[pyclass(name = "RenderAssetUsages", from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyRenderAssetUsages {
    inner: RenderAssetUsages,
}

impl From<RenderAssetUsages> for PyRenderAssetUsages {
    fn from(usages: RenderAssetUsages) -> Self {
        Self { inner: usages }
    }
}

impl From<PyRenderAssetUsages> for RenderAssetUsages {
    fn from(py_usages: PyRenderAssetUsages) -> Self {
        py_usages.inner
    }
}

impl Default for PyRenderAssetUsages {
    fn default() -> Self {
        Self::new()
    }
}

#[pymethods]
impl PyRenderAssetUsages {
    #[new]
    pub fn new() -> Self {
        Self {
            inner: RenderAssetUsages::default(),
        }
    }

    #[staticmethod]
    #[pyo3(name = "MAIN_WORLD")]
    pub fn main_world_flag() -> Self {
        Self {
            inner: RenderAssetUsages::MAIN_WORLD,
        }
    }

    #[staticmethod]
    #[pyo3(name = "RENDER_WORLD")]
    pub fn render_world_flag() -> Self {
        Self {
            inner: RenderAssetUsages::RENDER_WORLD,
        }
    }

    fn __or__(&self, other: &Self) -> Self {
        Self {
            inner: self.inner | other.inner,
        }
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    pub fn contains(&self, other: &Self) -> bool {
        self.inner.contains(other.inner)
    }

    pub fn __repr__(&self) -> String {
        let mut parts = Vec::new();
        if self.inner.contains(RenderAssetUsages::MAIN_WORLD) {
            parts.push("MAIN_WORLD");
        }
        if self.inner.contains(RenderAssetUsages::RENDER_WORLD) {
            parts.push("RENDER_WORLD");
        }
        if parts.is_empty() {
            "RenderAssetUsages()".to_string()
        } else {
            format!("RenderAssetUsages({})", parts.join(" | "))
        }
    }
}

#[pyclass(name = "ImageDataContext")]
pub struct ImageDataContext {
    array: Py<PyArray1<u8>>,
}

#[pymethods]
impl ImageDataContext {
    fn __enter__(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(slf.array.clone_ref(py).into_any())
    }

    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        // Do nothing - array remains valid as long as Image is alive
        Ok(false) // Don't suppress exceptions
    }
}

#[pyclass(name = "ImageDataContextMut")]
pub struct ImageDataContextMut {
    array: Py<PyArray1<u8>>,
}

#[pymethods]
impl ImageDataContextMut {
    fn __enter__(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(slf.array.clone_ref(py).into_any())
    }

    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        // Do nothing - array remains valid as long as Image is alive
        Ok(false) // Don't suppress exceptions
    }
}

#[pyclass(name = "ImagePixelContextMut")]
pub struct ImagePixelContextMut {
    array: Py<PyArray1<u8>>,
}

#[pymethods]
impl ImagePixelContextMut {
    fn __enter__(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(slf.array.clone_ref(py).into_any())
    }

    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        Ok(false)
    }
}

#[pyasset(Image, bridge)]
#[pyclass(name = "Image", extends = PyAsset, skip_from_py_object)]
#[derive(Debug)]
pub struct PyImage {
    pub storage: AssetStorage<Image>,
}

macro_rules! image_with {
    ($s:expr, $f:expr) => {{ $f($s.as_ref()?) }};
}

macro_rules! image_with_mut {
    ($s:expr, $f:expr) => {{ $f($s.as_mut()?) }};
}

#[pymethods]
impl PyImage {
    #[new]
    #[pyo3(signature = (size=PyExtent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 1
    }, dimension=None, data=None, format=None, asset_usage=None))]
    pub fn new(
        size: PyExtent3d,
        dimension: Option<PyTextureDimension>,
        data: Option<Vec<u8>>,
        format: Option<PyTextureFormat>,
        asset_usage: Option<PyRenderAssetUsages>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let extent: Extent3d = size.into();
        let format: TextureFormat = format.unwrap_or(PyTextureFormat::Rgba8UnormSrgb).into();
        let pixel_count = (extent.width * extent.height * extent.depth_or_array_layers) as usize;
        let data = match data {
            Some(data) => {
                if let Ok(pixel_size) = format.pixel_size() {
                    let expected = pixel_count * pixel_size;
                    if data.len() != expected {
                        return Err(PyValueError::new_err(format!(
                            "data length {} does not match {}x{}x{} with format {:?} (expected {})",
                            data.len(),
                            extent.width,
                            extent.height,
                            extent.depth_or_array_layers,
                            format,
                            expected
                        )));
                    }
                }
                data
            }
            // Default to max-value bytes: white for 8-bit unorm formats
            None => {
                let pixel_size = format.pixel_size().map_err(|_| {
                    PyValueError::new_err(format!(
                        "cannot default-fill data for format {format:?}; pass data explicitly"
                    ))
                })?;
                vec![255u8; pixel_count * pixel_size]
            }
        };

        Ok(Self::from_owned(Image::new(
            extent,
            dimension.unwrap_or(PyTextureDimension::D2).into(),
            data,
            format,
            asset_usage.map(Into::into).unwrap_or_default(),
        )).into())
    }

    #[staticmethod]
    #[pyo3(signature = (size, pixel, format=None, dimension=None))]
    pub fn new_fill(
        py: Python<'_>,
        size: PyExtent3d,
        pixel: Vec<u8>,
        format: Option<PyTextureFormat>,
        dimension: Option<PyTextureDimension>,
    ) -> PyResult<Py<PyImage>> {
        let image = Image::new_fill(
            size.into(),
            dimension.unwrap_or(PyTextureDimension::D2).into(),
            &pixel,
            format.unwrap_or(PyTextureFormat::Rgba8UnormSrgb).into(),
            RenderAssetUsages::default(),
        );

        Py::new(py, Self::from_owned(image))
    }

    #[staticmethod]
    pub fn transparent(py: Python<'_>) -> PyResult<Py<PyImage>> {
        Py::new(py, Self::from_owned(Image::transparent()))
    }

    #[staticmethod]
    pub fn new_target_texture(
        py: Python<'_>,
        width: u32,
        height: u32,
        format: PyTextureFormat,
    ) -> PyResult<Py<PyImage>> {
        let image = Image::new_target_texture(width, height, format.into(), None);

        Py::new(py, Self::from_owned(image))
    }

    /// Create a render target texture with RGBA8 sRGB format and `COPY_SRC` usage.
    ///
    /// This is a convenience method for headless rendering / screenshots.
    /// Use [`new_target_texture`] if you need a custom format.
    #[staticmethod]
    pub fn new_render_target(py: Python<'_>, width: u32, height: u32) -> PyResult<Py<PyImage>> {
        let mut image =
            Image::new_target_texture(width, height, TextureFormat::Rgba8UnormSrgb, None);
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;

        Py::new(py, Self::from_owned(image))
    }

    #[staticmethod]
    #[pyo3(signature = (buffer, is_srgb=true))]
    pub fn from_buffer(py: Python<'_>, buffer: Vec<u8>, is_srgb: bool) -> PyResult<Py<PyImage>> {
        let dyn_img = image::load_from_memory(&buffer)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to load image: {e}")))?;

        let image = Image::from_dynamic(dyn_img, is_srgb, RenderAssetUsages::default());

        Py::new(py, Self::from_owned(image))
    }

    pub fn width(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.width())
    }
    pub fn height(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.height())
    }

    pub fn size(&self) -> PyResult<PyUVec2> {
        Ok(self.as_ref()?.size().into())
    }

    pub fn size_f32(&self) -> PyResult<PyVec2> {
        let size = self.as_ref()?.size();
        Ok(PyVec2::new(size.x as f32, size.y as f32))
    }

    pub fn aspect_ratio(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.aspect_ratio().into())
    }

    pub fn is_compressed(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_compressed())
    }

    #[getter]
    pub fn copy_on_resize(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.copy_on_resize)
    }

    #[setter]
    pub fn set_copy_on_resize(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.copy_on_resize = value;
        Ok(())
    }

    pub fn data_len(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.data.as_ref().map(|d| d.len()).unwrap_or(0))
    }

    pub fn data(&self, py: Python<'_>) -> PyResult<Py<ImageDataContext>> {
        image_with!(self, |image: &Image| {
            let image_data = image
                .data
                .as_ref()
                .ok_or_else(|| PyRuntimeError::new_err("Image has no data"))?;

            let view = ArrayView1::from(&image_data[..]);
            let np_array = unsafe {
                PyArray1::borrow_from_array(&view, py.None().bind(py).clone().into_any())
            }
            .readwrite()
            .make_nonwriteable();

            let context = ImageDataContext {
                array: Bound::clone(&np_array).unbind(),
            };

            Py::new(py, context)
        })
    }

    pub fn data_mut(&mut self, py: Python<'_>) -> PyResult<Py<ImageDataContextMut>> {
        image_with_mut!(self, |image: &mut Image| {
            let image_data = image
                .data
                .as_mut()
                .ok_or_else(|| PyRuntimeError::new_err("Image has no data"))?;

            let view = ArrayViewMut1::from(&mut image_data[..]);
            let np_array = unsafe {
                PyArray1::borrow_from_array(&view, py.None().bind(py).clone().into_any())
            };

            let context = ImageDataContextMut {
                array: np_array.clone().unbind(),
            };

            Py::new(py, context)
        })
    }

    pub fn data_copy(&self, py: Python<'_>) -> PyResult<Py<PyArray1<u8>>> {
        image_with!(self, |image: &Image| {
            let image_data = image
                .data
                .as_ref()
                .ok_or_else(|| PyRuntimeError::new_err("Image has no data"))?;

            let array = PyArray1::from_slice(py, image_data);
            Ok(array.unbind())
        })
    }

    pub fn set_data(&mut self, data: PyReadonlyArray1<u8>) -> PyResult<()> {
        image_with_mut!(self, |image: &mut Image| {
            let data_slice = data.as_slice().map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to access array data: {}", e))
            })?;

            image.data = Some(data_slice.to_vec());
            Ok(())
        })
    }

    pub fn pixel_data_offset(&self, coords: PyUVec3) -> PyResult<Option<usize>> {
        image_with!(self, |image: &Image| {
            Ok(image.pixel_data_offset(coords.into()).ok())
        })
    }

    pub fn pixel_bytes(&self, coords: PyUVec3) -> PyResult<Option<Vec<u8>>> {
        image_with!(self, |image: &Image| {
            Ok(image
                .pixel_bytes(coords.into())
                .ok()
                .map(|bytes| bytes.to_vec()))
        })
    }

    pub fn pixel_bytes_mut(
        &mut self,
        py: Python<'_>,
        coords: PyUVec3,
    ) -> PyResult<Py<ImagePixelContextMut>> {
        image_with_mut!(self, |image: &mut Image| {
            let bevy_coords = coords.into();
            let pixel_bytes = image.pixel_bytes_mut(bevy_coords).map_err(|_| {
                PyRuntimeError::new_err("Invalid pixel coordinates or no image data")
            })?;

            let view = ArrayViewMut1::from(&mut pixel_bytes[..]);
            let np_array = unsafe {
                PyArray1::borrow_from_array(&view, py.None().bind(py).clone().into_any())
            };

            let context = ImagePixelContextMut {
                array: np_array.clone().unbind(),
            };

            Py::new(py, context)
        })
    }

    #[getter]
    pub fn format(&self) -> PyResult<PyTextureFormat> {
        image_with!(self, |image: &Image| {
            Ok(image.texture_descriptor.format.into())
        })
    }

    #[getter]
    pub fn dimension(&self) -> PyResult<PyTextureDimension> {
        image_with!(self, |image: &Image| {
            Ok(image.texture_descriptor.dimension.into())
        })
    }

    #[getter]
    pub fn mip_level_count(&self) -> PyResult<u32> {
        image_with!(self, |image: &Image| {
            Ok(image.texture_descriptor.mip_level_count)
        })
    }

    #[getter]
    pub fn sampler(&self) -> PyResult<PyImageSampler> {
        image_with!(self, |image: &Image| Ok(image.sampler.clone().into()))
    }

    #[setter]
    pub fn set_sampler(&mut self, sampler: PyImageSampler) -> PyResult<()> {
        image_with_mut!(self, |image: &mut Image| {
            image.sampler = sampler.into();
            Ok(())
        })
    }

    #[getter]
    pub fn asset_usage(&self) -> PyResult<PyRenderAssetUsages> {
        image_with!(self, |image: &Image| Ok(image.asset_usage.into()))
    }

    pub fn get_color_at_1d(&self, x: u32, py: Python<'_>) -> PyResult<Py<PyColor>> {
        image_with!(self, |image: &Image| {
            let color = image.get_color_at_1d(x).map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to get color at ({x}): {e}"))
            })?;
            PyColor::from_color(color, py)
        })
    }

    pub fn get_color_at(&self, x: u32, y: u32, py: Python<'_>) -> PyResult<Py<PyColor>> {
        image_with!(self, |image: &Image| {
            let color = image.get_color_at(x, y).map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to get color at ({x}, {y}): {e}"))
            })?;
            PyColor::from_color(color, py)
        })
    }

    pub fn get_color_at_3d(&self, x: u32, y: u32, z: u32, py: Python<'_>) -> PyResult<Py<PyColor>> {
        image_with!(self, |image: &Image| {
            let color = image.get_color_at_3d(x, y, z).map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to get color at ({x}, {y}, {z}): {e}"))
            })?;
            PyColor::from_color(color, py)
        })
    }

    pub fn set_color_at_1d(&mut self, x: u32, color: PyColor) -> PyResult<()> {
        image_with_mut!(self, |image: &mut Image| {
            image
                .set_color_at_1d(x, color.into())
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to set color at ({x}): {e}")))
        })
    }

    pub fn set_color_at(&mut self, x: u32, y: u32, color: PyColor) -> PyResult<()> {
        image_with_mut!(self, |image: &mut Image| {
            image.set_color_at(x, y, color.into()).map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to set color at ({x}, {y}): {e}"))
            })
        })
    }

    pub fn set_color_at_3d(&mut self, x: u32, y: u32, z: u32, color: PyColor) -> PyResult<()> {
        image_with_mut!(self, |image: &mut Image| {
            image.set_color_at_3d(x, y, z, color.into()).map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to set color at ({x}, {y}, {z}): {e}"))
            })
        })
    }

    pub fn resize(&mut self, size: PyExtent3d) -> PyResult<()> {
        image_with_mut!(self, |image: &mut Image| {
            image.resize(size.into());
            Ok(())
        })
    }

    pub fn resize_in_place(&mut self, size: PyExtent3d) -> PyResult<()> {
        image_with_mut!(self, |image: &mut Image| {
            image.resize_in_place(size.into());
            Ok(())
        })
    }

    pub fn reinterpret_size(&mut self, size: PyExtent3d) -> PyResult<()> {
        image_with_mut!(self, |image: &mut Image| {
            let _ = image.reinterpret_size(size.into());
            Ok(())
        })
    }

    pub fn reinterpret_stacked_2d_as_array(&mut self, layers: u32) -> PyResult<()> {
        image_with_mut!(self, |image: &mut Image| {
            let _ = image.reinterpret_stacked_2d_as_array(layers);
            Ok(())
        })
    }

    pub fn clear(&mut self, pixel: Vec<u8>) -> PyResult<()> {
        image_with_mut!(self, |image: &mut Image| {
            image.clear(&pixel);
            Ok(())
        })
    }

    pub fn convert(&self, new_format: PyTextureFormat) -> PyResult<Option<Py<PyImage>>> {
        Python::attach(|py| {
            image_with!(self, |image: &Image| {
                match image.convert(new_format.into()) {
                    Some(converted) => {
                        let py_image = Py::new(py, Self::from_owned(converted))?;
                        Ok(Some(py_image))
                    }
                    None => Ok(None),
                }
            })
        })
    }

    #[pyo3(signature = (format=PyImageFormat::Png, quality=None))]
    pub fn save_to_buffer(&self, format: PyImageFormat, quality: Option<u8>) -> PyResult<Vec<u8>> {
        image_with!(self, |image: &Image| {
            let dynamic_image = image.clone().try_into_dynamic().map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to convert image to dynamic image: {e}"))
            })?;

            let mut buffer = Cursor::new(Vec::new());

            if format == PyImageFormat::Jpeg {
                let quality = quality.unwrap_or(95); // Default quality 95
                let encoder = JpegEncoder::new_with_quality(&mut buffer, quality);
                dynamic_image
                    .write_with_encoder(encoder)
                    .map_err(|e| PyRuntimeError::new_err(format!("Failed to encode JPEG: {e}")))?;
            } else {
                dynamic_image
                    .write_to(&mut buffer, py_format_to_rust(format))
                    .map_err(|e| PyRuntimeError::new_err(format!("Failed to encode image: {e}")))?;
            }

            Ok(buffer.into_inner())
        })
    }

    #[pyo3(signature = (path, format=PyImageFormat::Png, quality=None))]
    pub fn save_to_file(
        &self,
        path: String,
        format: PyImageFormat,
        quality: Option<u8>,
    ) -> PyResult<()> {
        let buffer = self.save_to_buffer(format, quality)?;
        std::fs::write(&path, buffer).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to write image to {path}: {e}"))
        })?;
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        format!("{:?}", self)
    }
}
