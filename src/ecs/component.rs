use bevy::ecs::{bundle::Bundle, component::ComponentId};
// Re-export PyComponent from pybevy_core to ensure all crates use the same type
pub use pybevy_core::PyComponent;
use pyo3::{PyClass, prelude::*};

use crate::ecs::component_type::PyComponentType;

/// Opaque identifier for a registered Bevy component type.
///
/// Returned by `World.component_id()` and used for low-level ECS operations.
#[pyclass(name = "ComponentId", eq, frozen)]
#[derive(Debug, PartialEq)]
pub struct PyComponentId(pub(crate) ComponentId);

#[pymethods]
impl PyComponentId {}

pub trait NativeComponent: Sized + PyClass {
    type Native: Bundle + TryFrom<Self, Error = PyErr>;

    #[allow(dead_code)]
    fn component_type() -> PyComponentType;
}
