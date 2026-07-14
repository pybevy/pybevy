//! Backend-agnostic custom resource registration.
//!
//! Python-defined resources keep their concrete interpreter values in a
//! backend-owned side table. This module owns only the neutral identity mapping:
//! exact interpreter type identity, full `module.qualname` hot-reload aliases,
//! and the stable Bevy [`ComponentId`] used for scheduler access.

use std::collections::HashMap;

use bevy::{
    ecs::{component::ComponentId, world::World},
    prelude::Resource,
};

use crate::custom_component::PythonObjectDescriptor;

/// Interpreter-neutral identity for a Python type object.
///
/// PyO3 uses `PyTypeObject* as usize`; RustPython uses `PyObjectRef::get_id()`.
pub type TypeKey = usize;

/// Neutral registry of custom Python resource types.
///
/// No Python objects or backend handles are stored here. Backends retain their
/// own class metadata and resource-value tables, keyed by the returned
/// [`ComponentId`].
#[derive(Resource, Default)]
pub struct CustomResourceRegistry {
    by_type: HashMap<TypeKey, ComponentId>,
    by_qualified_name: HashMap<String, ComponentId>,
}

impl CustomResourceRegistry {
    /// Look up an exact type identity, including identities added as hot-reload
    /// aliases.
    pub fn get(&self, type_key: TypeKey) -> Option<ComponentId> {
        self.by_type.get(&type_key).copied()
    }

    /// All exact type identities and aliases known to this registry.
    pub fn ids_by_type(&self) -> &HashMap<TypeKey, ComponentId> {
        &self.by_type
    }

    /// Look up a logical resource channel by its full `module.qualname`.
    pub fn id_by_qualified_name(&self, qualified_name: &str) -> Option<ComponentId> {
        self.by_qualified_name.get(qualified_name).copied()
    }
}

/// Result of registering a custom resource type.
///
/// The backend uses this result to synchronize interpreter-specific class
/// metadata after the neutral World-resource borrow has ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceRegisterOutcome {
    /// This exact type identity was already registered.
    Reused(ComponentId),
    /// A new type identity was attached to an existing qualified-name channel.
    Aliased(ComponentId),
    /// A new logical resource channel and [`ComponentId`] were created.
    Registered(ComponentId),
}

impl ResourceRegisterOutcome {
    /// The stable [`ComponentId`] selected by this registration.
    pub fn id(self) -> ComponentId {
        match self {
            Self::Reused(id) | Self::Aliased(id) | Self::Registered(id) => id,
        }
    }
}

/// Register a custom Python resource type using neutral identity semantics.
///
/// Exact type identity is the fast path. A different type identity aliases an
/// existing resource only when its full `module.qualname` matches, preserving
/// values across class redefinition without colliding unrelated classes that
/// merely share a bare `__name__`.
pub fn register_custom_resource_guarded<D: PythonObjectDescriptor>(
    world: &mut World,
    type_key: TypeKey,
    name: &str,
    qualified_name: Option<&str>,
) -> ResourceRegisterOutcome {
    if !world.contains_resource::<CustomResourceRegistry>() {
        world.insert_resource(CustomResourceRegistry::default());
    }

    if let Some(id) = world.resource::<CustomResourceRegistry>().get(type_key) {
        return ResourceRegisterOutcome::Reused(id);
    }

    if let Some(qualified_name) = qualified_name {
        let existing = world
            .resource::<CustomResourceRegistry>()
            .id_by_qualified_name(qualified_name);
        if let Some(id) = existing {
            world
                .resource_mut::<CustomResourceRegistry>()
                .by_type
                .insert(type_key, id);
            return ResourceRegisterOutcome::Aliased(id);
        }
    }

    let descriptor = D::create(name.to_owned());
    let id = world.register_component_with_descriptor(descriptor);

    let mut registry = world.resource_mut::<CustomResourceRegistry>();
    registry.by_type.insert(type_key, id);
    if let Some(qualified_name) = qualified_name {
        registry
            .by_qualified_name
            .insert(qualified_name.to_owned(), id);
    }

    ResourceRegisterOutcome::Registered(id)
}

#[cfg(test)]
mod tests {
    use std::alloc::Layout;

    use bevy::ecs::component::{ComponentCloneBehavior, ComponentDescriptor, StorageType};

    use super::*;

    struct TestObjectDescriptor;

    impl PythonObjectDescriptor for TestObjectDescriptor {
        fn create(name: String) -> ComponentDescriptor {
            // SAFETY: the test descriptor represents a `u64`; its layout is exact,
            // it needs no drop function, and it is never populated in these tests.
            unsafe {
                ComponentDescriptor::new_with_layout(
                    name,
                    StorageType::Table,
                    Layout::new::<u64>(),
                    None,
                    false,
                    ComponentCloneBehavior::Default,
                    None,
                )
            }
        }
    }

    fn register(
        world: &mut World,
        type_key: TypeKey,
        qualified_name: Option<&str>,
    ) -> ResourceRegisterOutcome {
        register_custom_resource_guarded::<TestObjectDescriptor>(
            world,
            type_key,
            "Settings",
            qualified_name,
        )
    }

    #[test]
    fn first_registration_creates_a_channel() {
        let mut world = World::new();
        let outcome = register(&mut world, 0x1000, Some("game.Settings"));

        assert!(matches!(outcome, ResourceRegisterOutcome::Registered(_)));
        let registry = world.resource::<CustomResourceRegistry>();
        assert_eq!(registry.get(0x1000), Some(outcome.id()));
        assert_eq!(
            registry.id_by_qualified_name("game.Settings"),
            Some(outcome.id())
        );
    }

    #[test]
    fn exact_type_identity_is_idempotent() {
        let mut world = World::new();
        let first = register(&mut world, 0x1000, Some("game.Settings"));
        let second = register(&mut world, 0x1000, Some("changed.Name"));

        assert_eq!(second, ResourceRegisterOutcome::Reused(first.id()));
        assert_eq!(
            world
                .resource::<CustomResourceRegistry>()
                .ids_by_type()
                .len(),
            1
        );
    }

    #[test]
    fn redefined_qualified_type_aliases_existing_channel() {
        let mut world = World::new();
        let first = register(&mut world, 0x1000, Some("game.Settings"));
        let second = register(&mut world, 0x2000, Some("game.Settings"));

        assert_eq!(second, ResourceRegisterOutcome::Aliased(first.id()));
        let registry = world.resource::<CustomResourceRegistry>();
        assert_eq!(registry.get(0x1000), Some(first.id()));
        assert_eq!(registry.get(0x2000), Some(first.id()));
    }

    #[test]
    fn same_bare_name_in_different_modules_does_not_alias() {
        let mut world = World::new();
        let first = register(&mut world, 0x1000, Some("alpha.Settings"));
        let second = register(&mut world, 0x2000, Some("beta.Settings"));

        assert!(matches!(second, ResourceRegisterOutcome::Registered(_)));
        assert_ne!(first.id(), second.id());
    }

    #[test]
    fn missing_qualified_names_do_not_alias() {
        let mut world = World::new();
        let first = register(&mut world, 0x1000, None);
        let second = register(&mut world, 0x2000, None);

        assert_ne!(first.id(), second.id());
    }
}
