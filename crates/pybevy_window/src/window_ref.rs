use bevy::{
    ecs::entity::{ContainsEntity, Entity},
    window::{NormalizedWindowRef, WindowRef},
};
use pybevy_core::PyEntity;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(WindowRef, empty_tuple, no_repr)]
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
    #[py_bevy(tuple)]
    Entity {
        #[py_type(PyEntity)]
        value: Entity,
    },
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
            Self::Entity { value } => {
                let value: PyEntity = (*value).into();
                format!("WindowRef.Entity({value:?})")
            }
        }
    }
}

#[pyclass(
    name = "NormalizedWindowRef",
    module = "pybevy.window",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
