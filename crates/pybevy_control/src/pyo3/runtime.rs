use bevy::ecs::{entity::Entity, world::World};
use pyo3::Python;
use serde_json::Value;

use super::{assets, batch, components, execute, inspect, resources, schema};
use crate::{
    engine::{
        channel::ControlRequest,
        dispatch::dispatch,
        runtime::{ComponentCatalog, ComponentFilter, ControlRuntime, EntityComponentNames},
    },
    protocol::{
        ControlError, EntityRef, GetComponentParams, GetResourceParams, RemoveComponentParams,
        SetAssetParams, SetComponentParams, SetResourceParams,
    },
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
                let result = dispatch(world, request.operation, self);

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
        execute::execute_python(world, code)
    }

    fn component_names(&mut self, world: &World, entity: Entity) -> EntityComponentNames {
        inspect::component_names(world, entity)
    }

    fn resolve_component_filter(
        &mut self,
        world: &World,
        name: &str,
    ) -> Result<Option<ComponentFilter>, ControlError> {
        inspect::resolve_component_filter(world, name)
    }

    fn component_catalog(&mut self, world: &World) -> ComponentCatalog {
        inspect::component_catalog(world)
    }

    fn resource_names(&mut self, world: &mut World) -> Result<Vec<String>, ControlError> {
        Ok(resources::resource_names(world))
    }

    fn get_entity(&mut self, world: &mut World, entity: EntityRef) -> Result<Value, ControlError> {
        inspect::get_entity(world, entity)
    }

    fn list_resources(&mut self, world: &mut World) -> Result<Value, ControlError> {
        resources::list_resources(world)
    }

    fn get_component_schema(
        &mut self,
        world: &mut World,
        name: String,
    ) -> Result<Value, ControlError> {
        schema::get_component_schema(world, name)
    }

    fn get_component(
        &mut self,
        world: &mut World,
        params: GetComponentParams,
    ) -> Result<Value, ControlError> {
        inspect::get_component(world, params.entity, params.component)
    }

    fn get_resource(
        &mut self,
        world: &mut World,
        params: GetResourceParams,
    ) -> Result<Value, ControlError> {
        resources::get_resource(world, params.resource_type)
    }

    fn spawn_entity(
        &mut self,
        world: &mut World,
        components: Value,
    ) -> Result<Value, ControlError> {
        components::spawn_entity(world, components)
    }

    fn set_component(
        &mut self,
        world: &mut World,
        params: SetComponentParams,
    ) -> Result<Value, ControlError> {
        components::set_component(world, params.entity, params.component, params.fields)
    }

    fn remove_component(
        &mut self,
        world: &mut World,
        params: RemoveComponentParams,
    ) -> Result<Value, ControlError> {
        components::remove_component(world, params.entity, params.component)
    }

    fn insert_resource(
        &mut self,
        world: &mut World,
        params: SetResourceParams,
    ) -> Result<Value, ControlError> {
        resources::insert_resource(world, params.resource_type, params.value)
    }

    fn remove_resource(
        &mut self,
        world: &mut World,
        resource_type: String,
    ) -> Result<Value, ControlError> {
        resources::remove_resource(world, resource_type)
    }

    fn batch_mutate(
        &mut self,
        world: &mut World,
        operations: Vec<Value>,
    ) -> Result<Value, ControlError> {
        batch::batch_mutate(world, operations)
    }

    fn mutate_asset(
        &mut self,
        world: &mut World,
        params: SetAssetParams,
    ) -> Result<Value, ControlError> {
        assets::mutate_asset(
            world,
            params.entity,
            params.component,
            params.asset_type,
            params.fields,
        )
    }
}
