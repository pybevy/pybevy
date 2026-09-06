use bevy::ui::UiTransform;
use pybevy_core::{ComponentStorage, PyComponent};
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
    #[pyo3(signature = (translation = None, scale = None, rotation = None))]
    pub fn new(
        translation: Option<PyVal2>,
        scale: Option<PyVec2>,
        rotation: Option<PyRot2>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let mut transform = UiTransform::IDENTITY;
        if let Some(t) = translation {
            transform.translation = t.into();
        }
        if let Some(s) = scale {
            transform.scale = s.try_into()?;
        }
        if let Some(r) = rotation {
            transform.rotation = r.try_into()?;
        }
        Ok(Self::from_owned(transform).into())
    }

    #[staticmethod]
    pub fn identity(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(UiTransform::IDENTITY))
    }

    #[staticmethod]
    pub fn from_translation(py: Python, translation: PyVal2) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(UiTransform::from_translation(translation.into())),
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
        Ok(self.as_ref()?.translation.into())
    }

    #[setter]
    pub fn set_translation(&mut self, value: PyVal2) -> PyResult<()> {
        self.as_mut()?.translation = value.into();
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
            "UiTransform(translation={:?}, scale={:?}, rotation={:?})",
            t.translation, t.scale, t.rotation
        ))
    }
}
