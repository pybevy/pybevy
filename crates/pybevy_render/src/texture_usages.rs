use bevy::render::render_resource::TextureUsages;
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

#[pyvalue]
#[pyclass(name = "TextureUsages", module = "pybevy.render", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyTextureUsages {
    storage: ValueStorage<TextureUsages>,
}

impl From<TextureUsages> for PyTextureUsages {
    fn from(value: TextureUsages) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyTextureUsages> for TextureUsages {
    type Error = PyErr;

    fn try_from(value: PyTextureUsages) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyTextureUsages> for TextureUsages {
    type Error = PyErr;

    fn try_from(value: &PyTextureUsages) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl Default for PyTextureUsages {
    fn default() -> Self {
        Self::new()
    }
}

#[pymethods]
impl PyTextureUsages {
    #[new]
    pub fn new() -> Self {
        TextureUsages::empty().into()
    }

    #[staticmethod]
    #[pyo3(name = "COPY_SRC")]
    pub fn copy_src_flag() -> Self {
        TextureUsages::COPY_SRC.into()
    }

    #[staticmethod]
    #[pyo3(name = "COPY_DST")]
    pub fn copy_dst_flag() -> Self {
        TextureUsages::COPY_DST.into()
    }

    #[staticmethod]
    #[pyo3(name = "TEXTURE_BINDING")]
    pub fn texture_binding_flag() -> Self {
        TextureUsages::TEXTURE_BINDING.into()
    }

    #[staticmethod]
    #[pyo3(name = "STORAGE_BINDING")]
    pub fn storage_binding_flag() -> Self {
        TextureUsages::STORAGE_BINDING.into()
    }

    #[staticmethod]
    #[pyo3(name = "RENDER_ATTACHMENT")]
    pub fn render_attachment_flag() -> Self {
        TextureUsages::RENDER_ATTACHMENT.into()
    }

    #[staticmethod]
    #[pyo3(name = "STORAGE_ATOMIC")]
    pub fn storage_atomic_flag() -> Self {
        TextureUsages::STORAGE_ATOMIC.into()
    }

    #[staticmethod]
    #[pyo3(name = "TRANSIENT")]
    pub fn transient_flag() -> Self {
        TextureUsages::TRANSIENT.into()
    }

    fn __or__(&self, other: &Self) -> PyResult<Self> {
        Ok((self.to_bevy()? | other.to_bevy()?).into())
    }

    fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.to_bevy()? == other.to_bevy()?)
    }

    fn __copy__(&self) -> PyResult<Self> {
        Ok(self.to_bevy()?.into())
    }

    pub fn contains(&self, other: &Self) -> PyResult<bool> {
        Ok(self.to_bevy()?.contains(other.to_bevy()?))
    }

    pub fn insert(slf: &Bound<'_, Self>, other: Self) -> PyResult<()> {
        let other = other.to_bevy()?;
        slf.try_borrow_mut()?.as_mut()?.insert(other);
        Ok(())
    }

    pub fn remove(slf: &Bound<'_, Self>, other: Self) -> PyResult<()> {
        let other = other.to_bevy()?;
        slf.try_borrow_mut()?.as_mut()?.remove(other);
        Ok(())
    }

    pub fn toggle(slf: &Bound<'_, Self>, other: Self) -> PyResult<()> {
        let other = other.to_bevy()?;
        slf.try_borrow_mut()?.as_mut()?.toggle(other);
        Ok(())
    }

    pub fn set(slf: &Bound<'_, Self>, other: Self, value: bool) -> PyResult<()> {
        let other = other.to_bevy()?;
        slf.try_borrow_mut()?.as_mut()?.set(other, value);
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let usages = self.to_bevy()?;
        let mut parts = Vec::new();
        if usages.contains(TextureUsages::COPY_SRC) {
            parts.push("COPY_SRC");
        }
        if usages.contains(TextureUsages::COPY_DST) {
            parts.push("COPY_DST");
        }
        if usages.contains(TextureUsages::TEXTURE_BINDING) {
            parts.push("TEXTURE_BINDING");
        }
        if usages.contains(TextureUsages::STORAGE_BINDING) {
            parts.push("STORAGE_BINDING");
        }
        if usages.contains(TextureUsages::RENDER_ATTACHMENT) {
            parts.push("RENDER_ATTACHMENT");
        }
        if usages.contains(TextureUsages::STORAGE_ATOMIC) {
            parts.push("STORAGE_ATOMIC");
        }
        if usages.contains(TextureUsages::TRANSIENT) {
            parts.push("TRANSIENT");
        }
        Ok(if parts.is_empty() {
            "TextureUsages()".to_string()
        } else {
            format!("TextureUsages({})", parts.join(" | "))
        })
    }
}
