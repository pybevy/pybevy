use bevy::image::ImageSamplerDescriptor;
use pyo3::prelude::*;

use crate::{
    image_address_mode::PyImageAddressMode, image_compare_function::PyImageCompareFunction,
    image_filter_mode::PyImageFilterMode, image_sampler_border_color::PyImageSamplerBorderColor,
};

#[pyclass(name = "ImageSamplerDescriptor", eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyImageSamplerDescriptor {
    inner: ImageSamplerDescriptor,
}

#[pymethods]
impl PyImageSamplerDescriptor {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        address_mode_u = PyImageAddressMode::ClampToEdge,
        address_mode_v = PyImageAddressMode::ClampToEdge,
        address_mode_w = PyImageAddressMode::ClampToEdge,
        mag_filter = PyImageFilterMode::Nearest,
        min_filter = PyImageFilterMode::Nearest,
        mipmap_filter = PyImageFilterMode::Nearest,
        lod_min_clamp = 0.0,
        lod_max_clamp = 32.0,
        compare = None,
        anisotropy_clamp = 1,
        border_color = None,
        label = None,
    ))]
    pub fn new(
        address_mode_u: PyImageAddressMode,
        address_mode_v: PyImageAddressMode,
        address_mode_w: PyImageAddressMode,
        mag_filter: PyImageFilterMode,
        min_filter: PyImageFilterMode,
        mipmap_filter: PyImageFilterMode,
        lod_min_clamp: f32,
        lod_max_clamp: f32,
        compare: Option<PyImageCompareFunction>,
        anisotropy_clamp: u16,
        border_color: Option<PyImageSamplerBorderColor>,
        label: Option<String>,
    ) -> Self {
        Self {
            inner: ImageSamplerDescriptor {
                address_mode_u: address_mode_u.into(),
                address_mode_v: address_mode_v.into(),
                address_mode_w: address_mode_w.into(),
                mag_filter: mag_filter.into(),
                min_filter: min_filter.into(),
                mipmap_filter: mipmap_filter.into(),
                lod_min_clamp,
                lod_max_clamp,
                compare: compare.map(Into::into),
                anisotropy_clamp,
                border_color: border_color.map(Into::into),
                label,
            },
        }
    }

    #[staticmethod]
    pub fn linear() -> Self {
        Self {
            inner: ImageSamplerDescriptor::linear(),
        }
    }

    #[staticmethod]
    pub fn nearest() -> Self {
        Self {
            inner: ImageSamplerDescriptor::nearest(),
        }
    }

    #[getter]
    pub fn address_mode_u(&self) -> PyImageAddressMode {
        self.inner.address_mode_u.into()
    }

    #[setter]
    pub fn set_address_mode_u(&mut self, value: PyImageAddressMode) {
        self.inner.address_mode_u = value.into();
    }

    #[getter]
    pub fn address_mode_v(&self) -> PyImageAddressMode {
        self.inner.address_mode_v.into()
    }

    #[setter]
    pub fn set_address_mode_v(&mut self, value: PyImageAddressMode) {
        self.inner.address_mode_v = value.into();
    }

    #[getter]
    pub fn address_mode_w(&self) -> PyImageAddressMode {
        self.inner.address_mode_w.into()
    }

    #[setter]
    pub fn set_address_mode_w(&mut self, value: PyImageAddressMode) {
        self.inner.address_mode_w = value.into();
    }

    #[getter]
    pub fn mag_filter(&self) -> PyImageFilterMode {
        self.inner.mag_filter.into()
    }

    #[setter]
    pub fn set_mag_filter(&mut self, value: PyImageFilterMode) {
        self.inner.mag_filter = value.into();
    }

    #[getter]
    pub fn min_filter(&self) -> PyImageFilterMode {
        self.inner.min_filter.into()
    }

    #[setter]
    pub fn set_min_filter(&mut self, value: PyImageFilterMode) {
        self.inner.min_filter = value.into();
    }

    #[getter]
    pub fn mipmap_filter(&self) -> PyImageFilterMode {
        self.inner.mipmap_filter.into()
    }

    #[setter]
    pub fn set_mipmap_filter(&mut self, value: PyImageFilterMode) {
        self.inner.mipmap_filter = value.into();
    }

    #[getter]
    pub fn lod_min_clamp(&self) -> f32 {
        self.inner.lod_min_clamp
    }

    #[setter]
    pub fn set_lod_min_clamp(&mut self, value: f32) {
        self.inner.lod_min_clamp = value;
    }

    #[getter]
    pub fn lod_max_clamp(&self) -> f32 {
        self.inner.lod_max_clamp
    }

    #[setter]
    pub fn set_lod_max_clamp(&mut self, value: f32) {
        self.inner.lod_max_clamp = value;
    }

    #[getter]
    pub fn compare(&self) -> Option<PyImageCompareFunction> {
        self.inner.compare.map(Into::into)
    }

    #[setter]
    pub fn set_compare(&mut self, value: Option<PyImageCompareFunction>) {
        self.inner.compare = value.map(Into::into);
    }

    #[getter]
    pub fn anisotropy_clamp(&self) -> u16 {
        self.inner.anisotropy_clamp
    }

    #[setter]
    pub fn set_anisotropy_clamp(&mut self, value: u16) {
        self.inner.anisotropy_clamp = value;
    }

    #[getter]
    pub fn border_color(&self) -> Option<PyImageSamplerBorderColor> {
        self.inner.border_color.map(Into::into)
    }

    #[setter]
    pub fn set_border_color(&mut self, value: Option<PyImageSamplerBorderColor>) {
        self.inner.border_color = value.map(Into::into);
    }

    #[getter]
    pub fn label(&self) -> Option<String> {
        self.inner.label.clone()
    }

    #[setter]
    pub fn set_label(&mut self, value: Option<String>) {
        self.inner.label = value;
    }

    pub fn __repr__(&self) -> String {
        format!(
            "ImageSamplerDescriptor(mag_filter={:?}, min_filter={:?}, mipmap_filter={:?})",
            self.inner.mag_filter, self.inner.min_filter, self.inner.mipmap_filter
        )
    }
}

impl From<ImageSamplerDescriptor> for PyImageSamplerDescriptor {
    fn from(inner: ImageSamplerDescriptor) -> Self {
        Self { inner }
    }
}

impl From<PyImageSamplerDescriptor> for ImageSamplerDescriptor {
    fn from(py: PyImageSamplerDescriptor) -> Self {
        py.inner
    }
}

impl From<&PyImageSamplerDescriptor> for ImageSamplerDescriptor {
    fn from(py: &PyImageSamplerDescriptor) -> Self {
        py.inner.clone()
    }
}

impl PyImageSamplerDescriptor {
    pub fn to_image_sampler(&self) -> bevy::image::ImageSampler {
        bevy::image::ImageSampler::Descriptor(self.inner.clone())
    }
}
