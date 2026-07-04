use bevy::light::Atmosphere;
use pybevy_core::{ComponentStorage, PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::pycomponent;
use pybevy_math::vec3::PyVec3;
use pyo3::prelude::*;

#[pycomponent(Atmosphere, bridge, view_only_fields = [inner_radius: f32, outer_radius: f32])]
#[pyclass(name = "Atmosphere", extends = PyComponent)]
pub struct PyAtmosphere {
    pub(crate) storage: ComponentStorage<Atmosphere>,
}

#[pymethods]
impl PyAtmosphere {
    #[new]
    #[pyo3(signature = (inner_radius, outer_radius, ground_albedo, medium))]
    pub fn new(
        inner_radius: f32,
        outer_radius: f32,
        ground_albedo: PyVec3,
        medium: &Bound<'_, PyAny>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let handle = extract_handle_from_any(medium)?;
        Ok(Self::from_owned(Atmosphere {
            inner_radius,
            outer_radius,
            ground_albedo: ground_albedo.into(),
            medium: handle.try_into()?,
        })
        .into())
    }
    #[staticmethod]
    #[pyo3(signature = (medium))]
    pub fn earth(py: Python<'_>, medium: &Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        let handle = extract_handle_from_any(medium)?;
        Py::new(py, Self::from_owned(Atmosphere::earth(handle.try_into()?)))
    }

    #[staticmethod]
    #[pyo3(signature = (medium))]
    pub fn mars(py: Python<'_>, medium: &Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        let handle = extract_handle_from_any(medium)?;
        Py::new(py, Self::from_owned(Atmosphere::mars(handle.try_into()?)))
    }

    #[getter]
    pub fn inner_radius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.inner_radius)
    }

    #[setter]
    pub fn set_inner_radius(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.inner_radius = value;
        Ok(())
    }

    #[getter]
    pub fn outer_radius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.outer_radius)
    }

    #[setter]
    pub fn set_outer_radius(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.outer_radius = value;
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
