use std::{
    collections::HashSet,
    sync::{LazyLock, Mutex, PoisonError},
};

use bevy::render::render_resource::TextureViewDescriptor;
use pybevy_core::{FieldStorage, FromBorrowedStorage, domain::IntegerInput};
use pybevy_macros::pyfield;
use pyo3::prelude::*;

use crate::{
    texture_aspect::PyTextureAspect, texture_format::PyTextureFormat,
    texture_usages::PyTextureUsages, texture_view_dimension::PyTextureViewDimension,
};

static LABELS: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Mutex::default);

/// Bevy stores a `&'static str` label, so each distinct Python label is leaked once and reused.
fn intern_label(label: &str) -> &'static str {
    let mut labels = LABELS.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(interned) = labels.get(label) {
        return interned;
    }
    let interned: &'static str = Box::leak(label.to_owned().into_boxed_str());
    labels.insert(interned);
    interned
}

#[pyfield(domains = [mip_level_count => [positive], array_layer_count => [positive]])]
#[pyclass(
    name = "TextureViewDescriptor",
    module = "pybevy.render",
    from_py_object
)]
#[derive(Debug)]
pub struct PyTextureViewDescriptor {
    storage: FieldStorage<TextureViewDescriptor<'static>>,
}

#[pymethods]
impl PyTextureViewDescriptor {
    #[new]
    #[pyo3(signature = (
        *,
        label = None,
        format = None,
        dimension = None,
        usage = None,
        aspect = PyTextureAspect::All,
        base_mip_level = 0,
        mip_level_count = None,
        base_array_layer = 0,
        array_layer_count = None
    ))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        label: Option<String>,
        format: Option<PyTextureFormat>,
        dimension: Option<PyTextureViewDimension>,
        usage: Option<PyTextureUsages>,
        aspect: PyTextureAspect,
        base_mip_level: u32,
        mip_level_count: Option<IntegerInput<u32>>,
        base_array_layer: u32,
        array_layer_count: Option<IntegerInput<u32>>,
    ) -> PyResult<Self> {
        let mip_level_count =
            Self::check_mip_level_count(mip_level_count)?.map(IntegerInput::stored);
        let array_layer_count =
            Self::check_array_layer_count(array_layer_count)?.map(IntegerInput::stored);
        Ok(Self::from_owned(TextureViewDescriptor {
            label: label.as_deref().map(intern_label),
            format: format.map(Into::into),
            dimension: dimension.map(Into::into),
            usage: usage.map(TryInto::try_into).transpose()?,
            aspect: aspect.into(),
            base_mip_level,
            mip_level_count,
            base_array_layer,
            array_layer_count,
        }))
    }

    #[getter]
    pub fn label(&self) -> PyResult<Option<String>> {
        Ok(self.as_ref()?.label.map(str::to_string))
    }

    #[setter]
    pub fn set_label(&mut self, label: Option<String>) -> PyResult<()> {
        self.as_mut()?.label = label.as_deref().map(intern_label);
        Ok(())
    }

    #[getter]
    pub fn format(&self) -> PyResult<Option<PyTextureFormat>> {
        self.as_ref()?
            .format
            .map(PyTextureFormat::try_from)
            .transpose()
    }

    #[setter]
    pub fn set_format(&mut self, format: Option<PyTextureFormat>) -> PyResult<()> {
        self.as_mut()?.format = format.map(Into::into);
        Ok(())
    }

    #[getter]
    pub fn dimension(&self) -> PyResult<Option<PyTextureViewDimension>> {
        Ok(self.as_ref()?.dimension.map(Into::into))
    }

    #[setter]
    pub fn set_dimension(&mut self, dimension: Option<PyTextureViewDimension>) -> PyResult<()> {
        self.as_mut()?.dimension = dimension.map(Into::into);
        Ok(())
    }

    #[getter]
    pub fn usage(&self) -> PyResult<Option<PyTextureUsages>> {
        Ok(self
            .storage
            .borrow_optional_field(
                "TextureViewDescriptor.usage",
                |descriptor| &descriptor.usage,
                |descriptor| &mut descriptor.usage,
            )?
            .map(<PyTextureUsages as FromBorrowedStorage<_>>::from_borrowed))
    }

    #[setter]
    pub fn set_usage(&mut self, usage: Option<PyTextureUsages>) -> PyResult<()> {
        let usage = usage.map(TryInto::try_into).transpose()?;
        self.as_mut()?.usage = usage;
        Ok(())
    }

    #[getter]
    pub fn aspect(&self) -> PyResult<PyTextureAspect> {
        Ok(self.as_ref()?.aspect.into())
    }

    #[setter]
    pub fn set_aspect(&mut self, aspect: PyTextureAspect) -> PyResult<()> {
        self.as_mut()?.aspect = aspect.into();
        Ok(())
    }

    #[getter]
    pub fn base_mip_level(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.base_mip_level)
    }

    #[setter]
    pub fn set_base_mip_level(&mut self, base_mip_level: u32) -> PyResult<()> {
        self.as_mut()?.base_mip_level = base_mip_level;
        Ok(())
    }

    #[getter]
    pub fn mip_level_count(&self) -> PyResult<Option<u32>> {
        Ok(self.as_ref()?.mip_level_count)
    }

    #[setter]
    pub fn set_mip_level_count(
        &mut self,
        mip_level_count: Option<IntegerInput<u32>>,
    ) -> PyResult<()> {
        self.as_mut()?.mip_level_count =
            Self::check_mip_level_count(mip_level_count)?.map(IntegerInput::stored);
        Ok(())
    }

    #[getter]
    pub fn base_array_layer(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.base_array_layer)
    }

    #[setter]
    pub fn set_base_array_layer(&mut self, base_array_layer: u32) -> PyResult<()> {
        self.as_mut()?.base_array_layer = base_array_layer;
        Ok(())
    }

    #[getter]
    pub fn array_layer_count(&self) -> PyResult<Option<u32>> {
        Ok(self.as_ref()?.array_layer_count)
    }

    #[setter]
    pub fn set_array_layer_count(
        &mut self,
        array_layer_count: Option<IntegerInput<u32>>,
    ) -> PyResult<()> {
        self.as_mut()?.array_layer_count =
            Self::check_array_layer_count(array_layer_count)?.map(IntegerInput::stored);
        Ok(())
    }

    fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from_owned(self.as_ref()?.clone()))
    }
}
