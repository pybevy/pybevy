use bevy::camera::Camera;
use pybevy_core::{ComponentStorage, FromBorrowedStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::{
    mat4::PyMat4, ray::PyRay3d, rect::PyRect, urect::PyURect, uvec2::PyUVec2, vec2::PyVec2,
    vec3::PyVec3,
};
use pybevy_transform::global_transform::PyGlobalTransform;
use pyo3::{exceptions::PyValueError, prelude::*};

use super::{
    clear_color_config::PyClearColorConfig, msaa_writeback::PyMsaaWriteback,
    sub_camera_view::PySubCameraView, viewport::PyViewport,
};

#[pycomponent(Camera, bridge, view_fields = [is_active])]
#[pyclass(name = "Camera", extends = PyComponent)]
pub struct PyCamera {
    pub(crate) storage: ComponentStorage<Camera>,
}

#[pymethods]
impl PyCamera {
    #[new]
    #[pyo3(signature = (
        is_active = true,
        *,
        order = 0,
        msaa_writeback = PyMsaaWriteback::Auto,
        clear_color = None,
        viewport = None,
        sub_camera_view = None,
    ))]
    pub fn new(
        is_active: bool,
        order: isize,
        msaa_writeback: PyMsaaWriteback,
        clear_color: Option<PyClearColorConfig>,
        viewport: Option<&PyViewport>,
        sub_camera_view: Option<PySubCameraView>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let mut camera = Camera {
            is_active,
            order,
            msaa_writeback: msaa_writeback.into(),
            ..Default::default()
        };
        if let Some(cc) = clear_color {
            camera.clear_color = cc.into();
        }
        if let Some(v) = viewport {
            camera.viewport = Some(v.try_into()?);
        }
        if let Some(scv) = sub_camera_view {
            camera.sub_camera_view = Some(scv.try_into()?);
        }
        Ok(Self::from_owned(camera).into())
    }

    #[getter]
    pub fn viewport(&self) -> PyResult<Option<PyViewport>> {
        Ok(self
            .storage
            .borrow_optional_field(|c| &c.viewport)?
            .map(PyViewport::from_borrowed))
    }

    #[setter]
    pub fn set_viewport(&mut self, value: Option<&PyViewport>) -> PyResult<()> {
        let mut camera = self.as_mut()?;
        camera.viewport = match value {
            Some(py_viewport) => Some(py_viewport.try_into()?),
            None => None,
        };
        Ok(())
    }

    #[getter]
    pub fn is_active(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_active)
    }

    #[setter]
    pub fn set_is_active(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.is_active = value;
        Ok(())
    }

    #[getter]
    pub fn order(&self) -> PyResult<isize> {
        Ok(self.as_ref()?.order)
    }

    #[setter]
    pub fn set_order(&mut self, value: isize) -> PyResult<()> {
        self.as_mut()?.order = value;
        Ok(())
    }

    #[getter]
    pub fn msaa_writeback(&self) -> PyResult<PyMsaaWriteback> {
        Ok(self.as_ref()?.msaa_writeback.into())
    }

    #[setter]
    pub fn set_msaa_writeback(&mut self, value: PyMsaaWriteback) -> PyResult<()> {
        self.as_mut()?.msaa_writeback = value.into();
        Ok(())
    }

    #[getter]
    pub fn invert_culling(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.invert_culling)
    }

    #[setter]
    pub fn set_invert_culling(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.invert_culling = value;
        Ok(())
    }

    #[getter]
    pub fn clear_color(&self) -> PyResult<PyClearColorConfig> {
        Ok(self.as_ref()?.clear_color.into())
    }

    #[setter]
    pub fn set_clear_color(&mut self, value: PyClearColorConfig) -> PyResult<()> {
        self.as_mut()?.clear_color = value.into();
        Ok(())
    }

    #[getter]
    pub fn sub_camera_view(&self) -> PyResult<Option<PySubCameraView>> {
        Ok(self
            .storage
            .borrow_optional_field(|c| &c.sub_camera_view)?
            .map(PySubCameraView::from_borrowed))
    }

    #[setter]
    pub fn set_sub_camera_view(&mut self, value: Option<PySubCameraView>) -> PyResult<()> {
        let converted = match value {
            Some(scv) => Some(bevy::camera::SubCameraView::try_from(scv)?),
            None => None,
        };
        self.as_mut()?.sub_camera_view = converted;
        Ok(())
    }

    pub fn to_logical(&self, physical_size: PyUVec2) -> PyResult<Option<PyVec2>> {
        Ok(self
            .as_ref()?
            .to_logical(physical_size.try_into()?)
            .map(|v| v.into()))
    }

    pub fn physical_viewport_rect(&self) -> PyResult<Option<PyURect>> {
        Ok(self.as_ref()?.physical_viewport_rect().map(|r| r.into()))
    }

    pub fn logical_viewport_rect(&self) -> PyResult<Option<PyRect>> {
        Ok(self.as_ref()?.logical_viewport_rect().map(|r| r.into()))
    }

    pub fn logical_viewport_size(&self) -> PyResult<Option<PyVec2>> {
        Ok(self.as_ref()?.logical_viewport_size().map(|v| v.into()))
    }

    pub fn physical_viewport_size(&self) -> PyResult<Option<PyUVec2>> {
        Ok(self.as_ref()?.physical_viewport_size().map(|v| v.into()))
    }

    pub fn logical_target_size(&self) -> PyResult<Option<PyVec2>> {
        Ok(self.as_ref()?.logical_target_size().map(|v| v.into()))
    }

    pub fn physical_target_size(&self) -> PyResult<Option<PyUVec2>> {
        Ok(self.as_ref()?.physical_target_size().map(|v| v.into()))
    }

    pub fn target_scaling_factor(&self) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.target_scaling_factor())
    }

    pub fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(camera) => {
                let viewport_str = if camera.viewport.is_some() {
                    "Some(...)"
                } else {
                    "None"
                };
                format!(
                    "Camera(is_active={}, order={}, viewport={})",
                    if camera.is_active { "True" } else { "False" },
                    camera.order,
                    viewport_str
                )
            }
            Err(_) => "Camera(<invalid>)".to_string(),
        }
    }

    pub fn viewport_to_world_2d(
        &self,
        camera_transform: &PyGlobalTransform,
        viewport_position: PyVec2,
    ) -> PyResult<PyVec2> {
        let camera = self.as_ref()?;
        let transform = camera_transform.as_ref()?;
        let viewport_pos = viewport_position.try_into()?;

        camera
            .viewport_to_world_2d(&transform, viewport_pos)
            .map(|v| v.into())
            .map_err(|e| PyValueError::new_err(format!("Viewport conversion failed: {:?}", e)))
    }

    pub fn world_to_viewport(
        &self,
        camera_transform: &PyGlobalTransform,
        world_position: PyVec3,
    ) -> PyResult<PyVec2> {
        let camera = self.as_ref()?;
        let transform = camera_transform.as_ref()?;
        let world_pos = world_position.try_into()?;

        camera
            .world_to_viewport(&transform, world_pos)
            .map(|v| v.into())
            .map_err(|e| PyValueError::new_err(format!("Viewport conversion failed: {:?}", e)))
    }

    pub fn viewport_to_world(
        &self,
        camera_transform: &PyGlobalTransform,
        viewport_position: PyVec2,
    ) -> PyResult<PyRay3d> {
        let camera = self.as_ref()?;
        let transform = camera_transform.as_ref()?;
        let viewport_pos = viewport_position.try_into()?;

        camera
            .viewport_to_world(&transform, viewport_pos)
            .map(|ray| ray.into())
            .map_err(|e| PyValueError::new_err(format!("Viewport conversion failed: {:?}", e)))
    }

    pub fn world_to_viewport_with_depth(
        &self,
        camera_transform: &PyGlobalTransform,
        world_position: PyVec3,
    ) -> PyResult<PyVec3> {
        let camera = self.as_ref()?;
        let transform = camera_transform.as_ref()?;
        let world_pos = world_position.try_into()?;

        camera
            .world_to_viewport_with_depth(&transform, world_pos)
            .map(|v| v.into())
            .map_err(|e| PyValueError::new_err(format!("Viewport conversion failed: {:?}", e)))
    }

    pub fn clip_from_view(&self) -> PyResult<PyMat4> {
        Ok(self.as_ref()?.clip_from_view().into())
    }

    pub fn depth_ndc_to_view_z(&self, ndc_depth: f32) -> PyResult<f32> {
        Ok(self.as_ref()?.depth_ndc_to_view_z(ndc_depth))
    }

    pub fn depth_ndc_to_view_z_2d(&self, ndc_depth: f32) -> PyResult<f32> {
        Ok(self.as_ref()?.depth_ndc_to_view_z_2d(ndc_depth))
    }
}
