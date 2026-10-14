use bevy::animation::graph::AnimationNodeIndex;
use pybevy_core::domain::{IntegerInput, NON_NEGATIVE, check_for};
use pyo3::prelude::*;

#[pyclass(
    name = "AnimationNodeIndex",
    module = "pybevy.animation",
    eq,
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyAnimationNodeIndex(pub AnimationNodeIndex);

#[pymethods]
impl PyAnimationNodeIndex {
    #[new]
    pub fn new(index: IntegerInput<u32>) -> PyResult<Self> {
        let index = check_for::<Self, _>("index", index, NON_NEGATIVE)?.stored();
        Ok(Self(AnimationNodeIndex::new(index as usize)))
    }

    pub fn index(&self) -> usize {
        self.0.index()
    }

    pub fn __repr__(&self) -> String {
        format!("AnimationNodeIndex({})", self.index())
    }
}

impl From<AnimationNodeIndex> for PyAnimationNodeIndex {
    fn from(index: AnimationNodeIndex) -> Self {
        PyAnimationNodeIndex(index)
    }
}

impl From<PyAnimationNodeIndex> for AnimationNodeIndex {
    fn from(index: PyAnimationNodeIndex) -> Self {
        index.0
    }
}
