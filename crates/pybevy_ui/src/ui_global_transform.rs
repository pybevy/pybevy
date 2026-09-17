use bevy::ui::UiGlobalTransform;
use pybevy_core::{ComponentStorage, PyComponent, computed_owned};
use pybevy_macros::pycomponent;
use pybevy_math::{
    affine2::{PyAffine2, PyMat2},
    rot2::PyRot2,
    vec2::PyVec2,
};
use pyo3::prelude::*;

#[pycomponent(UiGlobalTransform, bridge, no_insert)]
#[pyclass(name = "UiGlobalTransform", module = "pybevy.ui", extends = PyComponent, eq)]
#[derive(Debug)]
pub struct PyUiGlobalTransform {
    pub(crate) storage: ComponentStorage<UiGlobalTransform>,
}

impl PartialEq for PyUiGlobalTransform {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

#[pymethods]
impl PyUiGlobalTransform {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (UiGlobalTransform::default().into(), PyComponent).into()
    }

    #[staticmethod]
    pub fn from_translation(
        py: Python<'_>,
        translation: PyVec2,
    ) -> PyResult<Py<PyUiGlobalTransform>> {
        Py::new(
            py,
            (
                UiGlobalTransform::from_translation(translation.try_into()?).into(),
                PyComponent,
            ),
        )
    }

    #[staticmethod]
    pub fn from_xy(py: Python<'_>, x: f32, y: f32) -> PyResult<Py<PyUiGlobalTransform>> {
        Py::new(py, (UiGlobalTransform::from_xy(x, y).into(), PyComponent))
    }

    #[staticmethod]
    pub fn from_rotation(py: Python<'_>, rotation: PyRot2) -> PyResult<Py<PyUiGlobalTransform>> {
        Py::new(
            py,
            (
                UiGlobalTransform::from_rotation(rotation.try_into()?).into(),
                PyComponent,
            ),
        )
    }

    #[staticmethod]
    pub fn from_scale(py: Python<'_>, scale: PyVec2) -> PyResult<Py<PyUiGlobalTransform>> {
        Py::new(
            py,
            (
                UiGlobalTransform::from_scale(scale.try_into()?).into(),
                PyComponent,
            ),
        )
    }

    #[getter]
    pub fn matrix2(&self) -> PyResult<PyMat2> {
        Ok(computed_owned(self.as_ref()?.matrix2.into()))
    }

    #[getter]
    pub fn translation(&self) -> PyResult<PyVec2> {
        Ok(computed_owned(self.as_ref()?.translation.into()))
    }

    pub fn try_inverse(&self) -> PyResult<Option<PyAffine2>> {
        Ok(self.as_ref()?.try_inverse().map(Into::into))
    }

    pub fn to_scale_angle_translation(&self) -> PyResult<(PyVec2, f32, PyVec2)> {
        let (scale, angle, translation) = self.as_ref()?.to_scale_angle_translation();
        Ok((scale.into(), angle, translation.into()))
    }

    pub fn affine(&self) -> PyResult<PyAffine2> {
        Ok(self.as_ref()?.affine().into())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let transform = self.as_ref()?;
        Ok(format!(
            "UiGlobalTransform(matrix2={:?}, translation={:?})",
            transform.matrix2, transform.translation
        ))
    }
}
