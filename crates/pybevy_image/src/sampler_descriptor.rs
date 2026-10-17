use bevy::image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use pybevy_core::{
    FieldStorage, FromBorrowedStorage,
    domain::IntegerInput,
    public_error,
    repr::{float_repr, str_repr},
};
use pybevy_macros::pyfield;
use pyo3::{PyClass, exceptions::PyValueError, prelude::*};

use crate::{
    image_address_mode::PyImageAddressMode, image_compare_function::PyImageCompareFunction,
    image_filter_mode::PyImageFilterMode, image_sampler_border_color::PyImageSamplerBorderColor,
};

/// wgpu rejects an anisotropy_clamp above 1 unless all three filters are Linear.
fn check_anisotropic_filters(
    anisotropy_clamp: u16,
    mag_filter: ImageFilterMode,
    min_filter: ImageFilterMode,
    mipmap_filter: ImageFilterMode,
) -> PyResult<()> {
    if anisotropy_clamp <= 1 {
        return Ok(());
    }
    let filters = [
        ("mag_filter", mag_filter),
        ("min_filter", min_filter),
        ("mipmap_filter", mipmap_filter),
    ];
    match filters
        .into_iter()
        .find(|(_, filter)| *filter != ImageFilterMode::Linear)
    {
        Some((name, _)) => Err(PyValueError::new_err(
            public_error::sampler_anisotropy_requires_linear(
                <PyImageSamplerDescriptor as PyClass>::NAME,
                anisotropy_clamp,
                name,
            ),
        )),
        None => Ok(()),
    }
}

// wgpu requires 0 <= lod_min_clamp <= lod_max_clamp and anisotropy_clamp >= 1.
#[pyfield(domains = [
    lod_min_clamp => [non_negative],
    anisotropy_clamp => [at_least(1)],
    ordered(lod_min_clamp, lod_max_clamp)
])]
#[pyclass(
    name = "ImageSamplerDescriptor",
    module = "pybevy.image",
    eq,
    from_py_object
)]
#[derive(Debug)]
pub struct PyImageSamplerDescriptor {
    storage: FieldStorage<ImageSamplerDescriptor>,
}

