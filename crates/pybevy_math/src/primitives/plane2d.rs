use bevy::math::{Dir2, primitives::Plane2d};
use pybevy_core::{
    FromBorrowedStorage, ValueStorage, public_error::UNSUPPORTED_COMPARISON, repr::float_list_repr,
};
use pybevy_macros::pyvalue;
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use crate::{dir2::PyDir2, richcmp::comparison_result, vec2::PyVec2};

#[pyvalue]
#[pyclass(name = "Plane2d", module = "pybevy.math", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyPlane2d {
    pub(crate) storage: ValueStorage<Plane2d>,
}

#[pymethods]
impl PyPlane2d {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(Plane2d::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    #[pyo3(signature = (normal = PyVec2::Y))]
    pub fn new(normal: PyVec2) -> PyResult<Self> {
        let dir =
            Dir2::new(normal.try_get()?).map_err(|e| PyValueError::new_err(format!("{}", e)))?;
        Ok(Self::from_owned(Plane2d { normal: dir }))
    }

    #[staticmethod]
    pub fn from_dir(normal: PyDir2) -> PyResult<Self> {
        Ok(Self::from_owned(Plane2d {
            normal: normal.into_dir2()?,
        }))
    }

    #[getter]
    pub fn normal(&self) -> PyResult<PyDir2> {
        Ok(self.storage.borrow_field_as(|plane| &plane.normal)?)
    }

    #[setter]
    pub fn set_normal(&mut self, normal: PyDir2) -> PyResult<()> {
        self.as_mut()?.normal = normal.into_dir2()?;
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        let n = self.as_ref()?.normal;
        Ok(format!(
            "Plane2d(normal={})",
            float_list_repr(&n.to_array())
        ))
    }

    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other_value) = other.extract::<PyRef<'_, PyPlane2d>>() else {
            return Ok(py.NotImplemented());
        };
        let result = match op {
            CompareOp::Eq => *self.as_ref()? == *other_value.as_ref()?,
            CompareOp::Ne => *self.as_ref()? != *other_value.as_ref()?,
            _ => return Err(PyTypeError::new_err(UNSUPPORTED_COMPARISON)),
        };
        Ok(comparison_result(py, result))
    }
}

impl From<Plane2d> for PyPlane2d {
    fn from(plane: Plane2d) -> Self {
        Self::from_owned(plane)
    }
}

impl TryFrom<PyPlane2d> for Plane2d {
    type Error = PyErr;

    fn try_from(plane: PyPlane2d) -> PyResult<Self> {
        plane.to_bevy()
    }
}

impl TryFrom<&PyPlane2d> for Plane2d {
    type Error = PyErr;

    fn try_from(plane: &PyPlane2d) -> PyResult<Self> {
        plane.to_bevy()
    }
}
