use bevy::math::{Rect, Vec2};
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::vec2::PyVec2;

#[pyclass(name = "Rect", frozen, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyRect {
    #[pyo3(get)]
    min: PyVec2,
    #[pyo3(get)]
    max: PyVec2,
}

#[pymethods]
impl PyRect {
    #[new]
    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        let rect = Rect::new(x0, y0, x1, y1);
        Self {
            min: rect.min.into(),
            max: rect.max.into(),
        }
    }

    #[staticmethod]
    pub fn from_corners(p0: PyVec2, p1: PyVec2) -> Self {
        let rect = Rect::from_corners(p0.into(), p1.into());
        Self {
            min: rect.min.into(),
            max: rect.max.into(),
        }
    }

    #[staticmethod]
    pub fn from_center_size(origin: PyVec2, size: PyVec2) -> Self {
        let rect = Rect::from_center_size(origin.into(), size.into());
        Self {
            min: rect.min.into(),
            max: rect.max.into(),
        }
    }

    #[staticmethod]
    pub fn from_center_half_size(origin: PyVec2, half_size: PyVec2) -> Self {
        let rect = Rect::from_center_half_size(origin.into(), half_size.into());
        Self {
            min: rect.min.into(),
            max: rect.max.into(),
        }
    }

    pub fn center(&self) -> PyVec2 {
        self.to_bevy().center().into()
    }

    pub fn size(&self) -> PyVec2 {
        self.to_bevy().size().into()
    }

    pub fn half_size(&self) -> PyVec2 {
        self.to_bevy().half_size().into()
    }

    pub fn width(&self) -> f32 {
        self.to_bevy().width()
    }

    pub fn height(&self) -> f32 {
        self.to_bevy().height()
    }

    pub fn contains(&self, point: PyVec2) -> bool {
        self.to_bevy().contains(point.into())
    }

    pub fn is_empty(&self) -> bool {
        self.to_bevy().is_empty()
    }

    pub fn intersect(&self, other: &PyRect) -> PyResult<PyRect> {
        let result = self.to_bevy().intersect(other.to_bevy());
        if result.is_empty() {
            Err(PyValueError::new_err("Rectangles do not intersect"))
        } else {
            Ok(Self {
                min: result.min.into(),
                max: result.max.into(),
            })
        }
    }

    pub fn union(&self, other: &PyRect) -> PyRect {
        let result = self.to_bevy().union(other.to_bevy());
        Self {
            min: result.min.into(),
            max: result.max.into(),
        }
    }

    pub fn union_point(&self, point: PyVec2) -> PyRect {
        let result = self.to_bevy().union_point(point.into());
        Self {
            min: result.min.into(),
            max: result.max.into(),
        }
    }

    pub fn inflate(&self, expansion: f32) -> PyRect {
        let result = self.to_bevy().inflate(expansion);
        Self {
            min: result.min.into(),
            max: result.max.into(),
        }
    }

    pub fn __repr__(&self) -> String {
        let min_vec: Vec2 = (&self.min).into();
        let max_vec: Vec2 = (&self.max).into();
        format!(
            "Rect(min=Vec2({}, {}), max=Vec2({}, {}))",
            min_vec.x, min_vec.y, max_vec.x, max_vec.y
        )
    }
}

impl PartialEq for PyRect {
    fn eq(&self, other: &Self) -> bool {
        self.to_bevy() == other.to_bevy()
    }
}

impl PyRect {
    fn to_bevy(&self) -> Rect {
        Rect {
            min: (&self.min).into(),
            max: (&self.max).into(),
        }
    }
}

impl From<Rect> for PyRect {
    fn from(rect: Rect) -> Self {
        Self {
            min: rect.min.into(),
            max: rect.max.into(),
        }
    }
}

impl From<PyRect> for Rect {
    fn from(rect: PyRect) -> Self {
        Rect {
            min: rect.min.into(),
            max: rect.max.into(),
        }
    }
}
