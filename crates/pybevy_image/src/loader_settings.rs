use bevy::{
    asset::RenderAssetUsages,
    image::{
        ImageArrayLayout, ImageFormatSetting, ImageLoaderSettings, ImageSampler,
        ImageSamplerDescriptor,
    },
};
use pybevy_core::{
    FieldStorage, FromBorrowedStorage, Omittable, StorageRef, computed_owned,
    enum_comparison::compare_values,
    public_error::{enum_variant_changed, image_array_layout_zero},
};
use pybevy_macros::{pyenum, pyfield};
use pybevy_render::texture_format::PyTextureFormat;
use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
    pyclass::CompareOp,
    types::PyTuple,
};

use crate::{
    image::PyRenderAssetUsages, image_array_layout::PyImageArrayLayout,
    image_format_setting::PyImageFormatSetting, sampler_descriptor::PyImageSamplerDescriptor,
};

/// Bevy's loader divides the image size by these values, so zero panics it.
fn checked_array_layout(layout: Option<PyImageArrayLayout>) -> PyResult<Option<ImageArrayLayout>> {
    let Some(layout) = layout else {
        return Ok(None);
    };
    let layout = ImageArrayLayout::from(layout);
    let zero = match layout {
        ImageArrayLayout::RowCount { rows } => (rows == 0).then_some("rows"),
        ImageArrayLayout::RowHeight { pixels } => (pixels == 0).then_some("pixels"),
        ImageArrayLayout::GridCount { columns, rows } => (columns == 0)
            .then_some("columns")
            .or((rows == 0).then_some("rows"))
            .or(columns
                .checked_mul(rows)
                .is_none()
                .then_some("columns * rows")),
        ImageArrayLayout::GridSize {
            tile_width_pixels,
            tile_height_pixels,
        } => (tile_width_pixels == 0)
            .then_some("tile_width_pixels")
            .or((tile_height_pixels == 0).then_some("tile_height_pixels")),
    };
    match zero {
        Some(field) => Err(PyValueError::new_err(image_array_layout_zero(field))),
        None => Ok(Some(layout)),
    }
}

#[pyenum(ImageSampler, manual)]
#[pyclass(
    name = "ImageSampler",
    module = "pybevy.image",
    subclass,
    from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyImageSampler {
    storage: FieldStorage<ImageSampler>,
    expected: ImageSamplerVariant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageSamplerVariant {
    Default,
    Descriptor,
}

impl ImageSamplerVariant {
    fn of(sampler: &ImageSampler) -> Self {
        match sampler {
            ImageSampler::Default => Self::Default,
            ImageSampler::Descriptor(_) => Self::Descriptor,
        }
    }

    fn qualname(self) -> &'static str {
        match self {
            Self::Default => "ImageSampler.Default",
            Self::Descriptor => "ImageSampler.Descriptor",
        }
    }
}

impl From<ImageSampler> for PyImageSampler {
    fn from(sampler: ImageSampler) -> Self {
        Self {
            expected: ImageSamplerVariant::of(&sampler),
            storage: FieldStorage::owned(sampler),
        }
    }
}

impl TryFrom<PyImageSampler> for ImageSampler {
    type Error = PyErr;

    fn try_from(sampler: PyImageSampler) -> PyResult<Self> {
        sampler.resolved_clone()
    }
}

impl TryFrom<&PyImageSampler> for ImageSampler {
    type Error = PyErr;

    fn try_from(sampler: &PyImageSampler) -> PyResult<Self> {
        sampler.resolved_clone()
    }
}

#[pymethods]
impl PyImageSampler {
    #[staticmethod]
    pub fn linear(py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_sampler(
            ImageSampler::Descriptor(ImageSamplerDescriptor::linear()),
            py,
        )
    }

    #[staticmethod]
    pub fn nearest(py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_sampler(
            ImageSampler::Descriptor(ImageSamplerDescriptor::nearest()),
            py,
        )
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let sampler = self.storage.as_ref()?;
        let rendered = match sampler.reborrow() {
            ImageSampler::Default => "ImageSampler.Default".to_string(),
            ImageSampler::Descriptor(desc) => format!(
                "ImageSampler.Descriptor({})",
                PyImageSamplerDescriptor::from(desc.clone()).__repr__()?
            ),
        };
        if ImageSamplerVariant::of(&sampler) != self.expected {
            // Render variant-stale wrappers without bypassing storage validity.
            return Ok(format!(
                "<stale {} wrapper; the value is now {rendered}>",
                self.expected.qualname()
            ));
        }
        Ok(rendered)
    }

