use bevy::render::render_resource::TextureDescriptor;
use pybevy_core::{FieldStorage, FromBorrowedStorage, domain::IntegerInput};
use pybevy_macros::pyfield;
use pyo3::prelude::*;

use crate::{
    texture_dimension::PyTextureDimension, texture_format::PyTextureFormat,
    texture_usages::PyTextureUsages,
};

#[pyfield(domains = [mip_level_count => [positive]])]
#[pyclass(name = "TextureDescriptor", module = "pybevy.render", from_py_object)]
#[derive(Debug)]
pub struct PyTextureDescriptor {
    storage: FieldStorage<TextureDescriptor<'static>>,
}

#[pymethods]
impl PyTextureDescriptor {
    #[getter]
    pub fn mip_level_count(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.mip_level_count)
    }

    #[setter]
    pub fn set_mip_level_count(&mut self, mip_level_count: IntegerInput<u32>) -> PyResult<()> {
        self.as_mut()?.mip_level_count = Self::check_mip_level_count(mip_level_count)?.stored();
        Ok(())
    }

    #[getter]
    pub fn dimension(&self) -> PyResult<PyTextureDimension> {
        Ok(self.as_ref()?.dimension.into())
    }

    #[setter]
    pub fn set_dimension(&mut self, dimension: PyTextureDimension) -> PyResult<()> {
        self.as_mut()?.dimension = dimension.into();
        Ok(())
    }

    #[getter]
    pub fn format(&self) -> PyResult<PyTextureFormat> {
        PyTextureFormat::try_from(self.as_ref()?.format)
    }

    #[setter]
    pub fn set_format(&mut self, format: PyTextureFormat) -> PyResult<()> {
        self.as_mut()?.format = format.into();
        Ok(())
    }

    #[getter]
    pub fn usage(&self) -> PyResult<PyTextureUsages> {
        Ok(self.storage.borrow_resolved_field_as(
            |descriptor| &descriptor.usage,
            |descriptor| &mut descriptor.usage,
        )?)
    }

    #[setter]
    pub fn set_usage(&mut self, usage: PyTextureUsages) -> PyResult<()> {
        let usage = usage.try_into()?;
        self.as_mut()?.usage = usage;
        Ok(())
    }

    fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from_owned(self.as_ref()?.clone()))
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }
}
