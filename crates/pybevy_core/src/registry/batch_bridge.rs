use bevy::ecs::{component::ComponentId, entity::Entity, world::World};
use pyo3::prelude::*;

/// A Python batch object converted into fully owned Rust values.
///
/// Preparation may fail while inspecting Python or NumPy. Once constructed,
/// insertion has no ordinary error channel and does not access Python.
pub trait PreparedBatchComponent: Send + 'static {
    fn count(&self) -> usize;

    fn insert(&mut self, component_id: ComponentId, entities: &[Entity], world: &mut World);
}

/// One value prepared for cloning across a uniform batch.
pub trait PreparedUniformComponent: Send + 'static {
    fn insert(&mut self, component_id: ComponentId, entities: &[Entity], world: &mut World);
}

/// Prepared values for an ordinary native Bevy component.
pub struct PreparedNativeBatch<T> {
    values: Vec<T>,
}

impl<T> PreparedNativeBatch<T> {
    pub fn new(values: Vec<T>) -> Self {
        Self { values }
    }
}

impl<T: bevy::ecs::component::Component> PreparedBatchComponent for PreparedNativeBatch<T> {
    fn count(&self) -> usize {
        self.values.len()
    }

    fn insert(&mut self, _component_id: ComponentId, entities: &[Entity], world: &mut World) {
        assert_eq!(
            self.values.len(),
            entities.len(),
            "validated native batch count changed before commit"
        );
        for (entity, value) in entities.iter().copied().zip(self.values.drain(..)) {
            world.entity_mut(entity).insert(value);
        }
    }
}

/// Prepared uniform value for an ordinary cloneable Bevy component.
pub struct PreparedNativeUniform<T> {
    value: T,
}

impl<T> PreparedNativeUniform<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T> PreparedUniformComponent for PreparedNativeUniform<T>
where
    T: bevy::ecs::component::Component + Clone,
{
    fn insert(&mut self, _component_id: ComponentId, entities: &[Entity], world: &mut World) {
        for &entity in entities {
            world.entity_mut(entity).insert(self.value.clone());
        }
    }
}

/// Prepared insertion for constructible markers that do not implement Clone.
pub struct PreparedUniformFn {
    insert_one: fn(Entity, &mut World),
}

impl PreparedUniformFn {
    pub fn new(insert_one: fn(Entity, &mut World)) -> Self {
        Self { insert_one }
    }
}

impl PreparedUniformComponent for PreparedUniformFn {
    fn insert(&mut self, _component_id: ComponentId, entities: &[Entity], world: &mut World) {
        for &entity in entities {
            (self.insert_one)(entity, world);
        }
    }
}

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

    /// Copy/convert all interpreter-owned data into an owned Rust payload.
    fn prepare(
        &self,
        py: Python,
        batch: &Bound<PyAny>,
    ) -> PyResult<Box<dyn PreparedBatchComponent>>;

    /// Insert components for all entities in bulk (post-spawn).
    fn insert_bulk(
        &self,
        py: Python,
        batch: &Bound<PyAny>,
        entities: &[Entity],
        world: &mut World,
    ) -> PyResult<()>;
}
