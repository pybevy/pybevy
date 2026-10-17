use bevy::curve::{Interval, interval};
use pybevy_core::{
    FromBorrowedStorage, ValueStorage,
    domain::{FieldDomain, IntegerInput, check_for, check_ordered_for},
    public_error::INVALID_INTERVAL,
    repr::float_repr,
};
use pybevy_macros::pyvalue;
use pybevy_math::richcmp::comparison_result;
use pyo3::{basic::CompareOp, exceptions::PyValueError, prelude::*};

#[pyvalue]
#[pyclass(name = "Interval", module = "pybevy.curve", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyInterval {
    pub(crate) storage: ValueStorage<Interval>,
}

impl TryFrom<PyInterval> for Interval {
    type Error = PyErr;

    fn try_from(py_interval: PyInterval) -> PyResult<Self> {
        Ok(py_interval.storage.get()?)
    }
}

impl TryFrom<&PyInterval> for Interval {
    type Error = PyErr;

    fn try_from(py_interval: &PyInterval) -> PyResult<Self> {
        Ok(py_interval.storage.get()?)
    }
}

impl From<Interval> for PyInterval {
    fn from(interval: Interval) -> Self {
        PyInterval::from_owned(interval)
    }
}

const SPACED_POINTS_DOMAIN: &[FieldDomain] = &[FieldDomain::RangeInclusive {
    lo: 0.0,
    hi: 1_000_000.0,
}];

fn invalid_interval_error() -> PyErr {
    PyValueError::new_err(INVALID_INTERVAL)
}

#[pymethods]
impl PyInterval {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(Interval::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    pub fn new(start: f32, end: f32) -> PyResult<PyInterval> {
        check_ordered_for::<Self, _>("start", start, "end", end, true)?;
        interval(start, end)
            .map(Into::into)
            .map_err(|_| invalid_interval_error())
    }

    #[staticmethod]
    #[pyo3(name = "UNIT")]
    pub fn unit() -> PyInterval {
        Interval::UNIT.into()
    }

    #[staticmethod]
    #[pyo3(name = "EVERYWHERE")]
    pub fn everywhere() -> PyInterval {
        Interval::EVERYWHERE.into()
    }

    #[getter]
    pub fn start(&self) -> PyResult<f32> {
        Ok(self.to_bevy()?.start())
    }

    #[getter]
    pub fn end(&self) -> PyResult<f32> {
        Ok(self.to_bevy()?.end())
    }

    pub fn intersect(&self, other: &PyInterval) -> PyResult<PyInterval> {
        self.to_bevy()?
            .intersect(other.to_bevy()?)
            .map(Into::into)
            .map_err(|_| invalid_interval_error())
    }

    pub fn length(&self) -> PyResult<f32> {
        Ok(self.to_bevy()?.length())
    }

    pub fn is_bounded(&self) -> PyResult<bool> {
        Ok(self.to_bevy()?.is_bounded())
    }

    pub fn has_finite_start(&self) -> PyResult<bool> {
        Ok(self.to_bevy()?.has_finite_start())
    }

    pub fn has_finite_end(&self) -> PyResult<bool> {
        Ok(self.to_bevy()?.has_finite_end())
    }

    pub fn contains(&self, item: f32) -> PyResult<bool> {
        Ok(self.to_bevy()?.contains(item))
    }

    pub fn contains_interval(&self, other: &PyInterval) -> PyResult<bool> {
        Ok(self.to_bevy()?.contains_interval(other.to_bevy()?))
    }

    pub fn clamp(&self, value: f32) -> PyResult<f32> {
        Ok(self.to_bevy()?.clamp(value))
    }

    pub fn spaced_points(&self, points: IntegerInput<u64>) -> PyResult<Vec<f32>> {
        let points = check_for::<Self, _>("points", points, SPACED_POINTS_DOMAIN)?.stored();
        self.to_bevy()?
            .spaced_points(points as usize)
            .map(|points| points.collect())
            .map_err(|e| PyValueError::new_err(format!("{e}")))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let interval = self.to_bevy()?;
        Ok(format!(
            "Interval(start={}, end={})",
            float_repr(interval.start()),
            float_repr(interval.end())
        ))
    }

    pub fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Some(other_interval) = other.extract::<PyInterval>().ok() else {
            return Ok(py.NotImplemented());
        };
        let a = self.to_bevy()?;
        let b = other_interval.to_bevy()?;
        let result = match op {
            CompareOp::Eq => a == b,
            CompareOp::Ne => a != b,
            CompareOp::Lt => a < b,
            CompareOp::Le => a <= b,
            CompareOp::Gt => a > b,
            CompareOp::Ge => a >= b,
        };
        Ok(comparison_result(py, result))
    }
}
