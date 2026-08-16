use std::{io::Cursor, sync::Arc};

use bevy::{
    asset::RenderAssetUsages,
    image::{Image, ImageSampler, TextureFormatPixelInfo},
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};
use image::{ImageFormat as RustImageFormat, codecs::jpeg::JpegEncoder};
use pybevy_array::{BorrowProbe, PyArray, borrowed_read_only_u8, extract_u8_array_data, owned_u8};
use pybevy_color::color::PyColor;
use pybevy_core::{
    AssetStorage, PyAsset,
    borrowed_array_anchor::{AssetBorrowAnchor, AssetBorrowAnchorMut},
    computed_owned,
    content_hash::CanonicalContentHasher,
    numpy_view_guard::{PendingNumpyViewGuard, PyNumpyViewGuard},
};
use pybevy_macros::pyasset;
use pybevy_math::{uvec2::PyUVec2, uvec3::PyUVec3, vec2::PyVec2};
use pybevy_render::{
    extent3d::PyExtent3d, texture_dimension::PyTextureDimension, texture_format::PyTextureFormat,
    texture_view_dimension::PyTextureViewDimension,
};
use pyo3::{
    buffer::PyBuffer,
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyBytes, PyList, PyTuple},
};

use crate::{image_format::PyImageFormat, loader_settings::PyImageSampler};

struct ExtractedU8Data {
    bytes: Vec<u8>,
    shape: Option<Vec<usize>>,
}

/// Extract one of the documented byte-data inputs into owned storage.
fn extract_u8_data_from_any(data: &Bound<'_, PyAny>) -> PyResult<ExtractedU8Data> {
    if let Some((bytes, shape)) = extract_u8_array_data(data)? {
        return Ok(ExtractedU8Data {
            bytes,
            shape: Some(shape),
        });
    }

    if let Ok(buffer) = PyBuffer::<u8>::get(data) {
        return Ok(ExtractedU8Data {
            bytes: buffer.to_vec(data.py())?,
            shape: None,
        });
    }

    if data.is_instance_of::<PyList>() || data.is_instance_of::<PyTuple>() {
        return data
            .extract::<Vec<u8>>()
            .map(|bytes| ExtractedU8Data { bytes, shape: None })
            .map_err(|error| {
                let _ = error
                    .value(data.py())
                    .call_method1("add_note", ("while processing image byte data",));
                error
            });
    }

    Err(PyTypeError::new_err(
        "image byte data must be a bytes-like object, a list or tuple of integers, \
         a uint8 NumPy ndarray, or a uint8 pybevy.array.Array",
    ))
}

fn pixel_size_of(format: TextureFormat) -> Option<usize> {
    format.block_copy_size(None)?;
    format.pixel_size().ok()
}

fn reject_aspect_dependent_format(format: TextureFormat) -> PyResult<()> {
    if format.block_dimensions() == (1, 1) && format.block_copy_size(None).is_none() {
        return Err(PyValueError::new_err(format!(
            "cannot build an Image with format {format:?}: its texel size depends on the \
             texture aspect, which Bevy's Image constructors do not support"
        )));
    }
    Ok(())
}

fn checked_image_byte_len(extent: Extent3d, format: TextureFormat) -> PyResult<Option<usize>> {
    let Some(pixel_size) = pixel_size_of(format) else {
        return Ok(None);
    };
    let pixel_count = (extent.width as usize)
        .checked_mul(extent.height as usize)
        .and_then(|count| count.checked_mul(extent.depth_or_array_layers as usize))
        .ok_or_else(|| PyValueError::new_err("image extent element count overflows usize"))?;
    pixel_count
        .checked_mul(pixel_size)
        .map(Some)
        .ok_or_else(|| PyValueError::new_err("image byte length overflows usize"))
}

fn validate_render_target_dimensions(width: u32, height: u32) -> PyResult<()> {
    if width == 0 || height == 0 {
        return Err(PyValueError::new_err(format!(
            "render target width and height must be greater than zero (got {width}x{height})"
        )));
    }
    Ok(())
}

