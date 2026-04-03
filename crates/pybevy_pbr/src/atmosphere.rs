use bevy::pbr::Atmosphere;
use pybevy_core::{ComponentStorage, PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::pycomponent;
use pybevy_math::vec3::PyVec3;
use pyo3::prelude::*;

#[pycomponent(Atmosphere, bridge, view_only_fields = [bottom_radius: f32, top_radius: f32])]
#[pyclass(name = "Atmosphere", extends = PyComponent)]
#[derive(Clone)]
pub struct PyAtmosphere {
    pub(crate) storage: ComponentStorage<Atmosphere>,
}

#[pymethods]
impl PyAtmosphere {
    #[new]
    #[pyo3(signature = (medium, bottom_radius, top_radius, ground_albedo))]
    pub fn new(
        medium: &Bound<'_, PyAny>,
        bottom_radius: f32,
        top_radius: f32,
        ground_albedo: PyVec3,
    ) -> PyResult<(Self, PyComponent)> {
        let handle = extract_handle_from_any(medium)?;
        Ok(Self::from_owned(Atmosphere {
            bottom_radius,
            top_radius,
            ground_albedo: ground_albedo.into(),
            medium: handle.try_into()?,
        }))
    }
    #[staticmethod]
    #[pyo3(signature = (medium))]
    pub fn earthlike(py: Python<'_>, medium: &Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        let handle = extract_handle_from_any(medium)?;
        Py::new(
            py,
            Self::from_owned(Atmosphere::earthlike(handle.try_into()?)),
        )
    }

    #[getter]
    pub fn bottom_radius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.bottom_radius)
    }

    #[setter]
    pub fn set_bottom_radius(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.bottom_radius = value;
        Ok(())
    }

    #[getter]
    pub fn top_radius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.top_radius)
    }

    #[setter]
    pub fn set_top_radius(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.top_radius = value;
        Ok(())
    }

    #[getter]
    pub fn ground_albedo(&self) -> PyResult<PyVec3> {
        Ok(self.storage.borrow_field_as(|a| &a.ground_albedo)?)
    }

    #[setter]
    pub fn set_ground_albedo(&mut self, value: PyVec3) -> PyResult<()> {
        self.as_mut()?.ground_albedo = value.into();
        Ok(())
    }

    #[getter]
    pub fn medium(&self) -> PyResult<PyHandle> {
        Ok(self.as_ref()?.medium.clone().into())
    }

    #[setter]
    pub fn set_medium(&mut self, medium: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.medium = extract_handle_from_any(medium)?.try_into()?;
        Ok(())
    }
}
