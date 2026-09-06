use bevy::ecs::component::ComponentId;
// Re-export PyComponent from pybevy_core to ensure all crates use the same type
pub use pybevy_core::PyComponent;
use pyo3::prelude::*;

/// Opaque identifier for a registered Bevy component type.
///
/// Returned by `World.component_id()` and used for low-level ECS operations.
#[pyclass(name = "ComponentId", module = "pybevy.ecs", eq, frozen)]
#[derive(Debug, PartialEq)]
pub struct PyComponentId(pub(crate) ComponentId);

#[pymethods]
impl PyComponentId {}