fn natural_image_shapes(
    extent: Extent3d,
    dimension: TextureDimension,
    pixel_size: usize,
) -> Vec<Vec<usize>> {
    let base = match dimension {
        TextureDimension::D1 => vec![extent.width as usize],
        TextureDimension::D2 if extent.depth_or_array_layers > 1 => {
            vec![
                extent.depth_or_array_layers as usize,
                extent.height as usize,
                extent.width as usize,
            ]
        }
        TextureDimension::D2 => {
            vec![extent.height as usize, extent.width as usize]
        }
        TextureDimension::D3 => vec![
            extent.depth_or_array_layers as usize,
            extent.height as usize,
            extent.width as usize,
        ],
    };
    let mut with_bytes = base.clone();
    with_bytes.push(pixel_size);
    if pixel_size == 1 {
        vec![base, with_bytes]
    } else {
        vec![with_bytes]
    }
}

fn shape_repr(shape: &[usize]) -> String {
    let values = shape
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    if shape.len() == 1 {
        format!("({values},)")
    } else {
        format!("({values})")
    }
}

fn validate_image_data(
    data: &ExtractedU8Data,
    extent: Extent3d,
    dimension: TextureDimension,
    format: TextureFormat,
    expected_len: usize,
) -> PyResult<()> {
    if data.bytes.len() != expected_len {
        return Err(PyValueError::new_err(format!(
            "image byte data length {} does not match {}x{}x{} with format {:?} (expected {})",
            data.bytes.len(),
            extent.width,
            extent.height,
            extent.depth_or_array_layers,
            format,
            expected_len
        )));
    }

    let Some(shape) = &data.shape else {
        return Ok(());
    };
    if shape.as_slice() == [expected_len] {
        return Ok(());
    }

    let natural = match pixel_size_of(format) {
        Some(pixel_size)
            if pixel_size > 0 && checked_image_byte_len(extent, format)? == Some(expected_len) =>
        {
            natural_image_shapes(extent, dimension, pixel_size)
        }
        _ => Vec::new(),
    };
    if natural.iter().any(|candidate| candidate == shape) {
        return Ok(());
    }

    let mut expected_shapes = vec![shape_repr(&[expected_len])];
    expected_shapes.extend(natural.iter().map(|candidate| shape_repr(candidate)));
    Err(PyValueError::new_err(format!(
        "image byte data shape {} does not match {}x{}x{} with format {:?}; expected {}",
        shape_repr(shape),
        extent.width,
        extent.height,
        extent.depth_or_array_layers,
        format,
        expected_shapes.join(" or ")
    )))
}

fn validate_pixel_data(data: &[u8], format: TextureFormat, extent: Extent3d) -> PyResult<()> {
    let pixel_size = pixel_size_of(format).ok_or_else(|| {
        PyValueError::new_err(format!(
            "cannot determine pixel byte size for format {format:?}"
        ))
    })?;
    if data.len() != pixel_size {
        return Err(PyValueError::new_err(format!(
            "pixel byte data length {} does not match format {:?} (expected {})",
            data.len(),
            format,
            pixel_size
        )));
    }
    // Bevy also asserts the pixel fits the destination buffer, which a
    // zero-volume extent makes impossible: a correctly sized pixel would
    // otherwise pass this check and abort inside Bevy.
    if let Some(byte_len) = checked_image_byte_len(extent, format)?
        && data.len() > byte_len
    {
        return Err(PyValueError::new_err(format!(
            "pixel byte data length {} does not fit an image of {}x{}x{} (capacity {} bytes)",
            data.len(),
            extent.width,
            extent.height,
            extent.depth_or_array_layers,
            byte_len
        )));
    }
    Ok(())
}

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
fn py_format_to_rust(format: PyImageFormat) -> PyResult<RustImageFormat> {
    Ok(match format {
        PyImageFormat::Bmp => RustImageFormat::Bmp,
        PyImageFormat::Dds => RustImageFormat::Dds,
        PyImageFormat::Farbfeld => RustImageFormat::Farbfeld,
        PyImageFormat::Gif => RustImageFormat::Gif,
        PyImageFormat::OpenExr => RustImageFormat::OpenExr,
        PyImageFormat::Hdr => RustImageFormat::Hdr,
        PyImageFormat::Ico => RustImageFormat::Ico,
        PyImageFormat::Jpeg => RustImageFormat::Jpeg,
        // The `image` crate cannot encode KTX2.
        PyImageFormat::Ktx2 => {
            return Err(PyValueError::new_err(
                "KTX2 encoding is not supported; use Png, Jpeg, Bmp, Dds, OpenExr, Hdr, Qoi, Pnm, Tga, Tiff or WebP",
            ));
        }
        PyImageFormat::Png => RustImageFormat::Png,
        PyImageFormat::Pnm => RustImageFormat::Pnm,
        PyImageFormat::Qoi => RustImageFormat::Qoi,
        PyImageFormat::Tga => RustImageFormat::Tga,
        PyImageFormat::Tiff => RustImageFormat::Tiff,
        PyImageFormat::WebP => RustImageFormat::WebP,
    })
}

