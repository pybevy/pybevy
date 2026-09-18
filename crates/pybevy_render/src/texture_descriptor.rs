use bevy::render::render_resource::TextureDescriptor;
use pybevy_core::{FieldStorage, FromBorrowedStorage};
use pybevy_macros::pyfield;
use pyo3::prelude::*;

use crate::texture_usages::PyTextureUsages;

#[pyfield]
#[pyclass(name = "TextureDescriptor", module = "pybevy.render", from_py_object)]
#[derive(Debug)]
pub struct PyTextureDescriptor {
    storage: FieldStorage<TextureDescriptor<'static>>,
}

#[pymethods]
impl PyTextureDescriptor {
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
}
