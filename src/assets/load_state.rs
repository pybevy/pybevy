use bevy::asset::LoadState;
use pyo3::prelude::*;

#[pyclass(name = "LoadState", eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PyLoadState {
    NotLoaded(),
    Loading(),
    Loaded(),
    Failed(),
}

impl From<LoadState> for PyLoadState {
    fn from(state: LoadState) -> Self {
        match state {
            LoadState::NotLoaded => PyLoadState::NotLoaded(),
            LoadState::Loading => PyLoadState::Loading(),
            LoadState::Loaded => PyLoadState::Loaded(),
            LoadState::Failed(_) => PyLoadState::Failed(),
        }
    }
}

#[pymethods]
impl PyLoadState {
    pub fn is_not_loaded(&self) -> bool {
        matches!(self, PyLoadState::NotLoaded())
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, PyLoadState::Loading())
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, PyLoadState::Loaded())
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, PyLoadState::Failed())
    }

    fn __repr__(&self) -> &'static str {
        match self {
            PyLoadState::NotLoaded() => "LoadState.NotLoaded",
            PyLoadState::Loading() => "LoadState.Loading",
            PyLoadState::Loaded() => "LoadState.Loaded",
            PyLoadState::Failed() => "LoadState.Failed",
        }
    }

    fn __str__(&self) -> &'static str {
        match self {
            PyLoadState::NotLoaded() => "NotLoaded",
            PyLoadState::Loading() => "Loading",
            PyLoadState::Loaded() => "Loaded",
            PyLoadState::Failed() => "Failed",
        }
    }
}
