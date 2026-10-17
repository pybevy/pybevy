use bevy::{
    curve::{Curve, cubic_splines::RationalSegment},
    math::Vec3,
};
use pybevy_core::{
    FromBorrowedStorage, ValueStorage,
    public_error::UNSUPPORTED_COMPARISON,
    repr::{ReprLiteral, float_repr},
};
use pybevy_macros::pyvalue;
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use crate::{interval::PyInterval, richcmp::comparison_result, vec3::PyVec3};

#[pyvalue]
#[pyclass(
    name = "RationalSegment3d",
    module = "pybevy.curve",
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyRationalSegment3d {
    storage: ValueStorage<RationalSegment<Vec3>>,
}

impl From<RationalSegment<Vec3>> for PyRationalSegment3d {
    fn from(segment: RationalSegment<Vec3>) -> Self {
        Self::from_owned(segment)
    }
}

impl TryFrom<&PyRationalSegment3d> for RationalSegment<Vec3> {
    type Error = PyErr;

    fn try_from(segment: &PyRationalSegment3d) -> PyResult<Self> {
        segment.to_bevy()
    }
}

#[pymethods]
impl PyRationalSegment3d {
    #[new]
    #[pyo3(signature = (
        *,
        coeff = [PyVec3::ZERO, PyVec3::ZERO, PyVec3::ZERO, PyVec3::ZERO],
        weight_coeff = [0.0; 4],
        knot_span = 0.0
    ))]
    pub fn new(coeff: [PyVec3; 4], weight_coeff: [f32; 4], knot_span: f32) -> PyResult<Self> {
        let [a, b, c, d] = coeff;
        Ok(Self::from_owned(RationalSegment {
            coeff: [a.try_into()?, b.try_into()?, c.try_into()?, d.try_into()?],
            weight_coeff,
            knot_span,
        }))
    }

    #[getter]
    pub fn coeff(&self) -> PyResult<Vec<PyVec3>> {
        Ok(self
            .to_bevy()?
            .coeff
            .into_iter()
            .map(|value| PyVec3::from_borrowed(ValueStorage::read_only_snapshot(value)))
            .collect())
    }

    #[setter]
    pub fn set_coeff(&mut self, coeff: [PyVec3; 4]) -> PyResult<()> {
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

    pub fn position(&self, t: f32) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.to_bevy()?.position(t)))
    }

    pub fn velocity(&self, t: f32) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.to_bevy()?.velocity(t)))
    }

    pub fn acceleration(&self, t: f32) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.to_bevy()?.acceleration(t)))
    }

    pub fn domain(&self) -> PyResult<PyInterval> {
        Ok(self.to_bevy()?.domain().into())
    }

    pub fn sample(&self, t: f32) -> PyResult<Option<PyVec3>> {
        Ok(self.to_bevy()?.sample(t).map(PyVec3::from_vec3))
    }

    pub fn sample_clamped(&self, t: f32) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.to_bevy()?.sample_clamped(t)))
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
            "RationalSegment3d(coeff=[{coeff}], weight_coeff=[{weights}], knot_span={})",
            float_repr(segment.knot_span)
        ))
    }

    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other) = other.cast::<PyRationalSegment3d>() else {
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
