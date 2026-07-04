use bevy::ui::RadialGradientShape;
use pyo3::prelude::*;

use crate::val::PyVal;

#[pyclass(name = "RadialGradientShape", eq, from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyRadialGradientShape {
    pub(crate) inner: RadialGradientShape,
}

impl From<RadialGradientShape> for PyRadialGradientShape {
    fn from(shape: RadialGradientShape) -> Self {
        PyRadialGradientShape { inner: shape }
    }
}

impl From<PyRadialGradientShape> for RadialGradientShape {
    fn from(py_shape: PyRadialGradientShape) -> Self {
        py_shape.inner
    }
}

impl Default for PyRadialGradientShape {
    fn default() -> Self {
        Self::new()
    }
}

#[pymethods]
impl PyRadialGradientShape {
    #[new]
    pub fn new() -> Self {
        PyRadialGradientShape {
            inner: RadialGradientShape::default(),
        }
    }

    #[staticmethod]
    pub fn closest_side() -> Self {
        PyRadialGradientShape {
            inner: RadialGradientShape::ClosestSide,
        }
    }

    #[staticmethod]
    pub fn farthest_side() -> Self {
        PyRadialGradientShape {
            inner: RadialGradientShape::FarthestSide,
        }
    }

    #[staticmethod]
    pub fn closest_corner() -> Self {
        PyRadialGradientShape {
            inner: RadialGradientShape::ClosestCorner,
        }
    }

    #[staticmethod]
    pub fn farthest_corner() -> Self {
        PyRadialGradientShape {
            inner: RadialGradientShape::FarthestCorner,
        }
    }

    #[staticmethod]
    pub fn circle(radius: PyVal) -> Self {
        PyRadialGradientShape {
            inner: RadialGradientShape::Circle(radius.into()),
        }
    }

    #[staticmethod]
    pub fn ellipse(width: PyVal, height: PyVal) -> Self {
        PyRadialGradientShape {
            inner: RadialGradientShape::Ellipse(width.into(), height.into()),
        }
    }

    pub fn __repr__(&self) -> String {
        match &self.inner {
            RadialGradientShape::ClosestSide => "RadialGradientShape.closest_side()".to_string(),
            RadialGradientShape::FarthestSide => "RadialGradientShape.farthest_side()".to_string(),
            RadialGradientShape::ClosestCorner => {
                "RadialGradientShape.closest_corner()".to_string()
            }
            RadialGradientShape::FarthestCorner => {
                "RadialGradientShape.farthest_corner()".to_string()
            }
            RadialGradientShape::Circle(r) => format!("RadialGradientShape.circle({:?})", r),
            RadialGradientShape::Ellipse(w, h) => {
                format!("RadialGradientShape.ellipse({:?}, {:?})", w, h)
            }
        }
    }
}
