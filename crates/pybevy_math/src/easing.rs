use bevy_math::curve::{Curve, EaseFunction, easing::JumpAt};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(JumpAt)]
#[pyclass(name = "JumpAt", module = "pybevy.math", eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyJumpAt {
    Start,
    End,
    #[pyo3(name = "None_")]
    None,
    Both,
}

#[pyclass(name = "EaseFunction", module = "pybevy.math", skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyEaseFunction {
    ease_fn: EaseFunction,
}

impl From<PyEaseFunction> for EaseFunction {
    fn from(py_ease: PyEaseFunction) -> Self {
        py_ease.ease_fn
    }
}

impl From<EaseFunction> for PyEaseFunction {
    fn from(ease_fn: EaseFunction) -> Self {
        PyEaseFunction { ease_fn }
    }
}

impl PyEaseFunction {
    fn new(ease_fn: EaseFunction) -> Self {
        Self { ease_fn }
    }
}

#[pymethods]
impl PyEaseFunction {
    #[classattr]
    #[pyo3(name = "Linear")]
    fn linear() -> Self {
        Self::new(EaseFunction::Linear)
    }
    #[classattr]
    #[pyo3(name = "QuadraticIn")]
    fn quadratic_in() -> Self {
        Self::new(EaseFunction::QuadraticIn)
    }
    #[classattr]
    #[pyo3(name = "QuadraticOut")]
    fn quadratic_out() -> Self {
        Self::new(EaseFunction::QuadraticOut)
    }
    #[classattr]
    #[pyo3(name = "QuadraticInOut")]
    fn quadratic_in_out() -> Self {
        Self::new(EaseFunction::QuadraticInOut)
    }
    #[classattr]
    #[pyo3(name = "CubicIn")]
    fn cubic_in() -> Self {
        Self::new(EaseFunction::CubicIn)
    }
    #[classattr]
    #[pyo3(name = "CubicOut")]
    fn cubic_out() -> Self {
        Self::new(EaseFunction::CubicOut)
    }
    #[classattr]
    #[pyo3(name = "CubicInOut")]
    fn cubic_in_out() -> Self {
        Self::new(EaseFunction::CubicInOut)
    }
    #[classattr]
    #[pyo3(name = "QuarticIn")]
    fn quartic_in() -> Self {
        Self::new(EaseFunction::QuarticIn)
    }
    #[classattr]
    #[pyo3(name = "QuarticOut")]
    fn quartic_out() -> Self {
        Self::new(EaseFunction::QuarticOut)
    }
    #[classattr]
    #[pyo3(name = "QuarticInOut")]
    fn quartic_in_out() -> Self {
        Self::new(EaseFunction::QuarticInOut)
    }
    #[classattr]
    #[pyo3(name = "QuinticIn")]
    fn quintic_in() -> Self {
        Self::new(EaseFunction::QuinticIn)
    }
    #[classattr]
    #[pyo3(name = "QuinticOut")]
    fn quintic_out() -> Self {
        Self::new(EaseFunction::QuinticOut)
    }
    #[classattr]
    #[pyo3(name = "QuinticInOut")]
    fn quintic_in_out() -> Self {
        Self::new(EaseFunction::QuinticInOut)
    }
    #[classattr]
    #[pyo3(name = "SineIn")]
    fn sine_in() -> Self {
        Self::new(EaseFunction::SineIn)
    }
    #[classattr]
    #[pyo3(name = "SineOut")]
    fn sine_out() -> Self {
        Self::new(EaseFunction::SineOut)
    }
    #[classattr]
    #[pyo3(name = "SineInOut")]
    fn sine_in_out() -> Self {
        Self::new(EaseFunction::SineInOut)
    }
    #[classattr]
    #[pyo3(name = "CircularIn")]
    fn circular_in() -> Self {
        Self::new(EaseFunction::CircularIn)
    }
    #[classattr]
    #[pyo3(name = "CircularOut")]
    fn circular_out() -> Self {
        Self::new(EaseFunction::CircularOut)
    }
    #[classattr]
    #[pyo3(name = "CircularInOut")]
    fn circular_in_out() -> Self {
        Self::new(EaseFunction::CircularInOut)
    }
    #[classattr]
    #[pyo3(name = "ExponentialIn")]
    fn exponential_in() -> Self {
        Self::new(EaseFunction::ExponentialIn)
    }
    #[classattr]
    #[pyo3(name = "ExponentialOut")]
    fn exponential_out() -> Self {
        Self::new(EaseFunction::ExponentialOut)
    }
    #[classattr]
    #[pyo3(name = "ExponentialInOut")]
    fn exponential_in_out() -> Self {
        Self::new(EaseFunction::ExponentialInOut)
    }
    #[classattr]
    #[pyo3(name = "ElasticIn")]
    fn elastic_in() -> Self {
        Self::new(EaseFunction::ElasticIn)
    }
    #[classattr]
    #[pyo3(name = "ElasticOut")]
    fn elastic_out() -> Self {
        Self::new(EaseFunction::ElasticOut)
    }
    #[classattr]
    #[pyo3(name = "ElasticInOut")]
    fn elastic_in_out() -> Self {
        Self::new(EaseFunction::ElasticInOut)
    }
    #[classattr]
    #[pyo3(name = "BackIn")]
    fn back_in() -> Self {
        Self::new(EaseFunction::BackIn)
    }
    #[classattr]
    #[pyo3(name = "BackOut")]
    fn back_out() -> Self {
        Self::new(EaseFunction::BackOut)
    }
    #[classattr]
    #[pyo3(name = "BackInOut")]
    fn back_in_out() -> Self {
        Self::new(EaseFunction::BackInOut)
    }
    #[classattr]
    #[pyo3(name = "BounceIn")]
    fn bounce_in() -> Self {
        Self::new(EaseFunction::BounceIn)
    }
    #[classattr]
    #[pyo3(name = "BounceOut")]
    fn bounce_out() -> Self {
        Self::new(EaseFunction::BounceOut)
    }
    #[classattr]
    #[pyo3(name = "BounceInOut")]
    fn bounce_in_out() -> Self {
        Self::new(EaseFunction::BounceInOut)
    }
    #[classattr]
    #[pyo3(name = "SmoothStep")]
    fn smooth_step() -> Self {
        Self::new(EaseFunction::SmoothStep)
    }
    #[classattr]
    #[pyo3(name = "SmoothStepIn")]
    fn smooth_step_in() -> Self {
        Self::new(EaseFunction::SmoothStepIn)
    }
    #[classattr]
    #[pyo3(name = "SmoothStepOut")]
    fn smooth_step_out() -> Self {
        Self::new(EaseFunction::SmoothStepOut)
    }
    #[classattr]
    #[pyo3(name = "SmootherStep")]
    fn smoother_step() -> Self {
        Self::new(EaseFunction::SmootherStep)
    }
    #[classattr]
    #[pyo3(name = "SmootherStepIn")]
    fn smoother_step_in() -> Self {
        Self::new(EaseFunction::SmootherStepIn)
    }
    #[classattr]
    #[pyo3(name = "SmootherStepOut")]
    fn smoother_step_out() -> Self {
        Self::new(EaseFunction::SmootherStepOut)
    }

    #[staticmethod]
    #[pyo3(name = "Elastic")]
    fn elastic(amplitude: f32) -> Self {
        Self::new(EaseFunction::Elastic(amplitude))
    }

    #[staticmethod]
    #[pyo3(name = "Steps")]
    fn steps(steps: usize, jump: PyJumpAt) -> Self {
        Self::new(EaseFunction::Steps(steps, jump.into()))
    }

    pub fn ease(&self, t: f32) -> f32 {
        self.ease_fn.sample_clamped(t)
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.ease_fn)
    }
}
