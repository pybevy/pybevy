use std::f32::consts::FRAC_PI_4;

use bevy::camera::{CameraProjection, OrthographicProjection, PerspectiveProjection, Projection};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pybevy_math::{mat4::PyMat4, rect::PyRect, vec2::PyVec2, vec3a::PyVec3A};
use pybevy_transform::PyGlobalTransform;
use pyo3::{exceptions::PyTypeError, prelude::*};

use crate::{frustum::PyFrustum, scaling_mode::PyScalingMode, sub_camera_view::PySubCameraView};

#[pyclass(name = "PerspectiveProjection")]
#[derive(Clone, Debug)]
pub struct PyPerspectiveProjection {
    pub(crate) inner: PerspectiveProjection,
}

impl From<PerspectiveProjection> for PyPerspectiveProjection {
    fn from(proj: PerspectiveProjection) -> Self {
        Self { inner: proj }
    }
}

impl From<PyPerspectiveProjection> for PerspectiveProjection {
    fn from(proj: PyPerspectiveProjection) -> Self {
        proj.inner
    }
}

#[pymethods]
impl PyPerspectiveProjection {
    #[new]
    #[pyo3(signature = (fov = FRAC_PI_4, aspect_ratio = 1.0, near = 0.1, far = 1000.0))]
    pub fn new(fov: f32, aspect_ratio: f32, near: f32, far: f32) -> Self {
        Self {
            inner: PerspectiveProjection {
                fov,
                aspect_ratio,
                near,
                far,
                ..Default::default()
            },
        }
    }

    #[getter]
    pub fn fov(&self) -> f32 {
        self.inner.fov
    }

    #[setter]
    pub fn set_fov(&mut self, fov: f32) {
        self.inner.fov = fov;
    }

    #[getter]
    pub fn aspect_ratio(&self) -> f32 {
        self.inner.aspect_ratio
    }

    #[setter]
    pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
        self.inner.aspect_ratio = aspect_ratio;
    }

    #[getter]
    pub fn near(&self) -> f32 {
        self.inner.near
    }

    #[setter]
    pub fn set_near(&mut self, near: f32) {
        self.inner.near = near;
    }

    #[getter]
    pub fn far(&self) -> f32 {
        self.inner.far
    }

    #[setter]
    pub fn set_far(&mut self, far: f32) {
        self.inner.far = far;
    }

    #[getter]
    pub fn near_clip_plane(&self) -> pybevy_math::vec4::PyVec4 {
        self.inner.near_clip_plane.into()
    }

    #[setter]
    pub fn set_near_clip_plane(&mut self, value: pybevy_math::vec4::PyVec4) {
        self.inner.near_clip_plane = value.into();
    }

    pub fn get_clip_from_view(&self) -> PyMat4 {
        self.inner.get_clip_from_view().into()
    }

    pub fn update(&mut self, width: f32, height: f32) {
        self.inner.update(width, height);
    }

    pub fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> [PyVec3A; 8] {
        let corners = self.inner.get_frustum_corners(z_near, z_far);
        [
            corners[0].into(),
            corners[1].into(),
            corners[2].into(),
            corners[3].into(),
            corners[4].into(),
            corners[5].into(),
            corners[6].into(),
            corners[7].into(),
        ]
    }

    pub fn compute_frustum(
        &self,
        py: Python<'_>,
        camera_transform: &PyGlobalTransform,
    ) -> PyResult<Py<PyFrustum>> {
        let frustum = self.inner.compute_frustum(camera_transform.as_ref()?);
        Py::new(py, PyFrustum::from_owned(frustum))
    }

    pub fn get_clip_from_view_for_sub(&self, sub_view: &PySubCameraView) -> PyResult<PyMat4> {
        let bevy_sub_view: bevy::camera::SubCameraView = sub_view.into();
        Ok(self.inner.get_clip_from_view_for_sub(&bevy_sub_view).into())
    }
}

#[pyclass(name = "OrthographicProjection")]
#[derive(Clone, Debug)]
pub struct PyOrthographicProjection {
    pub(crate) inner: OrthographicProjection,
}

