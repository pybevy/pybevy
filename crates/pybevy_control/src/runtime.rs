use bevy::ecs::world::World;
use serde_json::Value;

use crate::bridge::{
    ControlError, ControlRequest, EntityRef, GetComponentParams, QueryEntitiesParams,
    RemoveComponentParams, SetAssetParams, SetComponentParams, SetResourceParams,
};

/// Trait for runtime-specific operations in the control server.
///
/// The control poll system and dispatch logic are runtime-agnostic.
/// This trait captures every operation that needs the interpreter.
pub trait ControlRuntime: 'static {
    /// Process a batch of sync requests within the runtime's scope.
    fn dispatch_batch(&mut self, world: &mut World, requests: Vec<ControlRequest>);

    fn execute_python(&mut self, world: &mut World, code: String) -> Result<Value, ControlError>;

    fn list_entities(&mut self, world: &mut World) -> Result<Value, ControlError>;

    fn get_entity(&mut self, world: &mut World, entity: EntityRef) -> Result<Value, ControlError>;

    fn list_resources(&mut self, world: &mut World) -> Result<Value, ControlError>;

    fn list_systems(&mut self, world: &mut World) -> Result<Value, ControlError>;

    fn query_entities(
        &mut self,
        world: &mut World,
        params: QueryEntitiesParams,
    ) -> Result<Value, ControlError>;

    fn get_component_schema(
        &mut self,
        world: &mut World,
        name: String,
    ) -> Result<Value, ControlError>;

    fn get_component(
        &mut self,
        world: &mut World,
        params: GetComponentParams,
    ) -> Result<Value, ControlError>;

    fn scene_summary(&mut self, world: &mut World) -> Result<Value, ControlError>;

    fn get_bounding_box(
        &mut self,
        world: &mut World,
        entity: EntityRef,
    ) -> Result<Value, ControlError>;

    fn debug_registry(&mut self, world: &mut World) -> Result<Value, ControlError>;

    fn spawn_entity(&mut self, world: &mut World, components: Value)
    -> Result<Value, ControlError>;

    fn set_component(
        &mut self,
        world: &mut World,
        params: SetComponentParams,
    ) -> Result<Value, ControlError>;

    fn remove_component(
        &mut self,
        world: &mut World,
        params: RemoveComponentParams,
    ) -> Result<Value, ControlError>;

    fn insert_resource(
        &mut self,
        world: &mut World,
        params: SetResourceParams,
    ) -> Result<Value, ControlError>;

    fn remove_resource(
        &mut self,
        world: &mut World,
        resource_type: String,
    ) -> Result<Value, ControlError>;

    fn batch_mutate(
        &mut self,
        world: &mut World,
        operations: Vec<Value>,
    ) -> Result<Value, ControlError>;

    fn mutate_asset(
        &mut self,
        world: &mut World,
        params: SetAssetParams,
    ) -> Result<Value, ControlError>;
}
