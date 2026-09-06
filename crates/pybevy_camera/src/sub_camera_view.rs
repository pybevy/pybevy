use bevy::camera::SubCameraView;
use pybevy_core::{FromBorrowedStorage, StorageMut, StorageRef, ValueStorage};
use pybevy_math::{uvec2::PyUVec2, vec2::PyVec2};
use pyo3::prelude::*;

#[pyclass(name = "SubCameraView", module = "pybevy.camera", from_py_object)]
#[derive(Debug, Clone)]
pub struct PySubCameraView {
    storage: ValueStorage<SubCameraView>,
}

impl Default for PySubCameraView {
    fn default() -> Self {
        Self {
            storage: ValueStorage::owned(SubCameraView::default()),
        }
    }
}

impl FromBorrowedStorage<ValueStorage<SubCameraView>> for PySubCameraView {
    fn from_borrowed(storage: ValueStorage<SubCameraView>) -> Self {
        PySubCameraView { storage }
    }
}

impl From<SubCameraView> for PySubCameraView {
    fn from(scv: SubCameraView) -> Self {
        Self {
            storage: ValueStorage::owned(scv),
        }
    }
}

impl From<&SubCameraView> for PySubCameraView {
    fn from(scv: &SubCameraView) -> Self {
        Self {
            storage: ValueStorage::owned(*scv),
        }
    }
}

impl TryFrom<PySubCameraView> for SubCameraView {
    type Error = PyErr;

    fn try_from(scv: PySubCameraView) -> Result<Self, Self::Error> {
        Self::try_from(&scv)
    }
}

impl TryFrom<&PySubCameraView> for SubCameraView {
    type Error = PyErr;

    fn try_from(scv: &PySubCameraView) -> Result<Self, Self::Error> {
        Ok(scv.storage.get()?)
    }
}

impl PySubCameraView {
    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, SubCameraView>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<StorageMut<'_, SubCameraView>> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PySubCameraView {
    #[new]
    #[pyo3(signature = (full_size=None, offset=None, size=None))]
    pub fn new(
        full_size: Option<PyUVec2>,
        offset: Option<PyVec2>,
        size: Option<PyUVec2>,
    ) -> PyResult<Self> {
        let default = SubCameraView::default();
        Ok(SubCameraView {
            full_size: full_size
                .map(TryInto::try_into)
                .transpose()?
                .unwrap_or(default.full_size),
            offset: offset
                .map(TryInto::try_into)
                .transpose()?
                .unwrap_or(default.offset),
            size: size
                .map(TryInto::try_into)
                .transpose()?
                .unwrap_or(default.size),
        }
        .into())
    }

    #[getter]
    pub fn full_size(&self) -> PyResult<PyUVec2> {
        Ok(self.storage.borrow_field_as(|s| &s.full_size)?)
    }

    #[setter]
    pub fn set_full_size(&mut self, value: PyUVec2) -> PyResult<()> {
        self.as_mut()?.full_size = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn offset(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|s| &s.offset)?)
    }

    #[setter]
    pub fn set_offset(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.offset = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn size(&self) -> PyResult<PyUVec2> {
        Ok(self.storage.borrow_field_as(|s| &s.size)?)
    }

    #[setter]
    pub fn set_size(&mut self, value: PyUVec2) -> PyResult<()> {
        self.as_mut()?.size = value.try_into()?;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let scv = self.as_ref()?;
        Ok(format!(
            "SubCameraView(full_size={:?}, offset={:?}, size={:?})",
            scv.full_size, scv.offset, scv.size
        ))
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        let a = self.as_ref()?;
        let b = other.as_ref()?;
        Ok(a.full_size == b.full_size && a.offset == b.offset && a.size == b.size)
    }
}
