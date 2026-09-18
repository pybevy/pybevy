use bevy::{
    asset::RenderAssetUsages,
    image::{ImageFormatSetting, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
};
use pybevy_core::{
    FieldStorage, FromBorrowedStorage, StorageRef, computed_owned,
    public_error::enum_variant_changed,
};
use pybevy_macros::{pyenum, pyfield};
use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyTuple};

use crate::{
    image::PyRenderAssetUsages, image_format::PyImageFormat,
    image_format_setting::PyImageFormatSetting, sampler_descriptor::PyImageSamplerDescriptor,
};

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

    pub fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let sampler = self.storage.as_ref()?;
        let rendered = match sampler.reborrow() {
            ImageSampler::Default => "ImageSampler.Default".to_string(),
            ImageSampler::Descriptor(desc) => format!(
                "ImageSampler.Descriptor({})",
                PyImageSamplerDescriptor::from(desc.clone()).__repr__(py)?
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

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.resolved_clone()? == other.resolved_clone()?)
    }

    pub fn __ne__(&self, other: &Self) -> PyResult<bool> {
        Ok(!self.__eq__(other)?)
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
        is_srgb = true,
        sampler = None
    ))]
    pub fn new(is_srgb: bool, sampler: Option<PyImageSampler>) -> PyResult<Self> {
        Ok(Self::from_owned(ImageLoaderSettings {
            format: ImageFormatSetting::FromExtension,
            texture_format: None,
            is_srgb,
            sampler: sampler
                .map(ImageSampler::try_from)
                .transpose()?
                .unwrap_or(ImageSampler::Default),
            asset_usage: RenderAssetUsages::default(),
            array_layout: None,
        }))
    }

    #[staticmethod]
    #[pyo3(signature = (format, is_srgb = true, sampler = None))]
    pub fn with_format(
        format: PyImageFormat,
        is_srgb: bool,
        sampler: Option<PyImageSampler>,
    ) -> PyResult<Self> {
        Ok(Self::from_owned(ImageLoaderSettings {
            format: ImageFormatSetting::Format(format.into()),
            texture_format: None,
            is_srgb,
            sampler: sampler
                .map(ImageSampler::try_from)
                .transpose()?
                .unwrap_or(ImageSampler::Default),
            asset_usage: RenderAssetUsages::default(),
            array_layout: None,
        }))
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

    pub fn __repr__(&self) -> PyResult<String> {
        let settings = self.as_ref()?;
        Ok(format!(
            "ImageLoaderSettings(format={:?}, is_srgb={}, sampler={:?})",
            settings.format,
            if settings.is_srgb { "True" } else { "False" },
            settings.sampler
        ))
    }
}
