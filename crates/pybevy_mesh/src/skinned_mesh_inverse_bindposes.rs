use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use pybevy_core::{AssetStorage, PyAsset};
use pybevy_macros::pyasset;
use pybevy_math::mat4::PyMat4;
use pyo3::prelude::*;

#[pyasset(SkinnedMeshInverseBindposes, no_clone, bridge, not_loadable)]
#[pyclass(name = "SkinnedMeshInverseBindposes", extends = PyAsset)]
#[derive(Debug)]
pub struct PySkinnedMeshInverseBindposes {
    pub storage: AssetStorage<SkinnedMeshInverseBindposes>,
}

#[pymethods]
impl PySkinnedMeshInverseBindposes {
    #[new]
    pub fn new(matrices: Vec<PyMat4>) -> PyClassInitializer<Self> {
        let mat4s: Vec<bevy::math::Mat4> = matrices.into_iter().map(|m| m.into()).collect();
        Self::from_owned(SkinnedMeshInverseBindposes::from(mat4s)).into()
    }

    pub fn __len__(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.len())
    }

    pub fn get(&self, index: usize) -> PyResult<Option<PyMat4>> {
        Ok(self.as_ref()?.get(index).map(|m| (*m).into()))
    }

    pub fn to_list(&self) -> PyResult<Vec<PyMat4>> {
        Ok(self.as_ref()?.iter().map(|m| (*m).into()).collect())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "SkinnedMeshInverseBindposes({} matrices)",
            self.as_ref()?.len()
        ))
    }
}
