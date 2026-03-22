use bevy::{math::Vec2, sprite::BorderRect};
use pybevy_math::PyVec2;
use pyo3::prelude::*;

#[pyclass(name = "BorderRect", frozen, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyBorderRect {
    #[pyo3(get)]
    pub min_inset: PyVec2,
    #[pyo3(get)]
    pub max_inset: PyVec2,
}

#[pymethods]
impl PyBorderRect {
    #[new]
    #[pyo3(signature = (min_inset = PyVec2::ZERO, max_inset = PyVec2::ZERO))]
    pub fn new(min_inset: PyVec2, max_inset: PyVec2) -> Self {
        Self {
            min_inset,
            max_inset,
        }
    }

    #[staticmethod]
    pub fn all(inset: f32) -> Self {
        Self {
            min_inset: PyVec2::splat(inset),
            max_inset: PyVec2::splat(inset),
        }
    }

    #[staticmethod]
    pub fn axes(horizontal: f32, vertical: f32) -> Self {
        let v = PyVec2::from_vec2(Vec2::new(horizontal, vertical));
        Self {
            min_inset: v.clone(),
            max_inset: v,
        }
    }

    pub fn __repr__(&self) -> String {
        format!(
            "BorderRect(min_inset={:?}, max_inset={:?})",
            self.min_inset, self.max_inset
        )
    }
}

impl From<PyBorderRect> for BorderRect {
    fn from(rect: PyBorderRect) -> Self {
        BorderRect {
            min_inset: rect.min_inset.into(),
            max_inset: rect.max_inset.into(),
        }
    }
}

impl From<BorderRect> for PyBorderRect {
    fn from(rect: BorderRect) -> Self {
        PyBorderRect {
            min_inset: PyVec2::from_vec2(rect.min_inset),
            max_inset: PyVec2::from_vec2(rect.max_inset),
        }
    }
}
