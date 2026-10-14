use bevy::{math::Mat4, mesh::skinning::SkinnedMeshInverseBindposes};
use pybevy_core::{AssetStorage, PyAsset, computed_owned, live_sequence::normalize_index};
use pybevy_macros::pyasset;
use pybevy_math::mat4::PyMat4;
use pyo3::{prelude::*, types::PyList};

#[pyasset(SkinnedMeshInverseBindposes, no_clone, bridge, not_loadable)]
#[pyclass(name = "SkinnedMeshInverseBindposes", module = "pybevy.mesh", extends = PyAsset)]
#[derive(Debug)]
pub struct PySkinnedMeshInverseBindposes {
    pub(crate) storage: AssetStorage<SkinnedMeshInverseBindposes>,
}

#[pymethods]
impl PySkinnedMeshInverseBindposes {
    #[new]
    pub fn new(matrices: Vec<PyMat4>) -> PyResult<PyClassInitializer<Self>> {
        let mat4s: Vec<Mat4> = matrices
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self::from_owned(SkinnedMeshInverseBindposes::from(mat4s)).into())
    }

    pub fn __len__(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.len())
    }

    pub fn get(&self, index: usize) -> PyResult<Option<PyMat4>> {
        Ok(computed_owned(
            self.as_ref()?.get(index).map(|m| (*m).into()),
        ))
    }

    pub fn __getitem__(&self, index: isize) -> PyResult<PyMat4> {
        let bindposes = self.as_ref()?;
        let index = normalize_index(index, bindposes.len())?;
        Ok(computed_owned(bindposes[index].into()))
    }

    pub fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let matrices: Vec<PyMat4> = self
            .as_ref()?
            .iter()
            .map(|matrix| computed_owned((*matrix).into()))
            .collect();
        Ok(PyList::new(py, matrices)?
            .call_method0("__iter__")?
            .unbind())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "SkinnedMeshInverseBindposes({} matrices)",
            self.as_ref()?.len()
        ))
    }
}
