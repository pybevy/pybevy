use bevy::ecs::{
    entity::Entity,
    world::{EntityRef as BevyEntityRef, World},
};
use serde_json::Value;

use crate::{
    engine::channel::ControlRequest,
    protocol::{
        ControlError, EntityRef, GetComponentParams, GetResourceParams, RemoveComponentParams,
        SetAssetParams, SetComponentParams, SetResourceParams,
    },
};

/// Trait for runtime-specific operations in the control server.
///
/// The control poll system and dispatch logic are runtime-agnostic.
/// This trait captures every operation that needs the interpreter.
pub trait ControlRuntime: 'static {
    /// Process a batch of sync requests within the runtime's scope.
    fn dispatch_batch(&mut self, world: &mut World, requests: Vec<ControlRequest>);

    fn execute_python(&mut self, world: &mut World, code: String) -> Result<Value, ControlError>;

    /// Names of the registered components present on `entity`.
    fn component_names(&mut self, world: &World, entity: Entity) -> EntityComponentNames;

    /// Resolves a user-facing component name; `None` when no component has it.
    fn resolve_component_filter(
        &mut self,
        world: &World,
        name: &str,
    ) -> Result<Option<ComponentFilter>, ControlError>;

    /// The registered component and resource types.
    fn component_catalog(&mut self, world: &World) -> ComponentCatalog;

    /// Exactly the names `get_resource` accepts, without reading resource values.
    fn resource_names(&mut self, world: &mut World) -> Result<Vec<String>, ControlError>;

    fn get_entity(&mut self, world: &mut World, entity: EntityRef) -> Result<Value, ControlError>;

    fn list_resources(&mut self, world: &mut World) -> Result<Value, ControlError>;

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

    fn get_resource(
        &mut self,
        world: &mut World,
        params: GetResourceParams,
    ) -> Result<Value, ControlError>;

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

/// Registered component names present on one entity.
#[derive(Debug, Default, Clone)]
pub struct EntityComponentNames {
    pub native: Vec<String>,
    pub custom: Vec<String>,
}

/// A component resolved from a user-facing name, testable against entities.
pub struct ComponentFilter {
    /// Whether the filter selects `Disabled`, so disabled entities are queried.
    pub selects_disabled: bool,
    matches: Box<dyn Fn(&BevyEntityRef) -> bool>,
}

impl ComponentFilter {
    pub fn new(selects_disabled: bool, matches: impl Fn(&BevyEntityRef) -> bool + 'static) -> Self {
        Self {
            selects_disabled,
            matches: Box::new(matches),
        }
    }

    pub fn matches(&self, entity: &BevyEntityRef) -> bool {
        (self.matches)(entity)
    }
}

/// Registered types, for `get_registry`.
#[derive(Debug, Default, Clone)]
pub struct ComponentCatalog {
    pub native_names: Vec<String>,
    /// Custom component names; duplicates are preserved.
    pub custom_names: Vec<String>,
    pub custom_qualified_names: Vec<String>,
    pub resource_type_count: usize,
}

#[cfg(test)]
pub(crate) mod fake {
    use bevy::ecs::{component::Component, entity_disabling::Disabled};

    use super::*;
    use crate::engine::dispatch::dispatch;

    #[derive(Component)]
    pub(crate) struct Crate;

    #[derive(Component)]
    pub(crate) struct Enemy;

    /// An interpreter-free runtime: `Crate` is a native component and `Enemy` a custom one.
    pub(crate) struct FakeRuntime;

    fn unsupported() -> Result<Value, ControlError> {
        Err(ControlError::internal("unsupported by FakeRuntime"))
    }

    impl ControlRuntime for FakeRuntime {
        fn dispatch_batch(&mut self, world: &mut World, requests: Vec<ControlRequest>) {
            for request in requests {
                let _ = request
                    .response_tx
                    .send(dispatch(world, request.operation, self));
            }
        }

        fn execute_python(&mut self, _: &mut World, _: String) -> Result<Value, ControlError> {
            unsupported()
        }

        fn component_names(&mut self, world: &World, entity: Entity) -> EntityComponentNames {
            let Ok(entity) = world.get_entity(entity) else {
                return EntityComponentNames::default();
            };
            let named = |present: bool, name: &str| {
                present
                    .then(|| name.to_string())
                    .into_iter()
                    .collect::<Vec<_>>()
            };
            EntityComponentNames {
                native: named(entity.contains::<Crate>(), "Crate"),
                custom: named(entity.contains::<Enemy>(), "Enemy"),
            }
        }

        fn resolve_component_filter(
            &mut self,
            _: &World,
            name: &str,
        ) -> Result<Option<ComponentFilter>, ControlError> {
            Ok(match name {
                "Crate" => Some(ComponentFilter::new(false, |entity| {
                    entity.contains::<Crate>()
                })),
                "Enemy" => Some(ComponentFilter::new(false, |entity| {
                    entity.contains::<Enemy>()
                })),
                "Disabled" => Some(ComponentFilter::new(true, |entity| {
                    entity.contains::<Disabled>()
                })),
                _ => None,
            })
        }

        fn component_catalog(&mut self, _: &World) -> ComponentCatalog {
            ComponentCatalog {
                native_names: vec!["Crate".into()],
                custom_names: vec!["Enemy".into()],
                custom_qualified_names: vec!["game.Enemy".into()],
                resource_type_count: 3,
            }
        }

        fn resource_names(&mut self, _: &mut World) -> Result<Vec<String>, ControlError> {
            Ok(vec!["Score".into(), "Clock".into(), "Score".into()])
        }

        fn get_entity(&mut self, _: &mut World, _: EntityRef) -> Result<Value, ControlError> {
            unsupported()
        }

        fn list_resources(&mut self, _: &mut World) -> Result<Value, ControlError> {
            unsupported()
        }

        fn get_component_schema(
            &mut self,
            _: &mut World,
            _: String,
        ) -> Result<Value, ControlError> {
            unsupported()
        }

        fn get_component(
            &mut self,
            _: &mut World,
            _: GetComponentParams,
        ) -> Result<Value, ControlError> {
            unsupported()
        }

        fn get_resource(
            &mut self,
            _: &mut World,
            _: GetResourceParams,
        ) -> Result<Value, ControlError> {
            unsupported()
        }

        fn spawn_entity(&mut self, _: &mut World, _: Value) -> Result<Value, ControlError> {
            unsupported()
        }

        fn set_component(
            &mut self,
            _: &mut World,
            _: SetComponentParams,
        ) -> Result<Value, ControlError> {
            unsupported()
        }

        fn remove_component(
            &mut self,
            _: &mut World,
            _: RemoveComponentParams,
        ) -> Result<Value, ControlError> {
            unsupported()
        }

        fn insert_resource(
            &mut self,
            _: &mut World,
            _: SetResourceParams,
        ) -> Result<Value, ControlError> {
            unsupported()
        }

        fn remove_resource(&mut self, _: &mut World, _: String) -> Result<Value, ControlError> {
            unsupported()
        }

        fn batch_mutate(&mut self, _: &mut World, _: Vec<Value>) -> Result<Value, ControlError> {
            unsupported()
        }

        fn mutate_asset(
            &mut self,
            _: &mut World,
            _: SetAssetParams,
        ) -> Result<Value, ControlError> {
            unsupported()
        }
    }
}
