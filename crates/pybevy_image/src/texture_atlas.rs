use bevy::image::TextureAtlas;
use pybevy_core::{
    FromBorrowedStorage, extract_handle_from_any, field_storage::FieldStorage, handle::PyHandle,
};
use pybevy_macros::pyfield;
use pyo3::prelude::*;

#[pyfield]
#[pyclass(name = "TextureAtlas", module = "pybevy.image", eq, from_py_object)]
#[derive(Debug)]
pub struct PyTextureAtlas {
    storage: FieldStorage<TextureAtlas>,
}

#[pymethods]
impl PyTextureAtlas {
    #[new]
    #[pyo3(signature = (layout = None, index = 0))]
    pub fn new(layout: Option<&Bound<'_, PyAny>>, index: usize) -> PyResult<Self> {
        let layout = match layout {
            Some(layout) => extract_handle_from_any(layout)?.try_into()?,
            None => Default::default(),
        };
        Ok(Self {
            storage: FieldStorage::owned(TextureAtlas { layout, index }),
        })
    }

    #[getter]
    pub fn layout(&self) -> PyResult<PyHandle> {
        Ok(PyHandle::from(&self.as_ref()?.layout))
    }

    #[setter]
    pub fn set_layout(&mut self, layout: &Bound<'_, PyAny>) -> PyResult<()> {
        let py_handle = extract_handle_from_any(layout)?;
        self.as_mut()?.layout = py_handle.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn index(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.index)
    }

    #[setter]
    pub fn set_index(&mut self, index: usize) -> PyResult<()> {
        self.as_mut()?.index = index;
        Ok(())
    }

    pub fn with_index(&self, index: usize) -> PyResult<Self> {
        let mut atlas = self.as_ref()?.clone();
        atlas.index = index;
        Ok(Self {
            storage: FieldStorage::owned(atlas),
        })
    }

    pub fn with_layout(&self, layout: &Bound<'_, PyAny>) -> PyResult<Self> {
        let py_handle = extract_handle_from_any(layout)?;
        let mut atlas = self.as_ref()?.clone();
        atlas.layout = py_handle.try_into()?;
        Ok(Self {
            storage: FieldStorage::owned(atlas),
        })
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("TextureAtlas(index={})", self.as_ref()?.index))
    }
}

impl From<&TextureAtlas> for PyTextureAtlas {
    fn from(atlas: &TextureAtlas) -> Self {
        Self {
            storage: FieldStorage::owned(atlas.clone()),
        }
    }
}

impl PyTextureAtlas {
    pub fn from_borrowed_storage(storage: FieldStorage<TextureAtlas>) -> Self {
        Self { storage }
    }
}

impl PartialEq for PyTextureAtlas {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a.layout == b.layout && a.index == b.index,
            _ => false,
        }
    }
}
