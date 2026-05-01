use bevy::ecs::world::World;
use pyo3::Python;
use serde_json::Value;

use crate::{
    bridge::{
        ControlError, ControlRequest, EntityRef, GetComponentParams, QueryEntitiesParams,
        RemoveComponentParams, SetAssetParams, SetComponentParams, SetResourceParams,
    },
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
        params: QueryEntitiesParams,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::scene::query_entities(world, params.with, params.without)
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
        params: GetComponentParams,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::scene::get_component(world, params.entity, params.component)
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
        params: SetComponentParams,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::mutate::set_component(world, params.entity, params.component, params.fields)
    }

    fn remove_component(
        &mut self,
        world: &mut World,
        params: RemoveComponentParams,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::mutate::remove_component(world, params.entity, params.component)
    }

    fn insert_resource(
        &mut self,
        world: &mut World,
        params: SetResourceParams,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::mutate::insert_resource(world, params.resource_type, params.value)
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
        params: SetAssetParams,
    ) -> Result<Value, ControlError> {
        handlers::pyo3::asset::mutate_asset(
            world,
            params.entity,
            params.component,
            params.asset_type,
            params.fields,
        )
    }
}
