use bevy::animation::graph::AnimationNodeIndex;
use pyo3::{exceptions::PyValueError, prelude::*};

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
    pub fn new(index: usize) -> PyResult<Self> {
        u32::try_from(index).map_err(|_| {
            PyValueError::new_err(format!(
                "AnimationNodeIndex index must fit in u32 (got {index})"
            ))
        })?;
        Ok(Self(AnimationNodeIndex::new(index)))
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
