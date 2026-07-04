use bevy::animation::graph::AnimationNodeIndex;
use pyo3::prelude::*;

#[pyclass(name = "AnimationNodeIndex", eq, skip_from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyAnimationNodeIndex(pub AnimationNodeIndex);

#[pymethods]
impl PyAnimationNodeIndex {
    #[new]
    pub fn new(index: usize) -> Self {
        Self(AnimationNodeIndex::new(index))
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
