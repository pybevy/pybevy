use bevy::asset::{Asset, AssetEvent};
use pybevy_core::handle::PyHandle;
use pyo3::prelude::*;

use crate::ecs::message::PyMessage;

#[pyclass(name = "AssetEventType", eq, eq_int, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyAssetEventType {
    Added,
    Modified,
    Removed,
    Unused,
    LoadedWithDependencies,
}

#[pymethods]
impl PyAssetEventType {
    fn __repr__(&self) -> &'static str {
        match self {
            PyAssetEventType::Added => "AssetEventType.Added",
            PyAssetEventType::Modified => "AssetEventType.Modified",
            PyAssetEventType::Removed => "AssetEventType.Removed",
            PyAssetEventType::Unused => "AssetEventType.Unused",
            PyAssetEventType::LoadedWithDependencies => "AssetEventType.LoadedWithDependencies",
        }
    }

    fn __str__(&self) -> &'static str {
        match self {
            PyAssetEventType::Added => "Added",
            PyAssetEventType::Modified => "Modified",
            PyAssetEventType::Removed => "Removed",
            PyAssetEventType::Unused => "Unused",
            PyAssetEventType::LoadedWithDependencies => "LoadedWithDependencies",
        }
    }
}

#[pyclass(name = "AssetEvent", extends = PyMessage, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyAssetEvent {
    #[pyo3(get)]
    pub handle: PyHandle,

    #[pyo3(get)]
    pub event_type: PyAssetEventType,
}

impl PyAssetEvent {
    pub fn from_bevy<A: Asset>(event: &AssetEvent<A>) -> Self {
        let (id, event_type) = match event {
            AssetEvent::Added { id } => (*id, PyAssetEventType::Added),
            AssetEvent::Modified { id } => (*id, PyAssetEventType::Modified),
            AssetEvent::Removed { id } => (*id, PyAssetEventType::Removed),
            AssetEvent::Unused { id } => (*id, PyAssetEventType::Unused),
            AssetEvent::LoadedWithDependencies { id } => {
                (*id, PyAssetEventType::LoadedWithDependencies)
            }
        };

        PyAssetEvent {
            handle: PyHandle::from(&id),
            event_type,
        }
    }
}

#[pymethods]
impl PyAssetEvent {
    pub fn is_added(&self) -> bool {
        self.event_type == PyAssetEventType::Added
    }

    pub fn is_modified(&self) -> bool {
        self.event_type == PyAssetEventType::Modified
    }

    pub fn is_removed(&self) -> bool {
        self.event_type == PyAssetEventType::Removed
    }

    pub fn is_unused(&self) -> bool {
        self.event_type == PyAssetEventType::Unused
    }

    pub fn is_loaded_with_dependencies(&self) -> bool {
        self.event_type == PyAssetEventType::LoadedWithDependencies
    }

    fn __repr__(&self) -> String {
        format!(
            "AssetEvent(handle={:?}, type={})",
            self.handle,
            self.event_type.__str__()
        )
    }
}
