use std::f32::consts::FRAC_PI_4;

use bevy::camera::{
    CameraProjection, CustomProjection, OrthographicProjection, PerspectiveProjection, Projection,
};
use pybevy_core::{ComponentStorage, FieldStorage, FromBorrowedStorage};
use pybevy_macros::{pyenum, pyfield};
use pybevy_math::{mat4::PyMat4, rect::PyRect, vec2::PyVec2, vec3a::PyVec3A};
use pybevy_transform::global_transform::PyGlobalTransform;
use pyo3::prelude::*;

use crate::{frustum::PyFrustum, scaling_mode::PyScalingMode, sub_camera_view::PySubCameraView};

#[pyfield]
#[pyclass(name = "PerspectiveProjection", from_py_object)]
#[derive(Debug)]
pub struct PyPerspectiveProjection {
    pub(crate) storage: FieldStorage<PerspectiveProjection>,
}

#[pymethods]
impl PyPerspectiveProjection {
    #[new]
    #[pyo3(signature = (fov = FRAC_PI_4, aspect_ratio = 1.0, near = 0.1, far = 1000.0))]
    pub fn new(fov: f32, aspect_ratio: f32, near: f32, far: f32) -> Self {
        Self::from_owned(PerspectiveProjection {
            fov,
            aspect_ratio,
            near,
            far,
            ..Default::default()
        })
    }

    #[getter]
    pub fn fov(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.fov)
    }

    #[setter]
    pub fn set_fov(&mut self, fov: f32) -> PyResult<()> {
        self.as_mut()?.fov = fov;
        Ok(())
    }

    #[getter]
    pub fn aspect_ratio(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.aspect_ratio)
    }

    #[setter]
    pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) -> PyResult<()> {
        self.as_mut()?.aspect_ratio = aspect_ratio;
        Ok(())
    }

    #[getter]
    pub fn near(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.near)
    }

    #[setter]
    pub fn set_near(&mut self, near: f32) -> PyResult<()> {
        self.as_mut()?.near = near;
        Ok(())
    }

    #[getter]
    pub fn far(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.far)
    }

    #[setter]
    pub fn set_far(&mut self, far: f32) -> PyResult<()> {
        self.as_mut()?.far = far;
        Ok(())
    }

    #[getter]
    pub fn near_clip_plane(&self) -> PyResult<pybevy_math::vec4::PyVec4> {
        Ok(self
            .storage
            .borrow_field_as(|projection| &projection.near_clip_plane)?)
    }

    #[setter]
    pub fn set_near_clip_plane(&mut self, value: pybevy_math::vec4::PyVec4) -> PyResult<()> {
        self.as_mut()?.near_clip_plane = value.try_into()?;
        Ok(())
    }

    pub fn get_clip_from_view(&self) -> PyResult<PyMat4> {
        Ok(self.as_ref()?.get_clip_from_view().into())
    }

    pub fn update(&mut self, width: f32, height: f32) -> PyResult<()> {
        self.as_mut()?.update(width, height);
        Ok(())
    }

    pub fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> PyResult<[PyVec3A; 8]> {
        let corners = self.as_ref()?.get_frustum_corners(z_near, z_far);
        Ok([
            corners[0].into(),
            corners[1].into(),
            corners[2].into(),
            corners[3].into(),
            corners[4].into(),
            corners[5].into(),
            corners[6].into(),
            corners[7].into(),
        ])
    }

    pub fn compute_frustum(
        &self,
        py: Python<'_>,
        camera_transform: &PyGlobalTransform,
    ) -> PyResult<Py<PyFrustum>> {
        let frustum = self
            .as_ref()?
            .compute_frustum(camera_transform.as_ref()?.reborrow());
        Py::new(py, PyFrustum::from_owned(frustum))
    }

    pub fn get_clip_from_view_for_sub(&self, sub_view: &PySubCameraView) -> PyResult<PyMat4> {
        let bevy_sub_view: bevy::camera::SubCameraView = sub_view.try_into()?;
        Ok(self
            .as_ref()?
            .get_clip_from_view_for_sub(&bevy_sub_view)
            .into())
    }
}

