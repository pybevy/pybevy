use bevy::math::{Mat4, Vec3, primitives::ViewFrustum};
use pyo3::{exceptions::PyValueError, prelude::*};

use super::half_space::PyHalfSpace;
use crate::{mat4::PyMat4, vec3::PyVec3};

#[pyclass(name = "ViewFrustum", eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyViewFrustum {
    pub(crate) inner: ViewFrustum,
}

impl From<ViewFrustum> for PyViewFrustum {
    fn from(frustum: ViewFrustum) -> Self {
        PyViewFrustum { inner: frustum }
    }
}

impl From<PyViewFrustum> for ViewFrustum {
    fn from(frustum: PyViewFrustum) -> Self {
        frustum.inner
    }
}

#[pymethods]
impl PyViewFrustum {
    #[classattr]
    pub const NEAR_PLANE_IDX: usize = ViewFrustum::NEAR_PLANE_IDX;
    #[classattr]
    pub const FAR_PLANE_IDX: usize = ViewFrustum::FAR_PLANE_IDX;

    #[new]
    pub fn new() -> Self {
        ViewFrustum::default().into()
    }

    #[staticmethod]
    pub fn from_clip_from_world(clip_from_world: &PyMat4) -> Self {
        let mat: Mat4 = clip_from_world.into();
        ViewFrustum::from_clip_from_world(&mat).into()
    }

    #[staticmethod]
    pub fn from_clip_from_world_custom_far(
        clip_from_world: &PyMat4,
        view_translation: PyVec3,
        view_backward: PyVec3,
        far: f32,
    ) -> Self {
        let mat: Mat4 = clip_from_world.into();
        let translation: Vec3 = view_translation.into();
        let backward: Vec3 = view_backward.into();
        ViewFrustum::from_clip_from_world_custom_far(&mat, &translation, &backward, far).into()
    }

    pub fn corners(&self) -> Option<Vec<PyVec3>> {
        self.inner
            .corners()
            .map(|corners| corners.into_iter().map(PyVec3::from).collect())
    }

    #[getter]
    pub fn half_spaces(&self) -> Vec<PyHalfSpace> {
        self.inner.half_spaces.iter().map(|&hs| hs.into()).collect()
    }

    #[setter]
    pub fn set_half_spaces(&mut self, half_spaces: Vec<PyHalfSpace>) -> PyResult<()> {
        if half_spaces.len() != 6 {
            return Err(PyValueError::new_err(
                "ViewFrustum requires exactly 6 half-spaces",
            ));
        }
        for (i, hs) in half_spaces.into_iter().enumerate() {
            self.inner.half_spaces[i] = hs.into();
        }
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