    pub fn __copy__(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_sampler(self.resolved_clone()?, py)
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<Self>> {
        self.__copy__(py)
    }

    pub fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<Py<PyAny>> {
        compare_values(self, other, op, |left, right| {
            Ok(left.resolved_clone()? == right.resolved_clone()?)
        })
    }

    #[classattr]
    const __hash__: Option<Py<PyAny>> = None;
}

impl PyImageSampler {
    pub fn from_sampler(sampler: ImageSampler, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_storage(FieldStorage::owned(sampler), py)
    }

    pub fn from_storage(storage: FieldStorage<ImageSampler>, py: Python<'_>) -> PyResult<Py<Self>> {
        let sampler = storage.get()?;
        let expected = ImageSamplerVariant::of(&sampler);
        match sampler {
            ImageSampler::Default => {
                let value = Py::new(
                    py,
                    PyClassInitializer::from(Self { storage, expected })
                        .add_subclass(PyImageSamplerDefault),
                )?;
                Ok(value.into_bound(py).into_super().unbind())
            }
            ImageSampler::Descriptor(_) => {
                let value = Py::new(
                    py,
                    PyClassInitializer::from(Self { storage, expected })
                        .add_subclass(PyImageSamplerDescriptorVariant),
                )?;
                Ok(value.into_bound(py).into_super().unbind())
            }
        }
    }

    fn validate_variant(&self, sampler: &ImageSampler) -> PyResult<()> {
        if ImageSamplerVariant::of(sampler) == self.expected {
            Ok(())
        } else {
            Err(PyRuntimeError::new_err(enum_variant_changed(
                self.expected.qualname(),
            )))
        }
    }

    fn as_ref(&self) -> PyResult<StorageRef<'_, ImageSampler>> {
        let sampler = self.storage.as_ref()?;
        self.validate_variant(&sampler)?;
        Ok(sampler)
    }

    pub fn resolved_clone(&self) -> PyResult<ImageSampler> {
        Ok(self.as_ref()?.clone())
    }
}

#[pyclass(name = "Default", module = "pybevy.image", extends = PyImageSampler)]
pub struct PyImageSamplerDefault;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyImageSamplerDefault {
    #[classattr]
    const __qualname__: &'static str = "ImageSampler.Default";

    #[classattr]
    fn __match_args__(py: Python<'_>) -> Py<PyTuple> {
        PyTuple::empty(py).unbind()
    }

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyImageSampler::from(ImageSampler::Default)).add_subclass(Self)
    }
}

#[pyclass(name = "Descriptor", module = "pybevy.image", extends = PyImageSampler)]
pub struct PyImageSamplerDescriptorVariant;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyImageSamplerDescriptorVariant {
    #[classattr]
    const __qualname__: &'static str = "ImageSampler.Descriptor";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("desc",)
    }

    #[new]
    pub fn new(desc: &PyImageSamplerDescriptor) -> PyResult<PyClassInitializer<Self>> {
        let desc = ImageSamplerDescriptor::try_from(desc)?;
        Ok(
            PyClassInitializer::from(PyImageSampler::from(ImageSampler::Descriptor(desc)))
                .add_subclass(Self),
        )
    }

    #[getter]
    pub fn desc(slf: PyRef<'_, Self>) -> PyResult<PyImageSamplerDescriptor> {
        let base = slf.into_super();
        Ok(base.storage.borrow_resolved_variant_as(
            "ImageSampler.Descriptor",
            |sampler| match sampler {
                ImageSampler::Descriptor(desc) => Some(desc),
                ImageSampler::Default => None,
            },
            |sampler| match sampler {
                ImageSampler::Descriptor(desc) => Some(desc),
                ImageSampler::Default => None,
            },
        )?)
    }

    #[setter]
    pub fn set_desc(slf: PyRefMut<'_, Self>, value: &PyImageSamplerDescriptor) -> PyResult<()> {
        let value = ImageSamplerDescriptor::try_from(value)?;
        let mut base = slf.into_super();
        drop(base.as_ref()?);
        let mut sampler = base.storage.as_mut()?;
        match &mut *sampler {
            ImageSampler::Descriptor(desc) => {
                *desc = value;
                Ok(())
            }
            ImageSampler::Default => Err(PyRuntimeError::new_err(enum_variant_changed(
                "ImageSampler.Descriptor",
            ))),
        }
    }
}

pub fn register_image_sampler_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let base = module.getattr("ImageSampler")?;
    base.setattr("Default", py.get_type::<PyImageSamplerDefault>())?;
    base.setattr(
        "Descriptor",
        py.get_type::<PyImageSamplerDescriptorVariant>(),
    )?;
    Ok(())
}

#[pyfield]
#[pyclass(name = "ImageLoaderSettings", module = "pybevy.image", from_py_object)]
#[derive(Debug)]
pub struct PyImageLoaderSettings {
    storage: FieldStorage<ImageLoaderSettings>,
}

