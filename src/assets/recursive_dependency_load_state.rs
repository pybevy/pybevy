use bevy::asset::RecursiveDependencyLoadState;
use pyo3::prelude::*;

#[pyclass(name = "RecursiveDependencyLoadState", eq, frozen, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PyRecursiveDependencyLoadState {
    NotLoaded(),
    Loading(),
    Loaded(),
    Failed(),
}

impl From<RecursiveDependencyLoadState> for PyRecursiveDependencyLoadState {
    fn from(state: RecursiveDependencyLoadState) -> Self {
        match state {
            RecursiveDependencyLoadState::NotLoaded => Self::NotLoaded(),
            RecursiveDependencyLoadState::Loading => Self::Loading(),
            RecursiveDependencyLoadState::Loaded => Self::Loaded(),
            RecursiveDependencyLoadState::Failed(_) => Self::Failed(),
        }
    }
}

#[pymethods]
impl PyRecursiveDependencyLoadState {
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading())
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded())
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed())
    }

    fn __repr__(&self) -> &'static str {
        match self {
            Self::NotLoaded() => "RecursiveDependencyLoadState.NotLoaded",
            Self::Loading() => "RecursiveDependencyLoadState.Loading",
            Self::Loaded() => "RecursiveDependencyLoadState.Loaded",
            Self::Failed() => "RecursiveDependencyLoadState.Failed",
        }
    }
}