impl From<OrthographicProjection> for PyOrthographicProjection {
    fn from(proj: OrthographicProjection) -> Self {
        Self { inner: proj }
    }
}

impl From<PyOrthographicProjection> for OrthographicProjection {
    fn from(proj: PyOrthographicProjection) -> Self {
        proj.inner
    }
}

#[pymethods]
impl PyOrthographicProjection {
    #[new]
    #[pyo3(signature = (near=None, far=None, viewport_origin=None, scaling_mode=None, scale=None, area=None))]
    pub fn new(
        near: Option<f32>,
        far: Option<f32>,
        viewport_origin: Option<PyVec2>,
        scaling_mode: Option<PyScalingMode>,
        scale: Option<f32>,
        area: Option<PyRect>,
    ) -> Self {
        let mut proj = OrthographicProjection::default_3d();
        if let Some(near) = near {
            proj.near = near;
        }
        if let Some(far) = far {
            proj.far = far;
        }
        if let Some(viewport_origin) = viewport_origin {
            proj.viewport_origin = viewport_origin.into();
        }
        if let Some(scaling_mode) = scaling_mode {
            proj.scaling_mode = scaling_mode.into();
        }
        if let Some(scale) = scale {
            proj.scale = scale;
        }
        if let Some(area) = area {
            proj.area = area.into();
        }
        Self { inner: proj }
    }

    #[staticmethod]
    pub fn default_3d() -> Self {
        Self {
            inner: OrthographicProjection::default_3d(),
        }
    }

    #[staticmethod]
    pub fn default_2d() -> Self {
        Self {
            inner: OrthographicProjection::default_2d(),
        }
    }

    #[getter]
    pub fn near(&self) -> f32 {
        self.inner.near
    }

    #[setter]
    pub fn set_near(&mut self, near: f32) {
        self.inner.near = near;
    }

    #[getter]
    pub fn far(&self) -> f32 {
        self.inner.far
    }

    #[setter]
    pub fn set_far(&mut self, far: f32) {
        self.inner.far = far;
    }

    #[getter]
    pub fn viewport_origin(&self) -> PyVec2 {
        self.inner.viewport_origin.into()
    }

    #[setter]
    pub fn set_viewport_origin(&mut self, viewport_origin: PyVec2) {
        self.inner.viewport_origin = viewport_origin.into();
    }

    #[getter]
    pub fn scaling_mode(&self) -> PyScalingMode {
        self.inner.scaling_mode.into()
    }

    #[setter]
    pub fn set_scaling_mode(&mut self, scaling_mode: PyScalingMode) {
        self.inner.scaling_mode = scaling_mode.into();
    }

    #[getter]
    pub fn scale(&self) -> f32 {
        self.inner.scale
    }

    #[setter]
    pub fn set_scale(&mut self, scale: f32) {
        self.inner.scale = scale;
    }

    #[getter]
    pub fn area(&self) -> PyRect {
        self.inner.area.into()
    }

    #[setter]
    pub fn set_area(&mut self, area: PyRect) {
        self.inner.area = area.into();
    }

    pub fn get_clip_from_view(&self) -> PyMat4 {
        self.inner.get_clip_from_view().into()
    }

    pub fn update(&mut self, width: f32, height: f32) {
        self.inner.update(width, height);
    }

    pub fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> [PyVec3A; 8] {
        let corners = self.inner.get_frustum_corners(z_near, z_far);
        [
            corners[0].into(),
            corners[1].into(),
            corners[2].into(),
            corners[3].into(),
            corners[4].into(),
            corners[5].into(),
            corners[6].into(),
            corners[7].into(),
        ]
    }

    pub fn compute_frustum(
        &self,
        py: Python<'_>,
        camera_transform: &PyGlobalTransform,
    ) -> PyResult<Py<PyFrustum>> {
        let frustum = self.inner.compute_frustum(camera_transform.as_ref()?);
        Py::new(py, PyFrustum::from_owned(frustum))
    }