#[pymethods]
impl PyImageLoaderSettings {
    #[new]
    #[pyo3(signature = (
        *,
        format = Omittable::default(),
        texture_format = None,
        is_srgb = true,
        sampler = PyImageSampler::from(ImageSampler::Default),
        asset_usage = PyRenderAssetUsages::from(RenderAssetUsages::default()),
        array_layout = None,
        expand_grayscale = true
    ))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        format: Omittable<PyRef<'_, PyImageFormatSetting>>,
        texture_format: Option<PyTextureFormat>,
        is_srgb: bool,
        sampler: PyImageSampler,
        asset_usage: PyRenderAssetUsages,
        array_layout: Option<PyImageArrayLayout>,
        expand_grayscale: bool,
    ) -> PyResult<Self> {
        Ok(Self::from_owned(ImageLoaderSettings {
            format: format
                .into_option()
                .map_or(ImageFormatSetting::FromExtension, |format| {
                    (*format).clone().into()
                }),
            texture_format: texture_format.map(Into::into),
            is_srgb,
            sampler: ImageSampler::try_from(sampler)?,
            asset_usage: RenderAssetUsages::try_from(asset_usage)?,
            array_layout: checked_array_layout(array_layout)?,
            expand_grayscale,
        }))
    }

    #[getter]
    pub fn texture_format(&self) -> PyResult<Option<PyTextureFormat>> {
        self.as_ref()?
            .texture_format
            .map(PyTextureFormat::try_from)
            .transpose()
    }

    #[setter]
    pub fn set_texture_format(&mut self, value: Option<PyTextureFormat>) -> PyResult<()> {
        self.as_mut()?.texture_format = value.map(Into::into);
        Ok(())
    }

    #[getter]
    pub fn array_layout(&self) -> PyResult<Option<PyImageArrayLayout>> {
        Ok(self.as_ref()?.array_layout.map(Into::into))
    }

    #[setter]
    pub fn set_array_layout(&mut self, value: Option<PyImageArrayLayout>) -> PyResult<()> {
        let layout = checked_array_layout(value)?;
        self.as_mut()?.array_layout = layout;
        Ok(())
    }

    #[getter]
    pub fn is_srgb(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_srgb)
    }

    #[setter]
    pub fn set_is_srgb(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.is_srgb = value;
        Ok(())
    }

    #[getter]
    pub fn sampler(&self, py: Python<'_>) -> PyResult<Py<PyImageSampler>> {
        let storage: FieldStorage<ImageSampler> = self
            .storage
            .borrow_field(|settings: &ImageLoaderSettings| &settings.sampler)?;
        PyImageSampler::from_storage(storage, py)
    }

    #[setter]
    pub fn set_sampler(&mut self, value: PyImageSampler) -> PyResult<()> {
        let sampler = ImageSampler::try_from(value)?;
        self.as_mut()?.sampler = sampler;
        Ok(())
    }

    #[getter]
    pub fn format(&self) -> PyResult<PyImageFormatSetting> {
        Ok(computed_owned(self.as_ref()?.format.clone().into()))
    }

    #[setter]
    pub fn set_format(&mut self, value: PyRef<'_, PyImageFormatSetting>) -> PyResult<()> {
        self.as_mut()?.format = (*value).clone().into();
        Ok(())
    }

    #[getter]
    pub fn asset_usage(&self) -> PyResult<PyRenderAssetUsages> {
        Ok(self.storage.borrow_resolved_field_as(
            |settings| &settings.asset_usage,
            |settings| &mut settings.asset_usage,
        )?)
    }

    #[setter]
    pub fn set_asset_usage(&mut self, value: PyRenderAssetUsages) -> PyResult<()> {
        let usage = RenderAssetUsages::try_from(value)?;
        self.as_mut()?.asset_usage = usage;
        Ok(())
    }

    #[getter]
    pub fn expand_grayscale(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.expand_grayscale)
    }

    #[setter]
    pub fn set_expand_grayscale(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.expand_grayscale = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let settings = self.as_ref()?;
        let texture_format = match settings.texture_format {
            Some(format) => format!("TextureFormat.{format:?}"),
            None => "None".to_string(),
        };
        let array_layout = match settings.array_layout {
            Some(layout) => PyImageArrayLayout::from(layout).__repr__(),
            None => "None".to_string(),
        };
        Ok(format!(
            "ImageLoaderSettings(format={}, texture_format={texture_format}, is_srgb={}, sampler={}, array_layout={array_layout}, expand_grayscale={})",
            PyImageFormatSetting::from(settings.format.clone()).__repr__(),
            if settings.is_srgb { "True" } else { "False" },
            PyImageSampler::from(settings.sampler.clone()).__repr__()?,
            if settings.expand_grayscale {
                "True"
            } else {
                "False"
            }
        ))
    }
}
