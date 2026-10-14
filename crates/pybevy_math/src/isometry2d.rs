use bevy::math::{Dir2, Isometry2d, Rot2, Vec2};
use pybevy_core::{FromBorrowedStorage, ValueStorage, repr::ReprLiteral};
use pyo3::prelude::*;

use crate::{check_direction_length, dir2::PyDir2, rot2::PyRot2, vec2::PyVec2};

#[pyclass(name = "Isometry2d", module = "pybevy.math", eq, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyIsometry2d {
    inner: Isometry2d,
}

impl From<PyIsometry2d> for Isometry2d {
    fn from(iso: PyIsometry2d) -> Self {
        iso.inner
    }
}

impl From<Isometry2d> for PyIsometry2d {
    fn from(iso: Isometry2d) -> Self {
        PyIsometry2d { inner: iso }
    }
}

#[pymethods]
impl PyIsometry2d {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(self.clone())
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[staticmethod]
    #[pyo3(name = "IDENTITY")]
    fn identity() -> Self {
        Self {
            inner: Isometry2d::IDENTITY,
        }
    }

    #[new]
    #[pyo3(signature = (translation = PyVec2::ZERO, rotation = PyRot2::rot2(Rot2::IDENTITY)))]
    pub fn new(translation: PyVec2, rotation: PyRot2) -> PyResult<Self> {
        Ok(PyIsometry2d {
            inner: Isometry2d::new(translation.try_into()?, rotation.try_into()?),
        })
    }

    #[staticmethod]
    pub fn from_rotation(rotation: &PyRot2) -> PyResult<Self> {
        Ok(PyIsometry2d {
            inner: Isometry2d::from_rotation(rotation.inner()?),
        })
    }

    #[staticmethod]
    pub fn from_translation(translation: PyVec2) -> PyResult<Self> {
        Ok(PyIsometry2d {
            inner: Isometry2d::from_translation(translation.try_into()?),
        })
    }

    #[staticmethod]
    pub fn from_xy(x: f32, y: f32) -> Self {
        PyIsometry2d {
            inner: Isometry2d::from_xy(x, y),
        }
    }

    #[getter]
    pub fn rotation(&self) -> PyRot2 {
        PyRot2::from_borrowed(ValueStorage::read_only_snapshot(self.inner.rotation))
    }

    #[setter]
    pub fn set_rotation(&mut self, rotation: &PyRot2) -> PyResult<()> {
        self.inner.rotation = rotation.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn translation(&self) -> PyVec2 {
        PyVec2::from_borrowed(ValueStorage::read_only_snapshot(self.inner.translation))
    }

    #[setter]
    pub fn set_translation(&mut self, translation: &PyVec2) -> PyResult<()> {
        self.inner.translation = translation.try_into()?;
        Ok(())
    }

    pub fn inverse(&self) -> Self {
        PyIsometry2d {
            inner: self.inner.inverse(),
        }
    }

    pub fn inverse_mul(&self, rhs: &PyIsometry2d) -> Self {
        PyIsometry2d {
            inner: self.inner.inverse_mul(rhs.inner),
        }
    }

    pub fn transform_point(&self, point: PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(
            self.inner.transform_point(point.try_into()?),
        ))
    }

    pub fn inverse_transform_point(&self, point: PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(
            self.inner.inverse_transform_point(point.try_into()?),
        ))
    }

    pub fn __mul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let iso = self.inner;
        if let Ok(other_iso) = other.extract::<PyIsometry2d>() {
            Ok(Py::new(
                py,
                PyIsometry2d {
                    inner: iso * other_iso.inner,
                },
            )?
            .into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec2>() {
            let transformed: Vec2 = iso * Vec2::try_from(other_vec)?;
            Ok(Py::new(py, PyVec2::from_vec2(transformed))?.into_any())
        } else if let Ok(other_dir) = other.extract::<PyDir2>() {
            let rotated = iso.rotation * Vec2::from(other_dir.into_dir2()?);
            check_direction_length("Isometry2d * Dir2", rotated.length_squared())?;
            Ok(Py::new(py, PyDir2::from_dir2(Dir2::new_unchecked(rotated)))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Isometry2d(translation={}, rotation={})",
            self.inner.translation.repr_literal(),
            PyRot2::from(self.inner.rotation).__repr__()?
        ))
    }
}
