use bevy::input::touch::ForceTouch;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ForceTouch)]
#[pyclass(
    name = "ForceTouch",
    module = "pybevy.input",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyForceTouch {
    #[pyo3(constructor = (*, force, max_possible_force, altitude_angle))]
    Calibrated {
        force: f64,
        max_possible_force: f64,
        altitude_angle: Option<f64>,
    },
    #[py_bevy(tuple)]
    Normalized { value: f64 },
}
