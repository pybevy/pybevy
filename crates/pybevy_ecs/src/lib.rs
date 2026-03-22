//! PyBevy ECS - ECS components for PyBevy
//!
//! This crate provides Python bindings for Bevy's ECS components.

pub mod name;

use bevy::ecs::name::Name;
pub use name::PyName;
use pybevy_core::registry::global_registry;
use pybevy_macros::component_bridge;
use pyo3::prelude::*;

// Generate component bridges
component_bridge!(Name, PyName);

/// Register ECS component bridges with global registries
pub fn register_ecs_bridges() {
    global_registry::register_component_bridge(NameBridge);
}

/// Add ECS module classes to Python module
pub fn add_ecs_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_ecs_bridges();

    m.add_class::<PyName>()?;
    Ok(())
}
