use std::f32::consts::FRAC_PI_3;

use bevy::math::primitives::Arc2d;
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pyo3::prelude::*;

use crate::vec2::PyVec2;

#[pyclass(name = "Arc2d", module = "pybevy.math", eq, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyArc2d {
    pub(crate) storage: ValueStorage<Arc2d>,
}

impl PyArc2d {
    pub fn from_read_only(arc: &Arc2d) -> Self {
        Self {
            storage: ValueStorage::read_only_snapshot(*arc),
        }
    }
}

impl FromBorrowedStorage<ValueStorage<Arc2d>> for PyArc2d {
    fn from_borrowed(storage: ValueStorage<Arc2d>) -> Self {
        Self { storage }
    }
}

#[pymethods]
impl PyArc2d {
    #[new]
    #[pyo3(signature = (radius = 0.5, half_angle = 2.0 * FRAC_PI_3))]
    pub fn new(radius: f32, half_angle: f32) -> Self {
        Self {
            storage: ValueStorage::owned(Arc2d::new(radius, half_angle)),
        }
    }

    #[staticmethod]
    pub fn from_radians(radius: f32, angle: f32) -> Self {
        Self {
            storage: ValueStorage::owned(Arc2d::from_radians(radius, angle)),
        }
    }

    #[staticmethod]
    pub fn from_degrees(radius: f32, angle: f32) -> Self {
        Self {
            storage: ValueStorage::owned(Arc2d::from_degrees(radius, angle)),
        }
    }

    #[staticmethod]
    pub fn from_turns(radius: f32, fraction: f32) -> Self {
        Self {
            storage: ValueStorage::owned(Arc2d::from_turns(radius, fraction)),
        }
    }

    #[getter]
    pub fn radius(&self) -> PyResult<f32> {
        Ok(self.storage.as_ref()?.radius)
    }

    #[setter]
    pub fn set_radius(&mut self, value: f32) -> PyResult<()> {
        self.storage.as_mut()?.radius = value;
        Ok(())
    }

    #[getter]
    pub fn half_angle(&self) -> PyResult<f32> {
        Ok(self.storage.as_ref()?.half_angle)
    }

    #[setter]
    pub fn set_half_angle(&mut self, value: f32) -> PyResult<()> {
        self.storage.as_mut()?.half_angle = value;
        Ok(())
    }

    pub fn angle(&self) -> PyResult<f32> {
        Ok(self.storage.as_ref()?.angle())
    }

    pub fn length(&self) -> PyResult<f32> {
        Ok(self.storage.as_ref()?.length())
    }

    pub fn right_endpoint(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.storage.as_ref()?.right_endpoint()))
    }

    pub fn left_endpoint(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.storage.as_ref()?.left_endpoint()))
    }

    pub fn endpoints(&self) -> PyResult<[PyVec2; 2]> {
        let [left, right] = self.storage.as_ref()?.endpoints();
        Ok([PyVec2::from_vec2(left), PyVec2::from_vec2(right)])
    }

    pub fn midpoint(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.storage.as_ref()?.midpoint()))
    }

    pub fn half_chord_length(&self) -> PyResult<f32> {
        Ok(self.storage.as_ref()?.half_chord_length())
    }

    pub fn chord_length(&self) -> PyResult<f32> {
        Ok(self.storage.as_ref()?.chord_length())
    }

    pub fn chord_midpoint(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.storage.as_ref()?.chord_midpoint()))
    }

    pub fn apothem(&self) -> PyResult<f32> {
        Ok(self.storage.as_ref()?.apothem())
    }

    pub fn sagitta(&self) -> PyResult<f32> {
        Ok(self.storage.as_ref()?.sagitta())
    }

    pub fn is_minor(&self) -> PyResult<bool> {
        Ok(self.storage.as_ref()?.is_minor())
    }

    pub fn is_major(&self) -> PyResult<bool> {
        Ok(self.storage.as_ref()?.is_major())
    }

    fn __repr__(&self) -> PyResult<String> {
        let arc = self.storage.as_ref()?;
        Ok(format!(
            "Arc2d(radius={}, half_angle={})",
            arc.radius, arc.half_angle
        ))
    }
}

impl From<Arc2d> for PyArc2d {
    fn from(arc: Arc2d) -> Self {
        Self {
            storage: ValueStorage::owned(arc),
        }
    }
}

impl TryFrom<PyArc2d> for Arc2d {
    type Error = PyErr;

    fn try_from(py_arc: PyArc2d) -> PyResult<Self> {
        Ok(py_arc.storage.get()?)
    }
}

impl TryFrom<&PyArc2d> for Arc2d {
    type Error = PyErr;

    fn try_from(py_arc: &PyArc2d) -> PyResult<Self> {
        Ok(py_arc.storage.get()?)
    }
}