    pub fn get_clip_from_view_for_sub(&self, sub_view: &PySubCameraView) -> PyResult<PyMat4> {
        let bevy_sub_view: bevy::camera::SubCameraView = sub_view.into();
        Ok(self.inner.get_clip_from_view_for_sub(&bevy_sub_view).into())
    }
}

#[component_storage(Projection)]
#[pyclass(name = "Projection", extends = PyComponent)]
#[derive(Clone)]
pub struct PyProjection {
    pub storage: ComponentStorage<Projection>,
}

#[pymethods]
impl PyProjection {
    #[new]
    pub fn new() -> PyResult<(Self, PyComponent)> {
        Ok(Self::from_owned(Projection::default()))
    }

    #[staticmethod]
    #[pyo3(name = "Perspective")]
    pub fn perspective(projection: PyPerspectiveProjection) -> PyResult<Py<Self>> {
        Python::attach(|py| {
            Py::new(
                py,
                Self::from_owned(Projection::Perspective(projection.inner)),
            )
        })
    }

    #[staticmethod]
    #[pyo3(name = "Orthographic")]
    pub fn orthographic(projection: PyOrthographicProjection) -> PyResult<Py<Self>> {
        Python::attach(|py| {
            Py::new(
                py,
                Self::from_owned(Projection::Orthographic(projection.inner)),
            )
        })
    }

    pub fn is_perspective(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_perspective())
    }

    pub fn as_orthographic(&self) -> PyResult<PyOrthographicProjection> {
        match self.as_ref()? {
            Projection::Orthographic(ortho) => Ok(PyOrthographicProjection {
                inner: ortho.clone(),
            }),
            Projection::Perspective(_) => Err(PyTypeError::new_err(
                "Projection is Perspective, not Orthographic",
            )),
            Projection::Custom(_) => Err(PyTypeError::new_err(
                "Projection is Custom, not Orthographic",
            )),
        }
    }

    pub fn as_perspective(&self) -> PyResult<PyPerspectiveProjection> {
        match self.as_ref()? {
            Projection::Perspective(persp) => Ok(PyPerspectiveProjection {
                inner: persp.clone(),
            }),
            Projection::Orthographic(_) => Err(PyTypeError::new_err(
                "Projection is Orthographic, not Perspective",
            )),
            Projection::Custom(_) => Err(PyTypeError::new_err(
                "Projection is Custom, not Perspective",
            )),
        }
    }

    #[getter]
    pub fn orthographic_scale(&self) -> PyResult<Option<f32>> {
        match self.as_ref()? {
            Projection::Orthographic(ortho) => Ok(Some(ortho.scale)),
            Projection::Perspective(_) | Projection::Custom(_) => Ok(None),
        }
    }

    #[setter]
    pub fn set_orthographic_scale(&mut self, scale: f32) -> PyResult<()> {
        match self.as_mut()? {
            Projection::Orthographic(ortho) => {
                ortho.scale = scale;
                Ok(())
            }
            Projection::Perspective(_) => Err(PyTypeError::new_err(
                "Cannot set orthographic_scale on Perspective projection",
            )),
            Projection::Custom(_) => Err(PyTypeError::new_err(
                "Cannot set orthographic_scale on Custom projection",
            )),
        }
    }

    #[getter]
    pub fn perspective_fov(&self) -> PyResult<Option<f32>> {
        match self.as_ref()? {
            Projection::Perspective(persp) => Ok(Some(persp.fov)),
            Projection::Orthographic(_) | Projection::Custom(_) => Ok(None),
        }
    }

    #[setter]
    pub fn set_perspective_fov(&mut self, fov: f32) -> PyResult<()> {
        match self.as_mut()? {
            Projection::Perspective(persp) => {
                persp.fov = fov;
                Ok(())
            }
            Projection::Orthographic(_) => Err(PyTypeError::new_err(
                "Cannot set perspective_fov on Orthographic projection",
            )),
            Projection::Custom(_) => Err(PyTypeError::new_err(
                "Cannot set perspective_fov on Custom projection",
            )),
        }
    }
}
