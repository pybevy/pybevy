use bevy::{
    ecs::entity::ContainsEntity,
    window::{NormalizedWindowRef, WindowRef},
};
use pybevy_core::PyEntity;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(WindowRef, manual)]
#[pyclass(
    name = "WindowRef",
    module = "pybevy.window",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyWindowRef {
    Primary(),
    Entity { value: PyEntity },
}

impl From<WindowRef> for PyWindowRef {
    fn from(value: WindowRef) -> Self {
        match value {
            WindowRef::Primary => Self::Primary(),
            WindowRef::Entity(entity) => Self::Entity {
                value: entity.into(),
            },
        }
    }
}

impl From<PyWindowRef> for WindowRef {
    fn from(value: PyWindowRef) -> Self {
        match value {
            PyWindowRef::Primary() => Self::Primary,
            PyWindowRef::Entity { value } => Self::Entity(value.into()),
        }
    }
}

impl Default for PyWindowRef {
    fn default() -> Self {
        WindowRef::default().into()
    }
}

#[pymethods]
impl PyWindowRef {
    #[pyo3(signature = (primary_window = None))]
    pub fn normalize(&self, primary_window: Option<PyEntity>) -> Option<PyNormalizedWindowRef> {
        let window_ref: WindowRef = (*self).into();
        window_ref
            .normalize(primary_window.map(Into::into))
            .map(Into::into)
    }

    fn __repr__(&self) -> String {
        match self {
            Self::Primary() => "WindowRef.Primary()".to_string(),
            Self::Entity { value } => format!("WindowRef.Entity({value:?})"),
        }
    }
}

#[pyclass(
    name = "NormalizedWindowRef",
    module = "pybevy.window",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyNormalizedWindowRef {
    inner: NormalizedWindowRef,
}

impl From<NormalizedWindowRef> for PyNormalizedWindowRef {
    fn from(value: NormalizedWindowRef) -> Self {
        Self { inner: value }
    }
}

impl From<PyNormalizedWindowRef> for NormalizedWindowRef {
    fn from(value: PyNormalizedWindowRef) -> Self {
        value.inner
    }
}

#[pymethods]
impl PyNormalizedWindowRef {
    #[getter]
    pub fn entity(&self) -> PyEntity {
        self.inner.entity().into()
    }

    fn __repr__(&self) -> String {
        format!("NormalizedWindowRef({:?})", self.inner.entity())
    }
}
