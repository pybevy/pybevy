use std::{io::Cursor, sync::Arc};

use bevy::{
    asset::RenderAssetUsages,
    image::{Image, TextureFormatPixelInfo},
    render::render_resource::{Extent3d, TextureFormat, TextureUsages},
};
use image::{ImageFormat as RustImageFormat, codecs::jpeg::JpegEncoder};
use pybevy_array::{BorrowProbe, PyArray, borrowed_mut_u8, borrowed_read_only_u8, owned_u8};
use pybevy_color::color::PyColor;
use pybevy_core::{
    AssetStorage, PyAsset, StorageError, borrowed_array_anchor::AssetBorrowAnchorMut,
    content_hash::CanonicalContentHasher, numpy_view_guard::PyNumpyViewGuard,
};
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

fn image_payload_hash(image: &Image) -> String {
    let descriptor = &image.texture_descriptor;
    let mut hasher = CanonicalContentHasher::new("pybevy.image.payload", 1);
    hasher.write("extent.width", &descriptor.size.width.to_le_bytes());
    hasher.write("extent.height", &descriptor.size.height.to_le_bytes());
    hasher.write(
        "extent.depth_or_array_layers",
        &descriptor.size.depth_or_array_layers.to_le_bytes(),
    );
    hasher.write(
        "dimension",
        format!("{:?}", descriptor.dimension).as_bytes(),
    );
    hasher.write("format", format!("{:?}", descriptor.format).as_bytes());
    match &image.data {
        Some(data) => hasher.write("data.some", data),
        None => hasher.write("data.none", &[]),
    }
    hasher.finish()
}

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
    array: Py<PyArray>,
    anchor: Arc<AssetBorrowAnchorMut>,
}

#[pymethods]
impl ImageDataContext {
    fn __enter__(&self, py: Python<'_>) -> Py<PyArray> {
        self.array.clone_ref(py)
    }

    #[pyo3(signature = (_exc_type=None, _exc_value=None, _traceback=None))]
    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> bool {
        self.anchor.close();
        false
    }
}

#[pyclass(name = "ImageDataContextMut")]
pub struct ImageDataContextMut {
    array: Py<PyArray>,
    anchor: Arc<AssetBorrowAnchorMut>,
}

#[pymethods]
impl ImageDataContextMut {
    fn __enter__(&self, py: Python<'_>) -> Py<PyArray> {
        self.array.clone_ref(py)
    }

    #[pyo3(signature = (_exc_type=None, _exc_value=None, _traceback=None))]
    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> bool {
        self.anchor.close();
        false
    }
}

#[pyclass(name = "ImagePixelContextMut")]
pub struct ImagePixelContextMut {
    array: Py<PyArray>,
    anchor: Arc<AssetBorrowAnchorMut>,
}

#[pymethods]
impl ImagePixelContextMut {
    fn __enter__(&self, py: Python<'_>) -> Py<PyArray> {
        self.array.clone_ref(py)
    }

    #[pyo3(signature = (_exc_type=None, _exc_value=None, _traceback=None))]
    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> bool {
        self.anchor.close();
        false
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
        data: Option<&Bound<'_, PyAny>>,
        format: Option<PyTextureFormat>,
        asset_usage: Option<PyRenderAssetUsages>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let extent: Extent3d = size.into();
        let format: TextureFormat = format.unwrap_or(PyTextureFormat::Rgba8UnormSrgb).into();
        let pixel_count = (extent.width * extent.height * extent.depth_or_array_layers) as usize;
        let data = match data {
            Some(data) => {
                if data
                    .getattr("dtype")
                    .and_then(|dtype| dtype.str())
                    .is_ok_and(|dtype| dtype.to_string() == "bool")
                {
                    let error = pyo3::exceptions::PyTypeError::new_err(
                        "'numpy.bool' object cannot be interpreted as an integer",
                    );
                    let _ = error
                        .value(data.py())
                        .call_method1("add_note", ("while processing 'data'",));
                    return Err(error);
                }
                let data = data.extract::<Vec<u8>>().map_err(|error| {
                    let _ = error
                        .value(data.py())
                        .call_method1("add_note", ("while processing 'data'",));
                    error
                })?;
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
        ))
        .into())
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

    /// Return the versioned SHA-256 digest of the image descriptor and pixel payload.
    pub fn _content_hash(&self) -> PyResult<String> {
        Ok(image_payload_hash(self.as_ref()?))
    }

