use bevy::asset::DependencyLoadState;
use pyo3::prelude::*;

#[pyclass(name = "DependencyLoadState", eq, frozen, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PyDependencyLoadState {
    NotLoaded(),
    Loading(),
    Loaded(),
    Failed(),
}

impl From<DependencyLoadState> for PyDependencyLoadState {
    fn from(state: DependencyLoadState) -> Self {
        match state {
            DependencyLoadState::NotLoaded => PyDependencyLoadState::NotLoaded(),
            DependencyLoadState::Loading => PyDependencyLoadState::Loading(),
            DependencyLoadState::Loaded => PyDependencyLoadState::Loaded(),
            DependencyLoadState::Failed(_) => PyDependencyLoadState::Failed(),
        }
    }
}

#[pymethods]
impl PyDependencyLoadState {
    pub fn is_not_loaded(&self) -> bool {
        matches!(self, Self::NotLoaded())
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading())
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded())
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed())
    }

    fn __str__(&self) -> &'static str {
        match self {
            Self::NotLoaded() => "NotLoaded",
            Self::Loading() => "Loading",
            Self::Loaded() => "Loaded",
            Self::Failed() => "Failed",
        }
    }

    fn __repr__(&self) -> &'static str {
        match self {
            Self::NotLoaded() => "DependencyLoadState.NotLoaded",
            Self::Loading() => "DependencyLoadState.Loading",
            Self::Loaded() => "DependencyLoadState.Loaded",
            Self::Failed() => "DependencyLoadState.Failed",
        }
    }
}
