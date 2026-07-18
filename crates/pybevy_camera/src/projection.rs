use std::f32::consts::FRAC_PI_4;

use bevy::camera::{CameraProjection, OrthographicProjection, PerspectiveProjection, Projection};
use pybevy_core::{ComponentStorage, PyComponent, registry::global_registry};
use pybevy_macros::{pycomponent, pyenum};
use pybevy_math::{mat4::PyMat4, rect::PyRect, vec2::PyVec2, vec3a::PyVec3A};
use pybevy_transform::global_transform::PyGlobalTransform;
use pyo3::{PyTypeInfo, prelude::*};

use crate::{frustum::PyFrustum, scaling_mode::PyScalingMode, sub_camera_view::PySubCameraView};

#[pyclass(name = "PerspectiveProjection", from_py_object)]
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

#[pyclass(name = "OrthographicProjection", from_py_object)]
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

#[pyenum(Projection, manual)]
#[pycomponent(Projection, bridge, materialize = materialize_projection)]
#[pyclass(
    name = "Projection",
    module = "pybevy.camera",
    extends = PyComponent,
    subclass
)]
pub struct PyProjection {
    pub storage: ComponentStorage<Projection>,
}

#[pymethods]
impl PyProjection {
    pub fn is_perspective(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_perspective())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref()? {
            Projection::Perspective(value) => Ok(format!("Projection.Perspective({value:?})")),
            Projection::Orthographic(value) => Ok(format!("Projection.Orthographic({value:?})")),
            Projection::Custom(value) => Ok(format!("Projection.Custom({value:?})")),
        }
    }
}

#[pyclass(
    name = "Perspective",
    module = "pybevy.camera",
    extends = PyProjection
)]
pub struct PyProjectionPerspective;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyProjectionPerspective {
    #[classattr]
    const __qualname__: &'static str = "Projection.Perspective";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }

    #[new]
    pub fn new(value: PyPerspectiveProjection) -> PyClassInitializer<Self> {
        projection_initializer(Projection::Perspective(value.into())).add_subclass(Self)
    }

    #[getter]
    pub fn value(slf: PyRef<'_, Self>) -> PyResult<PyPerspectiveProjection> {
        let base = slf.into_super();
        match base.storage.as_ref()? {
            Projection::Perspective(value) => Ok(value.clone().into()),
            _ => unreachable!("Projection.Perspective instance changed discriminant"),
        }
    }

    #[setter]
    pub fn set_value(slf: PyRefMut<'_, Self>, value: PyPerspectiveProjection) -> PyResult<()> {
        let mut base = slf.into_super();
        *base.storage.as_mut()? = Projection::Perspective(value.into());
        Ok(())
    }
}

#[pyclass(
    name = "Orthographic",
    module = "pybevy.camera",
    extends = PyProjection
)]
pub struct PyProjectionOrthographic;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyProjectionOrthographic {
    #[classattr]
    const __qualname__: &'static str = "Projection.Orthographic";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }

    #[new]
    pub fn new(value: PyOrthographicProjection) -> PyClassInitializer<Self> {
        projection_initializer(Projection::Orthographic(value.into())).add_subclass(Self)
    }

    #[getter]
    pub fn value(slf: PyRef<'_, Self>) -> PyResult<PyOrthographicProjection> {
        let base = slf.into_super();
        match base.storage.as_ref()? {
            Projection::Orthographic(value) => Ok(value.clone().into()),
            _ => unreachable!("Projection.Orthographic instance changed discriminant"),
        }
    }

    #[setter]
    pub fn set_value(slf: PyRefMut<'_, Self>, value: PyOrthographicProjection) -> PyResult<()> {
        let mut base = slf.into_super();
        *base.storage.as_mut()? = Projection::Orthographic(value.into());
        Ok(())
    }
}

pub fn materialize_projection(
    py: Python<'_>,
    storage: ComponentStorage<Projection>,
) -> PyResult<Py<PyAny>> {
    enum Variant {
        Perspective,
        Orthographic,
        Custom,
    }

    let variant = match storage.as_ref()? {
        Projection::Perspective(_) => Variant::Perspective,
        Projection::Orthographic(_) => Variant::Orthographic,
        Projection::Custom(_) => Variant::Custom,
    };
    let base = PyClassInitializer::from(PyComponent).add_subclass(PyProjection { storage });

    match variant {
        Variant::Perspective => {
            Ok(Py::new(py, base.add_subclass(PyProjectionPerspective))?.into_any())
        }
        Variant::Orthographic => {
            Ok(Py::new(py, base.add_subclass(PyProjectionOrthographic))?.into_any())
        }
        Variant::Custom => Ok(Py::new(py, base)?.into_any()),
    }
}

fn projection_initializer(value: Projection) -> PyClassInitializer<PyProjection> {
    PyClassInitializer::from(PyComponent).add_subclass(PyProjection {
        storage: ComponentStorage::owned(value),
    })
}

pub fn register_projection_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let base = module.getattr("Projection")?;
    base.setattr("Perspective", py.get_type::<PyProjectionPerspective>())?;
    base.setattr("Orthographic", py.get_type::<PyProjectionOrthographic>())?;

    let canonical = PyProjection::type_object_raw(py);
    for alias in [
        PyProjectionPerspective::type_object_raw(py),
        PyProjectionOrthographic::type_object_raw(py),
    ] {
        if !global_registry::register_component_bridge_alias(alias, canonical) {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Projection bridge was not registered before its variants",
            ));
        }
    }
    Ok(())
}
