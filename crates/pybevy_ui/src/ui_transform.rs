use bevy::ui::UiTransform;
use pybevy_core::{
    ComponentStorage, PyComponent,
    repr::{ReprLiteral, object_repr},
};
use pybevy_macros::pycomponent;
use pybevy_math::{rot2::PyRot2, vec2::PyVec2};
use pyo3::prelude::*;

use crate::val2::PyVal2;

#[pycomponent(UiTransform, bridge)]
#[pyclass(name = "UiTransform", module = "pybevy.ui", extends = PyComponent, eq)]
#[derive(Debug, PartialEq)]
pub struct PyUiTransform {
    pub(crate) storage: ComponentStorage<UiTransform>,
}

#[pymethods]
impl PyUiTransform {
    #[new]
    #[pyo3(signature = (
        *,
        translation = PyVal2::from(UiTransform::IDENTITY.translation),
        scale = PyVec2::from(UiTransform::IDENTITY.scale),
        rotation = PyRot2::from(UiTransform::IDENTITY.rotation)
    ))]
    pub fn new(
        translation: PyVal2,
        scale: PyVec2,
        rotation: PyRot2,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(UiTransform {
            translation: translation.try_into()?,
            scale: scale.try_into()?,
            rotation: rotation.try_into()?,
        })
        .into())
    }

    #[staticmethod]
    pub fn identity(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(UiTransform::IDENTITY))
    }

    #[staticmethod]
    pub fn from_translation(py: Python, translation: PyVal2) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(UiTransform::from_translation(translation.try_into()?)),
        )
    }

    #[staticmethod]
    pub fn from_rotation(py: Python, rotation: PyRot2) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(UiTransform::from_rotation(rotation.try_into()?)),
        )
    }

    #[staticmethod]
    pub fn from_scale(py: Python, scale: PyVec2) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(UiTransform::from_scale(scale.try_into()?)),
        )
    }

    #[getter]
    pub fn translation(&self) -> PyResult<PyVal2> {
        Ok(self
            .storage
            .borrow_field_as(|transform| &transform.translation)?)
    }

    #[setter]
    pub fn set_translation(&mut self, value: PyVal2) -> PyResult<()> {
        self.as_mut()?.translation = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn scale(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|s| &s.scale)?)
    }

    #[setter]
    pub fn set_scale(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.scale = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn rotation(&self) -> PyResult<PyRot2> {
        Ok(self
            .storage
            .borrow_field_as(|transform| &transform.rotation)?)
    }

    #[setter]
    pub fn set_rotation(&mut self, value: PyRot2) -> PyResult<()> {
        self.as_mut()?.rotation = value.try_into()?;
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        let t = self.as_ref()?;
        Ok(format!(
            "UiTransform(translation={}, scale={}, rotation={})",
            object_repr(PyVal2::from(t.translation)),
            t.scale.repr_literal(),
            object_repr(PyRot2::from(t.rotation))
        ))
    }
}
