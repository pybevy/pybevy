use bevy::{
    asset::RenderAssetUsages,
    image::{ImageFormatSetting, ImageLoaderSettings, ImageSampler},
};
use pyo3::prelude::*;

use crate::{
    image::PyRenderAssetUsages, image_address_mode::PyImageAddressMode,
    image_filter_mode::PyImageFilterMode, image_format::PyImageFormat,
    image_format_setting::PyImageFormatSetting, sampler_descriptor::PyImageSamplerDescriptor,
};

#[pyclass(name = "ImageSampler", eq, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyImageSampler {
    Default(),
    Descriptor { desc: PyImageSamplerDescriptor },
}

#[pymethods]
impl PyImageSampler {
    #[staticmethod]
    #[pyo3(name = "default")]
    pub fn default_sampler() -> Self {
        PyImageSampler::Default()
    }

    #[staticmethod]
    pub fn linear() -> Self {
        PyImageSampler::Descriptor {
            desc: PyImageSamplerDescriptor::linear(),
        }
    }

    #[staticmethod]
    pub fn nearest() -> Self {
        PyImageSampler::Descriptor {
            desc: PyImageSamplerDescriptor::nearest(),
        }
    }

    #[staticmethod]
    pub fn descriptor(desc: PyImageSamplerDescriptor) -> Self {
        PyImageSampler::Descriptor { desc }
    }

    #[getter]
    pub fn is_default(&self) -> bool {
        matches!(self, PyImageSampler::Default())
    }

    #[getter]
    pub fn mag_filter(&self) -> Option<PyImageFilterMode> {
        match self {
            PyImageSampler::Descriptor { desc } => Some(desc.mag_filter()),
            _ => None,
        }
    }

    #[getter]
    pub fn min_filter(&self) -> Option<PyImageFilterMode> {
        match self {
            PyImageSampler::Descriptor { desc } => Some(desc.min_filter()),
            _ => None,
        }
    }

    #[getter]
    pub fn mipmap_filter(&self) -> Option<PyImageFilterMode> {
        match self {
            PyImageSampler::Descriptor { desc } => Some(desc.mipmap_filter()),
            _ => None,
        }
    }

    #[getter]
    pub fn address_mode_u(&self) -> Option<PyImageAddressMode> {
        match self {
            PyImageSampler::Descriptor { desc } => Some(desc.address_mode_u()),
            _ => None,
        }
    }

    #[getter]
    pub fn address_mode_v(&self) -> Option<PyImageAddressMode> {
        match self {
            PyImageSampler::Descriptor { desc } => Some(desc.address_mode_v()),
            _ => None,
        }
    }

    #[getter]
    pub fn address_mode_w(&self) -> Option<PyImageAddressMode> {
        match self {
            PyImageSampler::Descriptor { desc } => Some(desc.address_mode_w()),
            _ => None,
        }
    }

    pub fn __repr__(&self) -> String {
        match self {
            PyImageSampler::Default() => "ImageSampler.Default".to_string(),
            PyImageSampler::Descriptor { desc } => {
                format!("ImageSampler.Descriptor({})", desc.__repr__())
            }
        }
    }
}

impl From<ImageSampler> for PyImageSampler {
    fn from(sampler: ImageSampler) -> Self {
        match sampler {
            ImageSampler::Default => PyImageSampler::Default(),
            ImageSampler::Descriptor(desc) => PyImageSampler::Descriptor { desc: desc.into() },
        }
    }
}

impl From<PyImageSampler> for ImageSampler {
    fn from(sampler: PyImageSampler) -> Self {
        match sampler {
            PyImageSampler::Default() => ImageSampler::Default,
            PyImageSampler::Descriptor { desc } => ImageSampler::Descriptor(desc.into()),
        }
    }
}

impl From<&PyImageSampler> for ImageSampler {
    fn from(sampler: &PyImageSampler) -> Self {
        match sampler {
            PyImageSampler::Default() => ImageSampler::Default,
            PyImageSampler::Descriptor { desc } => ImageSampler::Descriptor(desc.into()),
        }
    }
}

#[pyclass(name = "ImageLoaderSettings", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyImageLoaderSettings {
    inner: ImageLoaderSettings,
}

#[pymethods]
impl PyImageLoaderSettings {
    #[new]
    #[pyo3(signature = (
        is_srgb = true,
        sampler = None,
    ))]
    pub fn new(is_srgb: bool, sampler: Option<PyImageSampler>) -> Self {
        Self {
            inner: ImageLoaderSettings {
                format: ImageFormatSetting::FromExtension,
                texture_format: None,
                is_srgb,
                sampler: sampler.map(Into::into).unwrap_or(ImageSampler::Default),
                asset_usage: RenderAssetUsages::default(),
                array_layout: None,
            },
        }
    }

    #[staticmethod]
    #[pyo3(signature = (format, is_srgb = true, sampler = None))]
    pub fn with_format(
        format: PyImageFormat,
        is_srgb: bool,
        sampler: Option<PyImageSampler>,
    ) -> Self {
        Self {
            inner: ImageLoaderSettings {
                format: ImageFormatSetting::Format(format.into()),
                texture_format: None,
                is_srgb,
                sampler: sampler.map(Into::into).unwrap_or(ImageSampler::Default),
                asset_usage: RenderAssetUsages::default(),
                array_layout: None,
            },
        }
    }

    #[getter]
    pub fn is_srgb(&self) -> bool {
        self.inner.is_srgb
    }

    #[setter]
    pub fn set_is_srgb(&mut self, value: bool) {
        self.inner.is_srgb = value;
    }

    #[getter]
    pub fn sampler(&self) -> PyImageSampler {
        self.inner.sampler.clone().into()
    }

    #[setter]
    pub fn set_sampler(&mut self, value: PyImageSampler) {
        self.inner.sampler = value.into();
    }

    #[getter]
    pub fn format(&self) -> PyImageFormatSetting {
        self.inner.format.clone().into()
    }

    #[getter]
    pub fn asset_usage(&self) -> PyRenderAssetUsages {
        self.inner.asset_usage.into()
    }

    #[setter]
    pub fn set_asset_usage(&mut self, value: PyRenderAssetUsages) {
        self.inner.asset_usage = value.into();
    }

    pub fn __repr__(&self) -> String {
        format!(
            "ImageLoaderSettings(format={:?}, is_srgb={}, sampler={:?})",
            self.inner.format, self.inner.is_srgb, self.inner.sampler
        )
    }
}

impl From<ImageLoaderSettings> for PyImageLoaderSettings {
    fn from(inner: ImageLoaderSettings) -> Self {
        Self { inner }
    }
}

impl From<PyImageLoaderSettings> for ImageLoaderSettings {
    fn from(py: PyImageLoaderSettings) -> Self {
        py.inner
    }
}

impl From<&PyImageLoaderSettings> for ImageLoaderSettings {
    fn from(py: &PyImageLoaderSettings) -> Self {
        py.inner.clone()
    }
}
