use std::f32::consts::FRAC_PI_3;

use bevy::shape::Arc2d;
use pybevy_core::{
    FromBorrowedStorage, ValueStorage,
    domain::{DomainViolation, FINITE, check_for},
    repr::float_repr,
};
use pybevy_macros::pyvalue;
use pyo3::{PyClass, prelude::*};

use super::dimension::{Dimension, dimension};
use crate::vec2::PyVec2;

pub fn checked_arc<O: PyClass>(radius: f32, half_angle: f32) -> Result<Arc2d, DomainViolation> {
    Ok(Arc2d::new(
        dimension::<O, _>("radius", radius)?,
        check_for::<O, _>("half_angle", half_angle, FINITE)?,
    ))
}

pub fn checked_arc_from_radians<O: PyClass>(
    radius: f32,
    radians: f32,
) -> Result<Arc2d, DomainViolation> {
    Ok(Arc2d::from_radians(
        dimension::<O, _>("radius", radius)?,
        check_for::<O, _>("angle", radians, FINITE)?,
    ))
}

pub fn checked_arc_from_degrees<O: PyClass>(
    radius: f32,
    degrees: f32,
) -> Result<Arc2d, DomainViolation> {
    Ok(Arc2d::from_degrees(
        dimension::<O, _>("radius", radius)?,
        check_for::<O, _>("angle", degrees, FINITE)?,
    ))
}

pub fn checked_arc_from_turns<O: PyClass>(
    radius: f32,
    fraction: f32,
) -> Result<Arc2d, DomainViolation> {
    let arc = Arc2d::from_turns(
        dimension::<O, _>("radius", radius)?,
        check_for::<O, _>("fraction", fraction, FINITE)?,
    );
    check_for::<O, _>("half_angle", arc.half_angle, FINITE)?;
    Ok(arc)
}

#[pyvalue(domains = [radius => [finite, non_negative], half_angle => [finite]])]
#[pyclass(name = "Arc2d", module = "pybevy.shape", eq, from_py_object)]
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

#[pymethods]
impl PyArc2d {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(Arc2d::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    #[pyo3(signature = (radius = 0.5, half_angle = 2.0 * FRAC_PI_3))]
    pub fn new(radius: f32, half_angle: f32) -> PyResult<Self> {
        let radius = Self::check_radius(radius)?.without_negative_zero();
        let half_angle = Self::check_half_angle(half_angle)?;
        Ok(Self::from_owned(Arc2d::new(radius, half_angle)))
    }

    #[staticmethod]
    pub fn from_radians(radius: f32, angle: f32) -> PyResult<Self> {
        Ok(Self {
            storage: ValueStorage::owned(checked_arc_from_radians::<Self>(radius, angle)?),
        })
    }

    #[staticmethod]
    pub fn from_degrees(radius: f32, angle: f32) -> PyResult<Self> {
        Ok(Self {
            storage: ValueStorage::owned(checked_arc_from_degrees::<Self>(radius, angle)?),
        })
    }

    #[staticmethod]
    pub fn from_turns(radius: f32, fraction: f32) -> PyResult<Self> {
        Ok(Self {
            storage: ValueStorage::owned(checked_arc_from_turns::<Self>(radius, fraction)?),
        })
    }

    #[getter]
    pub fn radius(&self) -> PyResult<f32> {
        Ok(self.storage.as_ref()?.radius)
    }

    #[setter]
    pub fn set_radius(&mut self, value: f32) -> PyResult<()> {
        self.storage.as_mut()?.radius = Self::check_radius(value)?.without_negative_zero();
        Ok(())
    }

    #[getter]
    pub fn half_angle(&self) -> PyResult<f32> {
        Ok(self.storage.as_ref()?.half_angle)
    }

    #[setter]
    pub fn set_half_angle(&mut self, value: f32) -> PyResult<()> {
        self.storage.as_mut()?.half_angle = Self::check_half_angle(value)?;
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
            float_repr(arc.radius),
            float_repr(arc.half_angle)
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
