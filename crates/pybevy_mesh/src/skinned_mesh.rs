use bevy::mesh::skinning::SkinnedMesh;
use pybevy_core::{ComponentStorage, PyComponent, PyEntity, PyHandle};
use pybevy_macros::pycomponent;
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::PyList,
};

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
        inverse_bindposes: &Bound<'_, PyAny>,
        joints: &Bound<'_, PyAny>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let inverse_bindposes =
            inverse_bindposes
                .extract::<PyRef<'_, PyHandle>>()
                .map_err(|_| {
                    PyTypeError::new_err(format!(
                        "SkinnedMesh() inverse_bindposes expects a Handle, got '{}'",
                        inverse_bindposes
                            .get_type()
                            .name()
                            .map(|name| name.to_string())
                            .unwrap_or_else(|_| "unknown".to_string())
                    ))
                })?;
        let joints = joints.cast::<PyList>().map_err(|_| {
            PyTypeError::new_err(format!(
                "SkinnedMesh() joints expects a list, got '{}'",
                joints
                    .get_type()
                    .name()
                    .map(|name| name.to_string())
                    .unwrap_or_else(|_| "unknown".to_string())
            ))
        })?;
        let joints = joints
            .iter()
            .map(|joint| {
                joint.extract::<PyEntity>().map_err(|_| {
                    PyTypeError::new_err(format!(
                        "SkinnedMesh() joints list must contain Entity values, got '{}'",
                        joint
                            .get_type()
                            .name()
                            .map(|name| name.to_string())
                            .unwrap_or_else(|_| "unknown".to_string())
                    ))
                })
            })
            .collect::<PyResult<Vec<_>>>()?;

        Ok(Self::from_owned(SkinnedMesh {
            inverse_bindposes: (&*inverse_bindposes).try_into()?,
            joints: joints.into_iter().map(|e| e.0).collect(),
        })
        .into())
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