#[pyfield]
#[pyclass(name = "OrthographicProjection", from_py_object)]
#[derive(Debug)]
pub struct PyOrthographicProjection {
    pub(crate) storage: FieldStorage<OrthographicProjection>,
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
    ) -> PyResult<Self> {
        let mut proj = OrthographicProjection::default_3d();
        if let Some(near) = near {
            proj.near = near;
        }
        if let Some(far) = far {
            proj.far = far;
        }
        if let Some(viewport_origin) = viewport_origin {
            proj.viewport_origin = viewport_origin.try_into()?;
        }
        if let Some(scaling_mode) = scaling_mode {
            proj.scaling_mode = scaling_mode.into();
        }
        if let Some(scale) = scale {
            proj.scale = scale;
        }
        if let Some(area) = area {
            proj.area = area.try_into()?;
        }
        Ok(Self::from_owned(proj))
    }

    #[staticmethod]
    pub fn default_3d() -> Self {
        Self::from_owned(OrthographicProjection::default_3d())
    }

    #[staticmethod]
    pub fn default_2d() -> Self {
        Self::from_owned(OrthographicProjection::default_2d())
    }

    #[getter]
    pub fn near(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.near)
    }

    #[setter]
    pub fn set_near(&mut self, near: f32) -> PyResult<()> {
        self.as_mut()?.near = near;
        Ok(())
    }

    #[getter]
    pub fn far(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.far)
    }

    #[setter]
    pub fn set_far(&mut self, far: f32) -> PyResult<()> {
        self.as_mut()?.far = far;
        Ok(())
    }

    #[getter]
    pub fn viewport_origin(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|p| &p.viewport_origin)?)
    }

    #[setter]
    pub fn set_viewport_origin(&mut self, viewport_origin: PyVec2) -> PyResult<()> {
        self.as_mut()?.viewport_origin = viewport_origin.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn scaling_mode(&self) -> PyResult<PyScalingMode> {
        Ok(self.as_ref()?.scaling_mode.into())
    }

    #[setter]
    pub fn set_scaling_mode(&mut self, scaling_mode: PyScalingMode) -> PyResult<()> {
        self.as_mut()?.scaling_mode = scaling_mode.into();
        Ok(())
    }

    #[getter]
    pub fn scale(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.scale)
    }

    #[setter]
    pub fn set_scale(&mut self, scale: f32) -> PyResult<()> {
        self.as_mut()?.scale = scale;
        Ok(())
    }

    #[getter]
    pub fn area(&self) -> PyResult<PyRect> {
        Ok(self.storage.borrow_field_as(|p| &p.area)?)
    }

    #[setter]
    pub fn set_area(&mut self, area: PyRect) -> PyResult<()> {
        self.as_mut()?.area = area.try_into()?;
        Ok(())
    }

    pub fn get_clip_from_view(&self) -> PyResult<PyMat4> {
        Ok(self.as_ref()?.get_clip_from_view().into())
    }

    pub fn update(&mut self, width: f32, height: f32) -> PyResult<()> {
        self.as_mut()?.update(width, height);
        Ok(())
    }

    pub fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> PyResult<[PyVec3A; 8]> {
        let corners = self.as_ref()?.get_frustum_corners(z_near, z_far);
        Ok([
            corners[0].into(),
            corners[1].into(),
            corners[2].into(),
            corners[3].into(),
            corners[4].into(),
            corners[5].into(),
            corners[6].into(),
            corners[7].into(),
        ])
    }

    pub fn compute_frustum(
        &self,
        py: Python<'_>,
        camera_transform: &PyGlobalTransform,
    ) -> PyResult<Py<PyFrustum>> {
        let frustum = self
            .as_ref()?
            .compute_frustum(camera_transform.as_ref()?.reborrow());
        Py::new(py, PyFrustum::from_owned(frustum))
    }

    pub fn get_clip_from_view_for_sub(&self, sub_view: &PySubCameraView) -> PyResult<PyMat4> {
        let bevy_sub_view: bevy::camera::SubCameraView = sub_view.try_into()?;
        Ok(self
            .as_ref()?
            .get_clip_from_view_for_sub(&bevy_sub_view)
            .into())
    }
}

#[pyenum(Projection, component)]
#[pyclass(name = "Projection", module = "pybevy.camera")]
pub enum PyProjection {
    #[py_bevy(tuple)]
    Perspective {
        #[py_type(PyPerspectiveProjection)]
        #[py_try_into]
        #[py_borrow]
        #[py_set]
        value: PerspectiveProjection,
    },
    #[py_bevy(tuple)]
    Orthographic {
        #[py_type(PyOrthographicProjection)]
        #[py_try_into]
        #[py_borrow]
        #[py_set]
        value: OrthographicProjection,
    },
    #[py_unsupported]
    #[py_bevy(tuple)]
    Custom { value: CustomProjection },
}

#[pymethods]
impl PyProjection {
    pub fn is_perspective(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_perspective())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref()?.reborrow() {
            Projection::Perspective(value) => Ok(format!("Projection.Perspective({value:?})")),
            Projection::Orthographic(value) => Ok(format!("Projection.Orthographic({value:?})")),
            Projection::Custom(value) => Ok(format!("Projection.Custom({value:?})")),
        }
    }
}
