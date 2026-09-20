use bevy::ecs::{
    component::{Component, ComponentId},
    entity::Entity,
    world::World,
};
use pyo3::{PyTraverseError, PyVisit, prelude::*};

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

    fn traverse(&self, _visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        Ok(())
    }
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

impl<T: Component> PreparedBatchComponent for PreparedNativeBatch<T> {
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

/// Prepared uniform value for a component copied by an explicit adapter.
pub struct PreparedNativeUniformWith<T> {
    value: T,
    clone_with: fn(&T) -> T,
}

impl<T> PreparedNativeUniformWith<T> {
    pub fn new(value: T, clone_with: fn(&T) -> T) -> Self {
        Self { value, clone_with }
    }
}

impl<T> PreparedUniformComponent for PreparedNativeUniformWith<T>
where
    T: Component,
{
    fn insert(&mut self, _component_id: ComponentId, entities: &[Entity], world: &mut World) {
        for &entity in entities {
            world
                .entity_mut(entity)
                .insert((self.clone_with)(&self.value));
        }
    }
}

impl<T> PreparedNativeUniform<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T> PreparedUniformComponent for PreparedNativeUniform<T>
where
    T: Component + Clone,
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI32, Ordering};

    use bevy::ecs::{component::Component, entity::Entity, world::World};

    use super::{
        PreparedBatchComponent, PreparedNativeBatch, PreparedNativeUniform,
        PreparedNativeUniformWith, PreparedUniformComponent, PreparedUniformFn,
    };

    #[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
    struct Marker {
        value: i32,
    }

    #[test]
    fn prepared_native_batch_inserts_each_value_on_its_own_entity() {
        let mut world = World::new();
        let component_id = world.register_component::<Marker>();
        let entities: Vec<Entity> = (0..3).map(|_| world.spawn_empty().id()).collect();
        let unrelated = world.spawn_empty().id();

        let mut batch = PreparedNativeBatch::new(vec![
            Marker { value: 1 },
            Marker { value: 2 },
            Marker { value: 3 },
        ]);
        assert_eq!(batch.count(), 3);
        batch.insert(component_id, &entities, &mut world);

        for (entity, expected) in entities.iter().copied().zip([1, 2, 3]) {
            assert_eq!(
                world.get::<Marker>(entity).unwrap(),
                &Marker { value: expected }
            );
        }
        assert!(world.get::<Marker>(unrelated).is_none());
    }

    #[test]
    fn prepared_native_batch_rejects_a_changed_count_before_commit() {
        let mut world = World::new();
        let component_id = world.register_component::<Marker>();
        let entity = world.spawn_empty().id();
        let mut batch = PreparedNativeBatch::new(vec![Marker { value: 1 }, Marker { value: 2 }]);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            batch.insert(component_id, &[entity], &mut world);
        }));

        let payload = result.unwrap_err();
        let message = payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        assert!(
            message.contains("validated native batch count changed before commit"),
            "unexpected panic payload: {message:?}"
        );
        // The guard fires before the insertion loop: state is untouched.
        assert!(world.get::<Marker>(entity).is_none());
    }

    #[test]
    fn prepared_native_uniform_clones_one_value_to_every_entity() {
        let mut world = World::new();
        let component_id = world.register_component::<Marker>();
        let entities: Vec<Entity> = (0..3).map(|_| world.spawn_empty().id()).collect();

        PreparedNativeUniform::new(Marker { value: 7 }).insert(component_id, &entities, &mut world);

        for entity in entities {
            assert_eq!(world.get::<Marker>(entity).unwrap(), &Marker { value: 7 });
        }
    }

    #[test]
    fn prepared_native_uniform_with_uses_the_explicit_clone_adapter() {
        let mut world = World::new();
        let component_id = world.register_component::<Marker>();
        let entities: Vec<Entity> = (0..2).map(|_| world.spawn_empty().id()).collect();

        PreparedNativeUniformWith::new(Marker { value: 7 }, |marker| Marker {
            value: marker.value * 10,
        })
        .insert(component_id, &entities, &mut world);

        for entity in entities {
            // A plain Clone would leave 7; the adapter must be applied.
            assert_eq!(world.get::<Marker>(entity).unwrap(), &Marker { value: 70 });
        }
    }

    fn insert_counter_marker(entity: Entity, world: &mut World) {
        INSERT_CALLS.fetch_add(1, Ordering::SeqCst);
        world.entity_mut(entity).insert(Marker { value: 42 });
    }

    static INSERT_CALLS: AtomicI32 = AtomicI32::new(0);

    #[test]
    fn prepared_uniform_fn_runs_once_per_entity() {
        let mut world = World::new();
        let component_id = world.register_component::<Marker>();
        let entities: Vec<Entity> = (0..2).map(|_| world.spawn_empty().id()).collect();
        let before = INSERT_CALLS.load(Ordering::SeqCst);

        PreparedUniformFn::new(insert_counter_marker).insert(component_id, &entities, &mut world);

        assert_eq!(INSERT_CALLS.load(Ordering::SeqCst) - before, 2);
        for entity in entities {
            assert_eq!(world.get::<Marker>(entity).unwrap(), &Marker { value: 42 });
        }
    }
}
