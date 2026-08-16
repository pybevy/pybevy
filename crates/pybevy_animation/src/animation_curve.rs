use pyo3::prelude::*;

#[pyclass(name = "AnimationCurve", subclass, skip_from_py_object)]
#[derive(Debug, Clone, Default)]
pub struct PyAnimationCurve;

#[pyclass(name = "AnimatableCurve")]
pub struct PyAnimatableCurve {}

#[pyclass(name = "AnimatableKeyframeCurve")]
pub struct PyAnimatableKeyframeCurve {}

#[pyclass(name = "WeightsCurve")]
pub struct PyWeightsCurve {}

#[pyclass(name = "AnimatedField")]
pub struct PyAnimatedField {}
