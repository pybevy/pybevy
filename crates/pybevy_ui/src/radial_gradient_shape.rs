use bevy::ui::{RadialGradientShape, Val};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::val::PyVal;

#[pyenum(RadialGradientShape, empty_tuple, unit_parens)]
#[pyclass(
    name = "RadialGradientShape",
    module = "pybevy.ui",
    eq,
    frozen,
    from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub enum PyRadialGradientShape {
    ClosestSide(),
    FarthestSide(),
    ClosestCorner(),
    FarthestCorner(),
    #[py_bevy(tuple)]
    Circle {
        #[py_type(PyVal)]
        value: Val,
    },
    #[py_bevy(tuple)]
    Ellipse {
        #[py_type(PyVal)]
        width: Val,
        #[py_type(PyVal)]
        height: Val,
    },
}

impl Default for PyRadialGradientShape {
    fn default() -> Self {
        Self::ClosestCorner()
    }
}