#[pyclass(name = "RenderAssetUsages", from_py_object, frozen)]
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
    anchor: Arc<AssetBorrowAnchor>,
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
    ($s:expr, $f:expr) => {{
        let image = $s.as_ref()?;
        $f(&image)
    }};
}

macro_rules! image_with_mut {
    ($s:expr, $f:expr) => {{
        let mut image = $s.as_mut()?;
        $f(&mut image)
    }};
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
        let dimension: TextureDimension = dimension.unwrap_or(PyTextureDimension::D2).into();
        let format: TextureFormat = format.unwrap_or(PyTextureFormat::Rgba8UnormSrgb).into();
        reject_aspect_dependent_format(format)?;
        let data = match data {
            Some(data) => {
                let extracted = extract_u8_data_from_any(data)?;
                if let Some(expected_len) = checked_image_byte_len(extent, format)? {
                    validate_image_data(&extracted, extent, dimension, format, expected_len)?;
                } else if extracted.shape.is_some() {
                    // Compressed/opaque formats have no pixel-byte layout to
                    // validate, so shaped arrays must remain explicitly flat.
                    validate_image_data(
                        &extracted,
                        extent,
                        dimension,
                        format,
                        extracted.bytes.len(),
                    )?;
                }
                extracted.bytes
            }
            // Default to max-value bytes: white for 8-bit unorm formats
            None => {
                let expected_len = checked_image_byte_len(extent, format)?.ok_or_else(|| {
                    PyValueError::new_err(format!(
                        "cannot default-fill data for format {format:?}; pass data explicitly"
                    ))
                })?;
                vec![255u8; expected_len]
            }
        };

        Ok(Self::from_owned(Image::new(
            extent,
            dimension,
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
        pixel: &Bound<'_, PyAny>,
        format: Option<PyTextureFormat>,
        dimension: Option<PyTextureDimension>,
    ) -> PyResult<Py<PyImage>> {
        let pixel = extract_u8_data_from_any(pixel)?.bytes;
        let format: TextureFormat = format.unwrap_or(PyTextureFormat::Rgba8UnormSrgb).into();
        let extent: Extent3d = size.into();
        validate_pixel_data(&pixel, format, extent)?;
        let image = Image::new_fill(
            extent,
            dimension.unwrap_or(PyTextureDimension::D2).into(),
            &pixel,
            format,
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
        validate_render_target_dimensions(width, height)?;
        let format: TextureFormat = format.into();
        // Bevy sizes the target buffer with an expect() on pixel_size, which a
        // block-compressed format has no answer for.
        pixel_size_of(format).ok_or_else(|| {
            PyValueError::new_err(format!(
                "cannot determine pixel byte size for format {format:?}; \
                 render targets need an uncompressed format"
            ))
        })?;
        let image = Image::new_target_texture(width, height, format, None);

        Py::new(py, Self::from_owned(image))
    }

    /// Create a render target texture with RGBA8 sRGB format and `COPY_SRC` usage.
    ///
    /// This is a convenience method for headless rendering / screenshots.
    /// Use [`new_target_texture`] if you need a custom format.
    #[staticmethod]
    pub fn new_render_target(py: Python<'_>, width: u32, height: u32) -> PyResult<Py<PyImage>> {
        validate_render_target_dimensions(width, height)?;
        let mut image =
            Image::new_target_texture(width, height, TextureFormat::Rgba8UnormSrgb, None);
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;

        Py::new(py, Self::from_owned(image))
    }

    #[staticmethod]
    #[pyo3(signature = (buffer, is_srgb=true))]
    pub fn from_buffer(
        py: Python<'_>,
        buffer: &Bound<'_, PyAny>,
        is_srgb: bool,
    ) -> PyResult<Py<PyImage>> {
        let buffer = extract_u8_data_from_any(buffer)?.bytes;
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
        Ok(computed_owned(self.as_ref()?.size().into()))
    }

    pub fn size_f32(&self) -> PyResult<PyVec2> {
        let size = self.as_ref()?.size();
        Ok(computed_owned(PyVec2::new(size.x as f32, size.y as f32)))
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
        let image = self.as_ref()?;
        Ok(image_payload_hash(&image))
    }

    pub fn data(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<ImageDataContext>> {
        let this = slf.borrow();
        let claim = this.storage.prepare_read_view()?;
        let guard = PyNumpyViewGuard::from_acquired(claim, slf.clone().unbind().into_any());
        let validity = this.storage.validity_flag();
        let image = this.storage.as_ref()?;
        let image_data = image
            .data
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("Image has no data"))?;
        let ptr = image_data.as_ptr();
        let len = image_data.len();
        let anchor = Arc::new(AssetBorrowAnchor::new(validity, guard));
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
        let len = {
            let image = this.storage.as_ref()?;
            image
                .data
                .as_ref()
                .ok_or_else(|| PyRuntimeError::new_err("Image has no data"))?
                .len()
        };
        let validity = this.storage.validity_flag();
        let claim = this.storage.prepare_write_view()?;
        let guard = PendingNumpyViewGuard::from_acquired(claim, slf.clone().unbind().into_any());
        let anchor = Arc::new(AssetBorrowAnchorMut::new(validity, guard));
        let probe: Arc<dyn BorrowProbe> = anchor.clone();
        let array = Py::new(py, PyArray::pending_borrowed_mut_u8(len, &[len], probe)?)?;
        let context = Py::new(
            py,
            ImageDataContextMut {
                array: array.clone_ref(py),
                anchor: anchor.clone(),
            },
        )?;

        let mut transaction = this.storage.begin_write_view(anchor.pending_claim())?;
        let current_len = transaction
            .preflight()
            .data
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("Image has no data"))?
            .len();
        if current_len != len {
            return Err(PyRuntimeError::new_err(
                "Image data length changed during view acquisition",
            ));
        }
        let image = transaction.commit();
        let image_data = image
            .data
            .as_mut()
            .expect("preflight confirmed image data exists");
        let ptr = image_data.as_mut_ptr();
        {
            let mut pending = array.borrow_mut(py);
            // SAFETY: the committed transaction returned the same validated
            // byte buffer under the exclusive claim retained by `anchor`.
            unsafe { pending.bind_borrowed_mut_u8(ptr) };
        }
        anchor.commit();
        drop(transaction);
        Ok(context)
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

    pub fn set_data(&mut self, data: &Bound<'_, PyAny>) -> PyResult<()> {
        let extracted = extract_u8_data_from_any(data)?;
        let (extent, dimension, format, expected_len) = {
            let image = self.as_ref()?;
            let image_data = image
                .data
                .as_ref()
                .ok_or_else(|| PyRuntimeError::new_err("Image has no data"))?;
            (
                image.texture_descriptor.size,
                image.texture_descriptor.dimension,
                image.texture_descriptor.format,
                image_data.len(),
            )
        };
        validate_image_data(&extracted, extent, dimension, format, expected_len)?;
        image_with_mut!(self, |image: &mut Image| {
            let image_data = image
                .data
                .as_mut()
                .ok_or_else(|| PyRuntimeError::new_err("Image has no data"))?;
            image_data.copy_from_slice(&extracted.bytes);
            Ok(())
        })
    }

    pub fn pixel_data_offset(&self, coords: PyUVec3) -> PyResult<Option<usize>> {
        image_with!(self, |image: &Image| {
            Ok(image.pixel_data_offset(coords.try_into()?).ok())
        })
    }

    pub fn pixel_bytes(&self, py: Python<'_>, coords: PyUVec3) -> PyResult<Option<Py<PyBytes>>> {
        image_with!(self, |image: &Image| {
            Ok(image
                .pixel_bytes(coords.try_into()?)
                .ok()
                .map(|bytes| PyBytes::new(py, bytes).unbind()))
        })
    }

    pub fn pixel_bytes_mut(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        coords: PyUVec3,
    ) -> PyResult<Py<ImagePixelContextMut>> {
        let mut this = slf.borrow_mut();
        let bevy_coords = coords.try_into()?;
        let len = {
            let image = this.storage.as_ref()?;
            image
                .pixel_bytes(bevy_coords)
                .map_err(|_| PyRuntimeError::new_err("Invalid pixel coordinates or no image data"))?
                .len()
        };
        let validity = this.storage.validity_flag();
        let claim = this.storage.prepare_write_view()?;
        let guard = PendingNumpyViewGuard::from_acquired(claim, slf.clone().unbind().into_any());
        let anchor = Arc::new(AssetBorrowAnchorMut::new(validity, guard));
        let probe: Arc<dyn BorrowProbe> = anchor.clone();
        let array = Py::new(py, PyArray::pending_borrowed_mut_u8(len, &[len], probe)?)?;
        let context = Py::new(
            py,
            ImagePixelContextMut {
                array: array.clone_ref(py),
                anchor: anchor.clone(),
            },
        )?;

        let mut transaction = this.storage.begin_write_view(anchor.pending_claim())?;
        let current_len = transaction
            .preflight()
            .pixel_bytes(bevy_coords)
            .map_err(|_| PyRuntimeError::new_err("Invalid pixel coordinates or no image data"))?;
        if current_len.len() != len {
            return Err(PyRuntimeError::new_err(
                "Image pixel layout changed during view acquisition",
            ));
        }
        let image = transaction.commit();
        let pixel_bytes = image
            .pixel_bytes_mut(bevy_coords)
            .expect("preflight confirmed the pixel coordinates and image data");
        let ptr = pixel_bytes.as_mut_ptr();
        {
            let mut pending = array.borrow_mut(py);
            // SAFETY: the committed transaction returned the same validated
            // pixel subslice under the exclusive claim retained by `anchor`.
            unsafe { pending.bind_borrowed_mut_u8(ptr) };
        }
        anchor.commit();
        drop(transaction);
        Ok(context)
    }

    #[getter]
    pub fn format(&self) -> PyResult<PyTextureFormat> {
        image_with!(self, |image: &Image| {
            PyTextureFormat::try_from(image.texture_descriptor.format)
        })
    }

    #[getter]
    pub fn dimension(&self) -> PyResult<PyTextureDimension> {
        image_with!(self, |image: &Image| {
            Ok(image.texture_descriptor.dimension.into())
        })
    }

    #[getter]
    pub fn texture_view_dimension(&self) -> PyResult<Option<PyTextureViewDimension>> {
        image_with!(self, |image: &Image| {
            Ok(image
                .texture_view_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.dimension)
                .map(Into::into))
        })
    }

    #[setter]
    pub fn set_texture_view_dimension(
        &mut self,
        dimension: Option<PyTextureViewDimension>,
    ) -> PyResult<()> {
        image_with_mut!(self, |image: &mut Image| {
            match dimension {
                Some(dimension) => {
                    image
                        .texture_view_descriptor
                        .get_or_insert_default()
                        .dimension = Some(dimension.into());
                }
                None => {
                    let remove_descriptor =
                        if let Some(descriptor) = image.texture_view_descriptor.as_mut() {
                            descriptor.dimension = None;
                            *descriptor == Default::default()
                        } else {
                            false
                        };
                    if remove_descriptor {
                        image.texture_view_descriptor = None;
                    }
                }
            }
            Ok(())
        })
    }

    #[getter]
    pub fn mip_level_count(&self) -> PyResult<u32> {
        image_with!(self, |image: &Image| {
            Ok(image.texture_descriptor.mip_level_count)
        })
    }

    #[getter]
    pub fn sampler(&self, py: Python<'_>) -> PyResult<Py<PyImageSampler>> {
        let storage = self.storage.borrow_field(
            |image: &Image| &image.sampler,
            |image: &mut Image| &mut image.sampler,
        )?;
        PyImageSampler::from_storage(storage, py)
    }

    #[setter]
    pub fn set_sampler(&mut self, sampler: PyImageSampler) -> PyResult<()> {
        let sampler = ImageSampler::try_from(sampler)?;
        image_with_mut!(self, |image: &mut Image| {
            image.sampler = sampler;
            Ok(())
        })
    }

    #[getter]
    pub fn asset_usage(&self) -> PyResult<PyRenderAssetUsages> {
        image_with!(self, |image: &Image| Ok(image.asset_usage.into()))
    }

    #[setter]
    pub fn set_asset_usage(&mut self, usage: PyRenderAssetUsages) -> PyResult<()> {
        let usage = usage.into();
        image_with_mut!(self, |image: &mut Image| {
            image.asset_usage = usage;
            Ok(())
        })
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
        let color = color.try_into()?;
        image_with!(self, |image: &Image| image.get_color_at_1d(x))
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to set color at ({x}): {e}")))?;
        image_with_mut!(self, |image: &mut Image| image.set_color_at_1d(x, color))
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to set color at ({x}): {e}")))?;
        Ok(())
    }

    pub fn set_color_at(&mut self, x: u32, y: u32, color: PyColor) -> PyResult<()> {
        let color = color.try_into()?;
        image_with!(self, |image: &Image| image.get_color_at(x, y)).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to set color at ({x}, {y}): {e}"))
        })?;
        image_with_mut!(self, |image: &mut Image| image.set_color_at(x, y, color)).map_err(
            |e| PyRuntimeError::new_err(format!("Failed to set color at ({x}, {y}): {e}")),
        )?;
        Ok(())
    }

    pub fn set_color_at_3d(&mut self, x: u32, y: u32, z: u32, color: PyColor) -> PyResult<()> {
        let color = color.try_into()?;
        image_with!(self, |image: &Image| image.get_color_at_3d(x, y, z)).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to set color at ({x}, {y}, {z}): {e}"))
        })?;
        image_with_mut!(self, |image: &mut Image| image
            .set_color_at_3d(x, y, z, color))
        .map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to set color at ({x}, {y}, {z}): {e}"))
        })?;
        Ok(())
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
        let mut candidate = image_with!(self, |image: &Image| image.clone());
        candidate
            .reinterpret_size(size.into())
            .map_err(|e| PyValueError::new_err(format!("{e:?}")))?;
        image_with_mut!(self, |image: &mut Image| *image = candidate);
        Ok(())
    }

    pub fn reinterpret_stacked_2d_as_array(&mut self, layers: u32) -> PyResult<()> {
        let mut candidate = image_with!(self, |image: &Image| image.clone());
        candidate
            .reinterpret_stacked_2d_as_array(layers)
            .map_err(|e| PyValueError::new_err(format!("{e:?}")))?;
        image_with_mut!(self, |image: &mut Image| *image = candidate);
        Ok(())
    }

    pub fn clear(&mut self, pixel: &Bound<'_, PyAny>) -> PyResult<()> {
        let pixel = extract_u8_data_from_any(pixel)?.bytes;
        let descriptor = {
            let image = self.as_ref()?;
            (
                image.texture_descriptor.format,
                image.texture_descriptor.size,
            )
        };
        validate_pixel_data(&pixel, descriptor.0, descriptor.1)?;
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
                    .write_to(&mut buffer, py_format_to_rust(format)?)
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

    /// Never formats `data`: a 1080p image would render 8M integers.
    pub fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(image) => {
                let size = image.texture_descriptor.size;
                format!(
                    "Image({}x{}x{}, {:?}, {} bytes)",
                    size.width,
                    size.height,
                    size.depth_or_array_layers,
                    image.texture_descriptor.format,
                    image.data.as_ref().map(Vec::len).unwrap_or(0),
                )
            }
            Err(_) => "Image(<invalid>)".to_string(),
        }
    }
}
