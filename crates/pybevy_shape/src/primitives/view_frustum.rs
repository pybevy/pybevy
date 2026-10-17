use bevy::{
    math::{Mat4, Vec3},
    shape::{HalfSpace, ViewFrustum},
};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pybevy_math::{mat4::PyMat4, vec3::PyVec3};
use pyo3::{exceptions::PyValueError, prelude::*};

use super::{half_space::PyHalfSpace, half_spaces::PyHalfSpaces};

#[pyvalue]
#[pyclass(name = "ViewFrustum", module = "pybevy.shape", eq, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyViewFrustum {
    pub(crate) storage: ValueStorage<ViewFrustum>,
}

impl PartialEq for PyViewFrustum {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl From<ViewFrustum> for PyViewFrustum {
    fn from(frustum: ViewFrustum) -> Self {
        Self::from_owned(frustum)
    }
}

impl TryFrom<PyViewFrustum> for ViewFrustum {
    type Error = PyErr;

    fn try_from(frustum: PyViewFrustum) -> PyResult<Self> {
        frustum.to_bevy()
    }
}

impl TryFrom<&PyViewFrustum> for ViewFrustum {
    type Error = PyErr;

    fn try_from(frustum: &PyViewFrustum) -> PyResult<Self> {
        frustum.to_bevy()
    }
}

#[pymethods]
impl PyViewFrustum {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(ViewFrustum::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[classattr]
    pub const NEAR_PLANE_IDX: usize = ViewFrustum::NEAR_PLANE_IDX;
    #[classattr]
    pub const FAR_PLANE_IDX: usize = ViewFrustum::FAR_PLANE_IDX;

    #[new]
    pub fn new() -> Self {
        Self::from_owned(ViewFrustum::default())
    }

    #[staticmethod]
    pub fn from_clip_from_world(clip_from_world: &PyMat4) -> PyResult<Self> {
        let mat: Mat4 = clip_from_world.try_into()?;
        Ok(Self::from_owned(ViewFrustum::from_clip_from_world(&mat)))
    }

    #[staticmethod]
    pub fn from_clip_from_world_custom_far(
        clip_from_world: &PyMat4,
        view_translation: PyVec3,
        view_backward: PyVec3,
        far: f32,
    ) -> PyResult<Self> {
        let mat: Mat4 = clip_from_world.try_into()?;
        let translation: Vec3 = view_translation.try_into()?;
        let backward: Vec3 = view_backward.try_into()?;
        Ok(Self::from_owned(
            ViewFrustum::from_clip_from_world_custom_far(&mat, &translation, &backward, far),
        ))
    }

    pub fn corners(&self) -> PyResult<Option<Vec<PyVec3>>> {
        Ok(self
            .as_ref()?
            .corners()
            .map(|corners| corners.into_iter().map(PyVec3::from).collect()))
    }

    #[getter]
    pub fn half_spaces(&self) -> PyResult<PyHalfSpaces> {
        Ok(self
            .storage
            .borrow_field_as(|frustum| &frustum.half_spaces)?)
    }

    #[setter]
    pub fn set_half_spaces(&mut self, half_spaces: Vec<PyHalfSpace>) -> PyResult<()> {
        if half_spaces.len() != 6 {
            return Err(PyValueError::new_err(
                "ViewFrustum requires exactly 6 half-spaces",
            ));
        }
        let half_spaces: Vec<HalfSpace> = half_spaces.into_iter().map(Into::into).collect();
        self.storage
            .update(|frustum| frustum.half_spaces.clone_from_slice(&half_spaces))?;
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        "ViewFrustum(...)".to_string()
    }
}

impl Default for PyViewFrustum {
    fn default() -> Self {
        Self::new()
    }
}
