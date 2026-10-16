use bevy::math::{Dir3, Isometry3d, Vec3, Vec3A};
use pybevy_core::{FromBorrowedStorage, Omittable, ValueStorage, repr::ReprLiteral};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

use crate::{
    check_direction_length,
    dir3::PyDir3,
    quat::PyQuat,
    vec3::PyVec3,
    vec3a::{PyVec3A, extract_vec3a_from_any},
};

#[pyvalue(bevy::math::Isometry3d)]
#[pyclass(name = "Isometry3d", module = "pybevy.math", eq, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyIsometry3d {
    pub(crate) storage: ValueStorage<Isometry3d>,
}

impl PartialEq for PyIsometry3d {
    fn eq(&self, other: &Self) -> bool {
        match (self.to_bevy(), other.to_bevy()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl TryFrom<PyIsometry3d> for Isometry3d {
    type Error = PyErr;

    fn try_from(iso: PyIsometry3d) -> PyResult<Self> {
        Ok(iso.storage.get()?)
    }
}

impl TryFrom<&PyIsometry3d> for Isometry3d {
    type Error = PyErr;

    fn try_from(iso: &PyIsometry3d) -> PyResult<Self> {
        Ok(iso.storage.get()?)
    }
}

impl From<Isometry3d> for PyIsometry3d {
    fn from(iso: Isometry3d) -> Self {
        PyIsometry3d::from_owned(iso)
    }
}

#[pymethods]
impl PyIsometry3d {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(Isometry3d::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    #[pyo3(signature = (translation = Omittable::default(), rotation = PyQuat::IDENTITY))]
    pub fn new(translation: Omittable<Bound<'_, PyAny>>, rotation: PyQuat) -> PyResult<Self> {
        let t = match translation.into_option() {
            Some(obj) => extract_vec3a_from_any(&obj)?,
            None => Vec3A::ZERO,
        };
        Ok(PyIsometry3d::from_owned(Isometry3d::new(
            t,
            rotation.try_into()?,
        )))
    }

    #[staticmethod]
    #[pyo3(name = "IDENTITY")]
    fn identity() -> Self {
        PyIsometry3d::from_owned(Isometry3d::IDENTITY)
    }

    #[staticmethod]
    pub fn from_rotation(rotation: PyQuat) -> PyResult<Self> {
        Ok(PyIsometry3d::from_owned(Isometry3d::from_rotation(
            rotation.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_translation(translation: &Bound<'_, PyAny>) -> PyResult<Self> {
        let t = extract_vec3a_from_any(translation)?;
        Ok(PyIsometry3d::from_owned(Isometry3d::from_translation(t)))
    }

    #[staticmethod]
    pub fn from_xyz(x: f32, y: f32, z: f32) -> Self {
        PyIsometry3d::from_owned(Isometry3d::from_xyz(x, y, z))
    }

    #[getter]
    pub fn translation(&self) -> PyResult<PyVec3A> {
        Ok(self.storage.borrow_field_as(|i| &i.translation)?)
    }

    #[setter]
    pub fn set_translation(&mut self, value: PyVec3A) -> PyResult<()> {
        self.storage.as_mut()?.translation = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn rotation(&self) -> PyResult<PyQuat> {
        Ok(self.storage.borrow_field_as(|i| &i.rotation)?)
    }

    #[setter]
    pub fn set_rotation(&mut self, value: PyQuat) -> PyResult<()> {
        self.storage.as_mut()?.rotation = value.try_into()?;
        Ok(())
    }

    pub fn inverse(&self) -> PyResult<Self> {
        Ok(PyIsometry3d::from_owned(self.to_bevy()?.inverse()))
    }

    pub fn inverse_mul(&self, rhs: &PyIsometry3d) -> PyResult<Self> {
        Ok(PyIsometry3d::from_owned(
            self.to_bevy()?.inverse_mul(rhs.to_bevy()?),
        ))
    }

    pub fn transform_point(&self, point: &Bound<'_, PyAny>) -> PyResult<PyVec3> {
        let p = extract_vec3a_from_any(point)?;
        let result: Vec3 = self.to_bevy()?.transform_point(p).into();
        Ok(result.into())
    }

    pub fn inverse_transform_point(&self, point: &Bound<'_, PyAny>) -> PyResult<PyVec3> {
        let p = extract_vec3a_from_any(point)?;
        let result: Vec3 = self.to_bevy()?.inverse_transform_point(p).into();
        Ok(result.into())
    }

    pub fn __mul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let iso: Isometry3d = self.to_bevy()?;
        if let Ok(other_iso) = other.extract::<PyIsometry3d>() {
            let composed: Isometry3d = iso * other_iso.to_bevy()?;
            Ok(Py::new(py, PyIsometry3d::from_owned(composed))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec3A>() {
            let transformed: Vec3A = iso * Vec3A::try_from(&other_vec)?;
            Ok(Py::new(py, PyVec3A::from_vec3a(transformed))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec3>() {
            let transformed: Vec3 = iso * Vec3::try_from(&other_vec)?;
            Ok(Py::new(py, PyVec3::from_vec3(transformed))?.into_any())
        } else if let Ok(other_dir) = other.extract::<PyDir3>() {
            let rotated = iso.rotation * Vec3::from(Dir3::try_from(&other_dir)?);
            check_direction_length("Isometry3d * Dir3", rotated.length_squared())?;
            Ok(Py::new(py, PyDir3::from_dir3(Dir3::new_unchecked(rotated)))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __repr__(&self) -> PyResult<String> {
        let iso = self.to_bevy()?;
        let t = iso.translation;
        let r = iso.rotation;
        Ok(format!(
            "Isometry3d(translation={}, rotation={})",
            t.repr_literal(),
            r.repr_literal()
        ))
    }
}