#[pymethods]
impl PyImageSamplerDescriptor {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        *,
        label = None,
        address_mode_u = PyImageAddressMode::ClampToEdge,
        address_mode_v = PyImageAddressMode::ClampToEdge,
        address_mode_w = PyImageAddressMode::ClampToEdge,
        mag_filter = PyImageFilterMode::Nearest,
        min_filter = PyImageFilterMode::Nearest,
        mipmap_filter = PyImageFilterMode::Nearest,
        lod_min_clamp = 0.0,
        lod_max_clamp = 32.0,
        compare = None,
        anisotropy_clamp = IntegerInput::new(1),
        border_color = None
    ))]
    pub fn new(
        label: Option<String>,
        address_mode_u: PyImageAddressMode,
        address_mode_v: PyImageAddressMode,
        address_mode_w: PyImageAddressMode,
        mag_filter: PyImageFilterMode,
        min_filter: PyImageFilterMode,
        mipmap_filter: PyImageFilterMode,
        lod_min_clamp: f32,
        lod_max_clamp: f32,
        compare: Option<PyImageCompareFunction>,
        anisotropy_clamp: IntegerInput<u16>,
        border_color: Option<PyImageSamplerBorderColor>,
    ) -> PyResult<Self> {
        let anisotropy_clamp = Self::check_anisotropy_clamp(anisotropy_clamp)?.stored();
        let descriptor = ImageSamplerDescriptor {
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
        };
        Self::validate_domains(&descriptor)?;
        check_anisotropic_filters(
            descriptor.anisotropy_clamp,
            descriptor.mag_filter,
            descriptor.min_filter,
            descriptor.mipmap_filter,
        )?;
        Ok(descriptor.into())
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
        let value: ImageFilterMode = value.into();
        {
            let d = self.as_ref()?;
            check_anisotropic_filters(d.anisotropy_clamp, value, d.min_filter, d.mipmap_filter)?;
        }
        self.as_mut()?.mag_filter = value;
        Ok(())
    }

    #[getter]
    pub fn min_filter(&self) -> PyResult<PyImageFilterMode> {
        Ok(self.as_ref()?.min_filter.into())
    }

    #[setter]
    pub fn set_min_filter(&mut self, value: PyImageFilterMode) -> PyResult<()> {
        let value: ImageFilterMode = value.into();
        {
            let d = self.as_ref()?;
            check_anisotropic_filters(d.anisotropy_clamp, d.mag_filter, value, d.mipmap_filter)?;
        }
        self.as_mut()?.min_filter = value;
        Ok(())
    }

    #[getter]
    pub fn mipmap_filter(&self) -> PyResult<PyImageFilterMode> {
        Ok(self.as_ref()?.mipmap_filter.into())
    }

    #[setter]
    pub fn set_mipmap_filter(&mut self, value: PyImageFilterMode) -> PyResult<()> {
        let value: ImageFilterMode = value.into();
        {
            let d = self.as_ref()?;
            check_anisotropic_filters(d.anisotropy_clamp, d.mag_filter, d.min_filter, value)?;
        }
        self.as_mut()?.mipmap_filter = value;
        Ok(())
    }

    #[getter]
    pub fn lod_min_clamp(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.lod_min_clamp)
    }

    #[setter]
    pub fn set_lod_min_clamp(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_lod_min_clamp(value)?;
        Self::check_lod_min_clamp_order(value, self.as_ref()?.lod_max_clamp)?;
        self.as_mut()?.lod_min_clamp = value;
        Ok(())
    }

    #[getter]
    pub fn lod_max_clamp(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.lod_max_clamp)
    }

    #[setter]
    pub fn set_lod_max_clamp(&mut self, value: f32) -> PyResult<()> {
        Self::check_lod_min_clamp_order(self.as_ref()?.lod_min_clamp, value)?;
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
    pub fn set_anisotropy_clamp(&mut self, value: IntegerInput<u16>) -> PyResult<()> {
        let value = Self::check_anisotropy_clamp(value)?.stored();
        {
            let d = self.as_ref()?;
            check_anisotropic_filters(value, d.mag_filter, d.min_filter, d.mipmap_filter)?;
        }
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

    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from_owned(self.storage.get()?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let descriptor = self.as_ref()?;
        let compare = match descriptor.compare {
            Some(value) => PyImageCompareFunction::from(value).__repr__(),
            None => "None".to_owned(),
        };
        let border_color = match descriptor.border_color {
            Some(value) => PyImageSamplerBorderColor::from(value).__repr__(),
            None => "None".to_owned(),
        };
        let label = match &descriptor.label {
            Some(value) => str_repr(value),
            None => "None".to_owned(),
        };
        Ok(format!(
            "ImageSamplerDescriptor(address_mode_u={}, address_mode_v={}, \
             address_mode_w={}, mag_filter={}, min_filter={}, mipmap_filter={}, \
             lod_min_clamp={}, lod_max_clamp={}, compare={compare}, \
             anisotropy_clamp={}, border_color={border_color}, label={label})",
            PyImageAddressMode::from(descriptor.address_mode_u).__repr__(),
            PyImageAddressMode::from(descriptor.address_mode_v).__repr__(),
            PyImageAddressMode::from(descriptor.address_mode_w).__repr__(),
            PyImageFilterMode::from(descriptor.mag_filter).__repr__(),
            PyImageFilterMode::from(descriptor.min_filter).__repr__(),
            PyImageFilterMode::from(descriptor.mipmap_filter).__repr__(),
            float_repr(descriptor.lod_min_clamp),
            float_repr(descriptor.lod_max_clamp),
            descriptor.anisotropy_clamp,
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
