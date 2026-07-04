use bevy::mesh::skinning::SkinnedMesh;
use pybevy_core::{ComponentStorage, PyComponent, PyEntity, PyHandle};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyValueError, prelude::*};

#[pycomponent(SkinnedMesh, bridge)]
#[pyclass(name = "SkinnedMesh", extends = PyComponent)]
#[derive(Debug)]
pub struct PySkinnedMesh {
    pub(crate) storage: ComponentStorage<SkinnedMesh>,
}

#[pymethods]
impl PySkinnedMesh {
    #[new]
    pub fn new(
        inverse_bindposes: PyHandle,
        joints: Vec<PyEntity>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(SkinnedMesh {
            inverse_bindposes: inverse_bindposes.try_into()?,
            joints: joints.into_iter().map(|e| e.0).collect(),
        }).into())
    }

    #[getter]
    pub fn inverse_bindposes(&self) -> PyResult<PyHandle> {
        Ok(PyHandle::from(&self.as_ref()?.inverse_bindposes))
    }

    #[getter]
    pub fn joints(&self) -> PyResult<Vec<PyEntity>> {
        Ok(self.as_ref()?.joints.iter().map(|&e| PyEntity(e)).collect())
    }

    pub fn joint_count(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.joints.len())
    }

    pub fn get_joint(&self, index: usize) -> PyResult<PyEntity> {
        let joints = &self.as_ref()?.joints;
        joints.get(index).copied().map(PyEntity).ok_or_else(|| {
            PyValueError::new_err(format!(
                "Joint index {} out of bounds (total joints: {})",
                index,
                joints.len()
            ))
        })
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(mesh) => format!(
                "SkinnedMesh(joints={}, inverse_bindposes=Handle({:?}))",
                mesh.joints.len(),
                PyHandle::from(&mesh.inverse_bindposes)
            ),
            Err(_) => "SkinnedMesh(<invalid>)".to_string(),
        }
    }

    fn __len__(&self) -> PyResult<usize> {
        self.joint_count()
    }
}
