use bevy::ecs::{entity::Entity, world::World};
use pyo3::prelude::*;

/// Trait for batch component types (e.g., TransformBatch, VisibilityBatch).
///
/// Enables dynamic dispatch for batch spawning without the main crate
/// needing to know about specific batch types at compile time.
pub trait BatchComponent: Send + Sync + 'static {
    /// Human-readable name for error messages
    fn name(&self) -> &'static str;

    /// Python type-object identity of the component produced by this batch.
    ///
    /// The batch wrapper's own type is not necessarily the component type
    /// (`TransformBatch` produces `Transform`). Lifecycle dispatch therefore
    /// asks the bridge for the exact component class identity.
    fn component_type_ptr(&self, py: Python, batch: &Bound<PyAny>) -> PyResult<usize>;

    /// Get the count of entities in this batch
    fn count(&self, py: Python, batch: &Bound<PyAny>) -> PyResult<usize>;

    /// Insert components for all entities in bulk (post-spawn).
    fn insert_bulk(
        &self,
        py: Python,
        batch: &Bound<PyAny>,
        entities: &[Entity],
        world: &mut World,
    ) -> PyResult<()>;
}
