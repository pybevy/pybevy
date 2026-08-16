use std::ops::Range;

use bevy::camera::visibility::VisibilityRange;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::range::PyRange;
use pyo3::prelude::*;

#[pycomponent(VisibilityRange, bridge, view_fields = [use_aabb])]
#[pyclass(name = "VisibilityRange", extends = PyComponent)]
pub struct PyVisibilityRange {
    pub(crate) storage: ComponentStorage<VisibilityRange>,
}

#[pymethods]
impl PyVisibilityRange {
    #[new]
    #[pyo3(signature = (start_margin=None, end_margin=None, use_aabb=false))]
    pub fn new(
        start_margin: Option<&PyRange>,
        end_margin: Option<&PyRange>,
        use_aabb: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        let start: Range<f32> = match start_margin {
            Some(r) => r.try_into()?,
            None => 0.0..0.0,
        };
        let end: Range<f32> = match end_margin {
            Some(r) => r.try_into()?,
            None => f32::INFINITY..f32::INFINITY,
        };
        Ok(Self::from_owned(VisibilityRange {
            start_margin: start,
            end_margin: end,
            use_aabb,
        })
        .into())
    }

    #[staticmethod]
    pub fn abrupt(py: Python<'_>, start: f32, end: f32) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(VisibilityRange::abrupt(start, end)))
    }

    #[getter]
    pub fn start_margin(&self) -> PyResult<PyRange> {
        Ok(self.storage.borrow_field_as(|s| &s.start_margin)?)
    }

    #[setter]
    pub fn set_start_margin(&mut self, value: &PyRange) -> PyResult<()> {
        let range = value.try_into()?;
        self.as_mut()?.start_margin = range;
        Ok(())
    }

    #[getter]
    pub fn end_margin(&self) -> PyResult<PyRange> {
        Ok(self.storage.borrow_field_as(|s| &s.end_margin)?)
    }

    #[setter]
    pub fn set_end_margin(&mut self, value: &PyRange) -> PyResult<()> {
        let range = value.try_into()?;
        self.as_mut()?.end_margin = range;
        Ok(())
    }

    #[getter]
    pub fn use_aabb(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.use_aabb)
    }

    #[setter]
    pub fn set_use_aabb(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.use_aabb = value;
        Ok(())
    }

    pub fn is_abrupt(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_abrupt())
    }

    pub fn is_visible_at_all(&self, camera_distance: f32) -> PyResult<bool> {
        Ok(self.as_ref()?.is_visible_at_all(camera_distance))
    }

    pub fn is_culled(&self, camera_distance: f32) -> PyResult<bool> {
        Ok(self.as_ref()?.is_culled(camera_distance))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let vr = self.as_ref()?;
        Ok(format!(
            "VisibilityRange(start_margin={}..{}, end_margin={}..{}, use_aabb={})",
            vr.start_margin.start,
            vr.start_margin.end,
            vr.end_margin.start,
            vr.end_margin.end,
            vr.use_aabb
        ))
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}
