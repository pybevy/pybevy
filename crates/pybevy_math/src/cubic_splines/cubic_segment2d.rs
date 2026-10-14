use bevy::{
    curve::{Curve, cubic_splines::CubicSegment},
    math::Vec2,
};
use pybevy_core::{
    FromBorrowedStorage, ValueStorage, public_error::UNSUPPORTED_COMPARISON, repr::ReprLiteral,
};
use pybevy_macros::pyvalue;
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use crate::{interval::PyInterval, richcmp::comparison_result, vec2::PyVec2};

#[pyvalue]
#[pyclass(name = "CubicSegment2d", module = "pybevy.curve", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyCubicSegment2d {
    storage: ValueStorage<CubicSegment<Vec2>>,
}

impl From<CubicSegment<Vec2>> for PyCubicSegment2d {
    fn from(segment: CubicSegment<Vec2>) -> Self {
        Self::from_owned(segment)
    }
}

impl TryFrom<&PyCubicSegment2d> for CubicSegment<Vec2> {
    type Error = PyErr;

    fn try_from(segment: &PyCubicSegment2d) -> PyResult<Self> {
        segment.to_bevy()
    }
}

#[pymethods]
impl PyCubicSegment2d {
    #[new]
    #[pyo3(signature = (*, coeff = [PyVec2::ZERO, PyVec2::ZERO, PyVec2::ZERO, PyVec2::ZERO]))]
    pub fn new(coeff: [PyVec2; 4]) -> PyResult<Self> {
        let [a, b, c, d] = coeff;
        Ok(Self::from_owned(CubicSegment {
            coeff: [a.try_into()?, b.try_into()?, c.try_into()?, d.try_into()?],
        }))
    }

    #[staticmethod]
    pub fn new_bezier(points: [PyVec2; 4]) -> PyResult<Self> {
        let [a, b, c, d] = points;
        Ok(Self::from_owned(CubicSegment::new_bezier([
            a.try_into()?,
            b.try_into()?,
            c.try_into()?,
            d.try_into()?,
        ])))
    }

    #[getter]
    pub fn coeff(&self) -> PyResult<Vec<PyVec2>> {
        Ok(self
            .to_bevy()?
            .coeff
            .into_iter()
            .map(|value| PyVec2::from_borrowed(ValueStorage::read_only_snapshot(value)))
            .collect())
    }

    #[setter]
    pub fn set_coeff(&mut self, coeff: [PyVec2; 4]) -> PyResult<()> {
        let [a, b, c, d] = coeff;
        self.storage.as_mut()?.coeff = [a.try_into()?, b.try_into()?, c.try_into()?, d.try_into()?];
        Ok(())
    }

    pub fn position(&self, t: f32) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.to_bevy()?.position(t)))
    }

    pub fn velocity(&self, t: f32) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.to_bevy()?.velocity(t)))
    }

    pub fn acceleration(&self, t: f32) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.to_bevy()?.acceleration(t)))
    }

    pub fn domain(&self) -> PyResult<PyInterval> {
        Ok(self.to_bevy()?.domain().into())
    }

    pub fn sample(&self, t: f32) -> PyResult<Option<PyVec2>> {
        Ok(self.to_bevy()?.sample(t).map(PyVec2::from_vec2))
    }

    pub fn sample_clamped(&self, t: f32) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.to_bevy()?.sample_clamped(t)))
    }

    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(self.to_bevy()?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    fn __repr__(&self) -> PyResult<String> {
        let coeff = self
            .to_bevy()?
            .coeff
            .map(|value| value.repr_literal())
            .join(", ");
        Ok(format!("CubicSegment2d(coeff=[{coeff}])"))
    }

    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other) = other.cast::<PyCubicSegment2d>() else {
            return Ok(py.NotImplemented());
        };
        let equal = self.to_bevy()? == other.try_borrow()?.to_bevy()?;
        let result = match op {
            CompareOp::Eq => equal,
            CompareOp::Ne => !equal,
            _ => return Err(PyTypeError::new_err(UNSUPPORTED_COMPARISON)),
        };
        Ok(comparison_result(py, result))
    }
}
