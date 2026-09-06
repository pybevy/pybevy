use pyo3::prelude::*;

#[pyclass(
    name = "AnimationCurve",
    module = "pybevy.animation",
    subclass,
    skip_from_py_object
)]
#[derive(Debug, Clone, Default)]
pub struct PyAnimationCurve;

#[pyclass(name = "AnimatableCurve", module = "pybevy.animation")]
pub struct PyAnimatableCurve {}

#[pyclass(name = "AnimatableKeyframeCurve", module = "pybevy.animation")]
pub struct PyAnimatableKeyframeCurve {}

#[pyclass(name = "WeightsCurve", module = "pybevy.animation")]
pub struct PyWeightsCurve {}

#[pyclass(name = "AnimatedField", module = "pybevy.animation")]
pub struct PyAnimatedField {}
