use bevy::{asset::Handle, mesh::skinning::SkinnedMesh};
use pybevy_core::{
    ComponentStorage, Omittable, PyComponent, PyEntity, PyEntityLiveList, PyHandle,
    repr::object_repr,
};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyList};

#[pycomponent(SkinnedMesh, bridge)]
#[pyclass(name = "SkinnedMesh", module = "pybevy.mesh", extends = PyComponent)]
#[derive(Debug)]
pub struct PySkinnedMesh {
    pub(crate) storage: ComponentStorage<SkinnedMesh>,
}

#[pymethods]
impl PySkinnedMesh {
    #[new]
    #[pyo3(signature = (*, inverse_bindposes = Omittable::default(), joints = Omittable::default()))]
    pub fn new(
        inverse_bindposes: Omittable<Bound<'_, PyAny>>,
        joints: Omittable<Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let inverse_bindposes = match inverse_bindposes.into_option() {
            Some(inverse_bindposes) => {
                let handle = inverse_bindposes
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
                (&*handle).try_into()?
            }
            None => Handle::default(),
        };
        let joints = match joints.into_option() {
            Some(joints) => {
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
                joints
                    .iter()
                    .map(|joint| {
                        joint.extract::<PyEntity>().map(|entity| entity.0).map_err(|_| {
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
                    .collect::<PyResult<Vec<_>>>()?
            }
            None => Vec::new(),
        };

        Ok(Self::from_owned(SkinnedMesh {
            inverse_bindposes,
            joints,
        })
        .into())
    }

    #[getter]
    pub fn inverse_bindposes(&self) -> PyResult<PyHandle> {
        Ok(PyHandle::from(&self.as_ref()?.inverse_bindposes))
    }

    #[setter]
    pub fn set_inverse_bindposes(&mut self, value: &PyHandle) -> PyResult<()> {
        self.as_mut()?.inverse_bindposes = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn joints(&self) -> PyResult<PyEntityLiveList> {
        Ok(self.storage.borrow_field_as(|mesh| &mesh.joints)?)
    }

    #[setter]
    pub fn set_joints(&mut self, value: Vec<PyEntity>) -> PyResult<()> {
        self.as_mut()?.joints = value.into_iter().map(|entity| entity.0).collect();
        Ok(())
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(mesh) => format!(
                "SkinnedMesh(joints={}, inverse_bindposes={})",
                mesh.joints.len(),
                object_repr(PyHandle::from(&mesh.inverse_bindposes))
            ),
            Err(_) => "SkinnedMesh(<invalid>)".to_string(),
        }
    }
}