    pub fn data(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<ImageDataContext>> {
        let this = slf.borrow();
        if !this.storage.view_counters().try_acquire_read() {
            return Err(StorageError::AssetViewsLive.into());
        }
        let guard = PyNumpyViewGuard::from_acquired(
            this.storage.view_counters().reads.clone(),
            slf.clone().unbind().into_any(),
        );
        let validity = this.storage.validity_flag();
        let image = this.storage.as_ref()?;
        let image_data = image
            .data
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("Image has no data"))?;
        let ptr = image_data.as_ptr();
        let len = image_data.len();
        let anchor = Arc::new(AssetBorrowAnchorMut::new(validity, guard));
        let probe: Arc<dyn BorrowProbe> = anchor.clone();
        // SAFETY: the pointer and length come from a live contiguous `Vec<u8>`.
        // The anchor holds the read lease, owner, and validity fence for every
        // operation; the returned storage is read-only.
        let bounded = unsafe { borrowed_read_only_u8(ptr, len, &[len], probe)? };
        let array = Py::new(py, bounded)?;
        Py::new(py, ImageDataContext { array, anchor })
    }

    pub fn data_mut(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<ImageDataContextMut>> {
        let mut this = slf.borrow_mut();
        let writes = this.storage.view_counters().writes.clone();
        let validity = this.storage.validity_flag();
        if !this.storage.view_counters().try_acquire_write() {
            return Err(StorageError::AssetViewsLive.into());
        }
        let guard = PyNumpyViewGuard::from_acquired(writes, slf.clone().unbind().into_any());
        let anchor = Arc::new(AssetBorrowAnchorMut::new(validity, guard));
        let image = this.storage.as_mut_write_leased()?;
        let image_data = image
            .data
            .as_mut()
            .ok_or_else(|| PyRuntimeError::new_err("Image has no data"))?;
        let ptr = image_data.as_mut_ptr();
        let len = image_data.len();
        let probe: Arc<dyn BorrowProbe> = anchor.clone();
        // SAFETY: the pointer is the unique alias obtained under the exclusive
        // write lease. The anchor gates every read/write and blocks mutation or
        // reallocation until it closes.
        let bounded = unsafe { borrowed_mut_u8(ptr, len, &[len], probe)? };
        let array = Py::new(py, bounded)?;
        Py::new(py, ImageDataContextMut { array, anchor })
    }

    pub fn data_copy(&self, py: Python<'_>) -> PyResult<Py<PyArray>> {
        image_with!(self, |image: &Image| {
            let image_data = image
                .data
                .as_ref()
                .ok_or_else(|| PyRuntimeError::new_err("Image has no data"))?;
            let len = image_data.len();
            Py::new(py, owned_u8(image_data.to_vec(), &[len])?)
        })
    }

    pub fn set_data(&mut self, data: Vec<u8>) -> PyResult<()> {
        image_with_mut!(self, |image: &mut Image| {
            let image_data = image
                .data
                .as_mut()
                .ok_or_else(|| PyRuntimeError::new_err("Image has no data"))?;
            if data.len() != image_data.len() {
                return Err(PyValueError::new_err(format!(
                    "pixel data length {} does not match image data length {}",
                    data.len(),
                    image_data.len()
                )));
            }
            image_data.copy_from_slice(&data);
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
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        coords: PyUVec3,
    ) -> PyResult<Py<ImagePixelContextMut>> {
        let mut this = slf.borrow_mut();
        let writes = this.storage.view_counters().writes.clone();
        let validity = this.storage.validity_flag();
        if !this.storage.view_counters().try_acquire_write() {
            return Err(StorageError::AssetViewsLive.into());
        }
        let guard = PyNumpyViewGuard::from_acquired(writes, slf.clone().unbind().into_any());
        let anchor = Arc::new(AssetBorrowAnchorMut::new(validity, guard));
        let image = this.storage.as_mut_write_leased()?;
        let bevy_coords = coords.into();
        let pixel_bytes = image
            .pixel_bytes_mut(bevy_coords)
            .map_err(|_| PyRuntimeError::new_err("Invalid pixel coordinates or no image data"))?;
        let ptr = pixel_bytes.as_mut_ptr();
        let len = pixel_bytes.len();
        let probe: Arc<dyn BorrowProbe> = anchor.clone();
        // SAFETY: this uniquely borrowed pixel subslice remains part of the live
        // image buffer while the exclusive lease is held by the anchor.
        let bounded = unsafe { borrowed_mut_u8(ptr, len, &[len], probe)? };
        let array = Py::new(py, bounded)?;
        Py::new(py, ImagePixelContextMut { array, anchor })
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
