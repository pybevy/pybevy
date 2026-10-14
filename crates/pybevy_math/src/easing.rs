use bevy::curve::{Curve, EaseFunction, easing::JumpAt};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::interval::PyInterval;

#[pyenum(JumpAt)]
#[pyclass(name = "JumpAt", module = "pybevy.math", eq, from_py_object, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyJumpAt {
    Start,
    End,
    #[pyo3(name = "None_")]
    None,
    Both,
}

#[pyenum(EaseFunction, empty_tuple, unit_parens, non_exhaustive)]
#[pyclass(
    name = "EaseFunction",
    module = "pybevy.math",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyEaseFunction {
    Linear(),
    QuadraticIn(),
    QuadraticOut(),
    QuadraticInOut(),
    CubicIn(),
    CubicOut(),
    CubicInOut(),
    QuarticIn(),
    QuarticOut(),
    QuarticInOut(),
    QuinticIn(),
    QuinticOut(),
    QuinticInOut(),
    SmoothStepIn(),
    SmoothStepOut(),
    SmoothStep(),
    SmootherStepIn(),
    SmootherStepOut(),
    SmootherStep(),
    SineIn(),
    SineOut(),
    SineInOut(),
    CircularIn(),
    CircularOut(),
    CircularInOut(),
    ExponentialIn(),
    ExponentialOut(),
    ExponentialInOut(),
    ElasticIn(),
    ElasticOut(),
    ElasticInOut(),
    BackIn(),
    BackOut(),
    BackInOut(),
    BounceIn(),
    BounceOut(),
    BounceInOut(),
    #[py_bevy(tuple)]
    Steps {
        steps: usize,
        #[py_type(PyJumpAt)]
        jump: JumpAt,
    },
    #[py_bevy(tuple)]
    Elastic {
        omega: f32,
    },
}

#[pymethods]
impl PyEaseFunction {
    pub fn domain(&self) -> PyInterval {
        EaseFunction::from(*self).domain().into()
    }

    pub fn sample(&self, t: f32) -> Option<f32> {
        EaseFunction::from(*self).sample(t)
    }

    pub fn sample_clamped(&self, t: f32) -> f32 {
        EaseFunction::from(*self).sample_clamped(t)
    }
}
