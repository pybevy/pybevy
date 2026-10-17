use bevy::{
    curve::{Curve, cubic_splines::RationalSegment},
    math::Vec2,
};
use pybevy_core::{
    FromBorrowedStorage, ValueStorage,
    public_error::UNSUPPORTED_COMPARISON,
    repr::{ReprLiteral, float_repr},
};
use pybevy_macros::pyvalue;
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use crate::{interval::PyInterval, richcmp::comparison_result, vec2::PyVec2};

#[pyvalue]
#[pyclass(
    name = "RationalSegment2d",
    module = "pybevy.curve",
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyRationalSegment2d {
    storage: ValueStorage<RationalSegment<Vec2>>,
}

impl From<RationalSegment<Vec2>> for PyRationalSegment2d {
    fn from(segment: RationalSegment<Vec2>) -> Self {
        Self::from_owned(segment)
    }
}

impl TryFrom<&PyRationalSegment2d> for RationalSegment<Vec2> {
    type Error = PyErr;

    fn try_from(segment: &PyRationalSegment2d) -> PyResult<Self> {
        segment.to_bevy()
    }
}

#[pymethods]
impl PyRationalSegment2d {
    #[new]
    #[pyo3(signature = (
        *,
        coeff = [PyVec2::ZERO, PyVec2::ZERO, PyVec2::ZERO, PyVec2::ZERO],
        weight_coeff = [0.0; 4],
        knot_span = 0.0
    ))]
    pub fn new(coeff: [PyVec2; 4], weight_coeff: [f32; 4], knot_span: f32) -> PyResult<Self> {
        let [a, b, c, d] = coeff;
        Ok(Self::from_owned(RationalSegment {
            coeff: [a.try_into()?, b.try_into()?, c.try_into()?, d.try_into()?],
            weight_coeff,
            knot_span,
        }))
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

    #[getter]
    pub fn weight_coeff(&self) -> PyResult<[f32; 4]> {
        Ok(self.to_bevy()?.weight_coeff)
    }

    #[setter]
    pub fn set_weight_coeff(&mut self, weight_coeff: [f32; 4]) -> PyResult<()> {
        self.storage.as_mut()?.weight_coeff = weight_coeff;
        Ok(())
    }

    #[getter]
    pub fn knot_span(&self) -> PyResult<f32> {
        Ok(self.to_bevy()?.knot_span)
    }

    #[setter]
    pub fn set_knot_span(&mut self, knot_span: f32) -> PyResult<()> {
        self.storage.as_mut()?.knot_span = knot_span;
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
        let segment = self.to_bevy()?;
        let coeff = segment.coeff.map(|value| value.repr_literal()).join(", ");
        let weights = segment.weight_coeff.map(float_repr).join(", ");
        Ok(format!(
            "RationalSegment2d(coeff=[{coeff}], weight_coeff=[{weights}], knot_span={})",
            float_repr(segment.knot_span)
        ))
    }

    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other) = other.cast::<PyRationalSegment2d>() else {
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
