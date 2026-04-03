use bevy::ecs::world::World;
use pyo3::Python;
use serde_json::Value;

use crate::{
    bridge::{ControlError, ControlRequest, EntityRef},
    handlers,
    runtime::ControlRuntime,
};

/// PyO3-based implementation of `ControlRuntime`.
///
/// Delegates all operations to the existing handler functions,
/// wrapping GIL acquisition in `dispatch_batch`.
pub struct Pyo3ControlRuntime;

impl ControlRuntime for Pyo3ControlRuntime {
    fn dispatch_batch(&mut self, world: &mut World, requests: Vec<ControlRequest>) {
        Python::attach(|_py| {
            for request in requests {
                let result = handlers::dispatch(world, request.operation, self);

                // Inject pause warning into successful responses
                let result = result.map(|mut val| {
                    if let Some(time) =
                        world.get_resource::<bevy::time::Time<bevy::time::Virtual>>()
                        && time.is_paused()
                        && let Some(obj) = val.as_object_mut()
                    {
                        obj.insert("_time_paused".to_string(), serde_json::json!(true));
                    }
                    val
                });

                let _ = request.response_tx.send(result);
            }
        });
    }

    fn execute_python(&mut self, world: &mut World, code: String) -> Result<Value, ControlError> {
        handlers::pyo3::execute::execute_python(world, code)
    }

    fn list_entities(&mut self, world: &mut World) -> Result<Value, ControlError> {
        handlers::pyo3::scene::list_entities(world)
    }

    fn get_entity(&mut self, world: &mut World, entity: EntityRef) -> Result<Value, ControlError> {
        handlers::pyo3::scene::get_entity(world, entity)
    }

    fn list_resources(&mut self, world: &mut World) -> Result<Value, ControlError> {
        handlers::pyo3::scene::list_resources(world)
    }

    fn list_systems(&mut self, world: &mut World) -> Result<Value, ControlError> {
        handlers::pyo3::scene::list_systems(world)
    }

    fn query_entities(
        &mut self,
        world: &mut World,
        with: Vec<String>,
        without: Vec<String>,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::scene::query_entities(world, with, without)
    }

    fn get_component_schema(
        &mut self,
        world: &mut World,
        name: String,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::scene::get_component_schema(world, name)
    }

    fn get_component(
        &mut self,
        world: &mut World,
        entity: EntityRef,
        component: String,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::scene::get_component(world, entity, component)
    }

    fn scene_summary(&mut self, world: &mut World) -> Result<Value, ControlError> {
        handlers::pyo3::scene::scene_summary(world)
    }

    fn get_bounding_box(
        &mut self,
        world: &mut World,
        entity: EntityRef,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::scene::get_bounding_box(world, entity)
    }

    fn debug_registry(&mut self, world: &mut World) -> Result<Value, ControlError> {
        handlers::pyo3::scene::debug_registry(world)
    }

    fn spawn_entity(
        &mut self,
        world: &mut World,
        components: Value,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::mutate::spawn_entity(world, components)
    }

    fn set_component(
        &mut self,
        world: &mut World,
        entity: EntityRef,
        component: String,
        fields: Value,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::mutate::set_component(world, entity, component, fields)
    }

    fn remove_component(
        &mut self,
        world: &mut World,
        entity: EntityRef,
        component: String,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::mutate::remove_component(world, entity, component)
    }

    fn insert_resource(
        &mut self,
        world: &mut World,
        resource_type: String,
        value: Value,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::mutate::insert_resource(world, resource_type, value)
    }

    fn remove_resource(
        &mut self,
        world: &mut World,
        resource_type: String,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::mutate::remove_resource(world, resource_type)
    }

    fn batch_mutate(
        &mut self,
        world: &mut World,
        operations: Vec<Value>,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::mutate::batch_mutate(world, operations)
    }

    fn mutate_asset(
        &mut self,
        world: &mut World,
        entity: EntityRef,
        component: String,
        asset_type: String,
        fields: Value,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::asset::mutate_asset(world, entity, component, asset_type, fields)
    }
}
