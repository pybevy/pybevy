use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use pybevy_core::{FieldStorage, FromBorrowedStorage};
use pybevy_macros::pyfield;
use pyo3::prelude::*;

use crate::{
    image_address_mode::PyImageAddressMode, image_compare_function::PyImageCompareFunction,
    image_filter_mode::PyImageFilterMode, image_sampler_border_color::PyImageSamplerBorderColor,
};

#[pyfield]
#[pyclass(name = "ImageSamplerDescriptor", eq, from_py_object)]
#[derive(Debug)]
pub struct PyImageSamplerDescriptor {
    storage: FieldStorage<ImageSamplerDescriptor>,
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
        ImageSamplerDescriptor {
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
        }
        .into()
    }

    #[staticmethod]
    pub fn linear() -> Self {
        ImageSamplerDescriptor::linear().into()
    }

    #[staticmethod]
    pub fn nearest() -> Self {
        ImageSamplerDescriptor::nearest().into()
    }

    #[getter]
    pub fn address_mode_u(&self) -> PyResult<PyImageAddressMode> {
        Ok(self.as_ref()?.address_mode_u.into())
    }

    #[setter]
    pub fn set_address_mode_u(&mut self, value: PyImageAddressMode) -> PyResult<()> {
        self.as_mut()?.address_mode_u = value.into();
        Ok(())
    }

    #[getter]
    pub fn address_mode_v(&self) -> PyResult<PyImageAddressMode> {
        Ok(self.as_ref()?.address_mode_v.into())
    }

    #[setter]
    pub fn set_address_mode_v(&mut self, value: PyImageAddressMode) -> PyResult<()> {
        self.as_mut()?.address_mode_v = value.into();
        Ok(())
    }

    #[getter]
    pub fn address_mode_w(&self) -> PyResult<PyImageAddressMode> {
        Ok(self.as_ref()?.address_mode_w.into())
    }

    #[setter]
    pub fn set_address_mode_w(&mut self, value: PyImageAddressMode) -> PyResult<()> {
        self.as_mut()?.address_mode_w = value.into();
        Ok(())
    }

    #[getter]
    pub fn mag_filter(&self) -> PyResult<PyImageFilterMode> {
        Ok(self.as_ref()?.mag_filter.into())
    }

    #[setter]
    pub fn set_mag_filter(&mut self, value: PyImageFilterMode) -> PyResult<()> {
        self.as_mut()?.mag_filter = value.into();
        Ok(())
    }

    #[getter]
    pub fn min_filter(&self) -> PyResult<PyImageFilterMode> {
        Ok(self.as_ref()?.min_filter.into())
    }

    #[setter]
    pub fn set_min_filter(&mut self, value: PyImageFilterMode) -> PyResult<()> {
        self.as_mut()?.min_filter = value.into();
        Ok(())
    }

    #[getter]
    pub fn mipmap_filter(&self) -> PyResult<PyImageFilterMode> {
        Ok(self.as_ref()?.mipmap_filter.into())
    }

    #[setter]
    pub fn set_mipmap_filter(&mut self, value: PyImageFilterMode) -> PyResult<()> {
        self.as_mut()?.mipmap_filter = value.into();
        Ok(())
    }

    #[getter]
    pub fn lod_min_clamp(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.lod_min_clamp)
    }

    #[setter]
    pub fn set_lod_min_clamp(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.lod_min_clamp = value;
        Ok(())
    }

    #[getter]
    pub fn lod_max_clamp(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.lod_max_clamp)
    }

    #[setter]
    pub fn set_lod_max_clamp(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.lod_max_clamp = value;
        Ok(())
    }

    #[getter]
    pub fn compare(&self) -> PyResult<Option<PyImageCompareFunction>> {
        Ok(self.as_ref()?.compare.map(Into::into))
    }

    #[setter]
    pub fn set_compare(&mut self, value: Option<PyImageCompareFunction>) -> PyResult<()> {
        self.as_mut()?.compare = value.map(Into::into);
        Ok(())
    }

    #[getter]
    pub fn anisotropy_clamp(&self) -> PyResult<u16> {
        Ok(self.as_ref()?.anisotropy_clamp)
    }

    #[setter]
    pub fn set_anisotropy_clamp(&mut self, value: u16) -> PyResult<()> {
        self.as_mut()?.anisotropy_clamp = value;
        Ok(())
    }

    #[getter]
    pub fn border_color(&self) -> PyResult<Option<PyImageSamplerBorderColor>> {
        Ok(self.as_ref()?.border_color.map(Into::into))
    }

    #[setter]
    pub fn set_border_color(&mut self, value: Option<PyImageSamplerBorderColor>) -> PyResult<()> {
        self.as_mut()?.border_color = value.map(Into::into);
        Ok(())
    }

    #[getter]
    pub fn label(&self) -> PyResult<Option<String>> {
        Ok(self.as_ref()?.label.clone())
    }

    #[setter]
    pub fn set_label(&mut self, value: Option<String>) -> PyResult<()> {
        self.as_mut()?.label = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let descriptor = self.as_ref()?;
        Ok(format!(
            "ImageSamplerDescriptor(mag_filter={:?}, min_filter={:?}, mipmap_filter={:?})",
            descriptor.mag_filter, descriptor.min_filter, descriptor.mipmap_filter
        ))
    }
}

impl PyImageSamplerDescriptor {
    pub fn to_image_sampler(&self) -> PyResult<ImageSampler> {
        Ok(ImageSampler::Descriptor(self.as_ref()?.clone()))
    }
}

impl PartialEq for PyImageSamplerDescriptor {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(left), Ok(right)) => *left == *right,
            _ => false,
        }
    }
}
