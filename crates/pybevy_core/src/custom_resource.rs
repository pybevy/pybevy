//! Backend-agnostic custom resource registration.
//!
//! This module owns the neutral identity mapping: exact interpreter type
//! identity, full `module.qualname` hot-reload aliases, and the stable Bevy
//! [`ComponentId`] used for scheduler access and resource-entity storage.

use std::collections::{HashMap, HashSet};

use bevy::{
    ecs::{
        change_detection::MaybeLocation,
        component::ComponentId,
        entity::Entity,
        hierarchy::{ChildOf, Children},
        resource::IsResource,
        world::World,
    },
    prelude::Resource,
    ptr::OwningPtr,
};

use crate::{custom_component::PythonObjectDescriptor, public_error};

/// Interpreter-neutral identity for a Python type object.
///
/// Adapters map their stable type-object identity to this integer key.
pub type TypeKey = usize;

/// Insert a dynamically registered resource value through Bevy's resource-entity cache.
///
/// Bevy 0.19 does not expose by-ID required-component registration. Dynamic resource
/// adapters therefore establish the canonical entity with [`IsResource`] before using
/// `World::insert_resource_by_id`. Replacements and reinsertion after removal reuse the
/// same entity through Bevy's cache.
///
/// # Safety
///
/// `component_id` must have been registered in `world` with a descriptor whose layout
/// and drop function exactly match `T`. The component must be reserved for resource use;
/// callers must reject every ordinary entity-component insertion path for the ID.
pub unsafe fn insert_dynamic_resource_value<T>(
    world: &mut World,
    component_id: ComponentId,
    value: T,
) {
    if world.resource_entities().get(component_id).is_none() {
        world.spawn(IsResource::new(component_id));
    }

    OwningPtr::make(value, |ptr| {
        // SAFETY: upheld by this function's caller. The IsResource insertion above
        // establishes the canonical resource entity before Bevy consumes the value.
        unsafe {
            world.insert_resource_by_id(component_id, ptr, MaybeLocation::caller());
        }
    });
}

/// Neutral registry of custom Python resource types.
///
/// No Python objects or backend handles are stored here. Interpreter adapters
/// retain only their class metadata.
#[derive(Resource, Default)]
pub struct CustomResourceRegistry {
    by_type: HashMap<TypeKey, ComponentId>,
    /// `module.qualname` each type identity was recorded under. A type object
    /// freed while its alias survives leaves the address reusable by an
    /// unrelated class, so the address alone is not an identity.
    alias_names: HashMap<TypeKey, Option<String>>,
    alias_generations: HashMap<TypeKey, u32>,
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

    /// Logical resource names and their stable Bevy component IDs.
    ///
    /// Hot-reload aliases are collapsed by full `module.qualname`, so each
    /// logical Python resource appears once.
    pub fn ids_by_qualified_name(&self) -> impl Iterator<Item = (&str, ComponentId)> + '_ {
        self.by_qualified_name
            .iter()
            .map(|(name, id)| (name.as_str(), *id))
    }

    /// Number of interpreter type identities retained for live/rollback generations.
    pub fn alias_count(&self) -> usize {
        self.by_type.len()
    }

    /// Remove exact type identities older than the hot-reload rollback window.
    pub fn prune_aliases(&mut self, minimum_generation: u32) {
        let removed = self
            .alias_generations
            .iter()
            .filter_map(|(type_key, generation)| {
                (*generation < minimum_generation).then_some(*type_key)
            })
            .collect::<Vec<_>>();
        for type_key in removed {
            self.by_type.remove(&type_key);
            self.alias_names.remove(&type_key);
            self.alias_generations.remove(&type_key);
        }
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

/// Whether `entity` is a Bevy resource entity.
///
/// Resource entities are owned by Bevy's resource storage. Despawning one, or
/// stripping its `IsResource` marker, runs the `Discard` hook: the resource
/// value is destroyed with only a warning, so later reads report the resource
/// as absent. Every path that can despawn an entity or remove a component must
/// reject these, on the Python API and the control plane alike.
pub fn is_resource_entity(world: &World, entity: Entity) -> bool {
    world
        .get_entity(entity)
        .is_ok_and(|entity_ref| entity_ref.contains::<IsResource>())
}

/// Whether `root` or any of its descendants is a resource entity.
///
/// Bevy despawns descendants through its own relationship cascade, so a
/// resource entity anywhere under `root` must reject the whole recursive
/// despawn rather than only the root itself.
pub fn hierarchy_contains_resource_entity(world: &World, root: Entity) -> bool {
    let mut pending = vec![root];
    let mut seen = HashSet::new();
    while let Some(entity) = pending.pop() {
        if !seen.insert(entity) || !world.entities().contains(entity) {
            continue;
        }
        if is_resource_entity(world, entity) {
            return true;
        }
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter());
        }
    }
    false
}

/// Reason a requested parent-child relationship is invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HierarchyLinkError {
    ResourceEntity,
    MissingParent(Entity),
    SelfParent,
    Cycle,
}

impl std::fmt::Display for HierarchyLinkError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResourceEntity => formatter.write_str(public_error::RESOURCE_ENTITY_REPARENT),
            Self::MissingParent(entity) => formatter.write_str(
                &public_error::hierarchy_parent_does_not_exist(entity.to_bits()),
            ),
            Self::SelfParent => formatter.write_str(public_error::HIERARCHY_SELF_PARENT),
            Self::Cycle => formatter.write_str(public_error::HIERARCHY_CYCLE),
        }
    }
}

/// Validate a relationship before inserting [`ChildOf`].
pub fn validate_hierarchy_link(
    world: &World,
    child: Entity,
    parent: Entity,
) -> Result<(), HierarchyLinkError> {
    if hierarchy_contains_resource_entity(world, child) || is_resource_entity(world, parent) {
        return Err(HierarchyLinkError::ResourceEntity);
    }
    if world.get_entity(parent).is_err() {
        return Err(HierarchyLinkError::MissingParent(parent));
    }
    if child == parent {
        return Err(HierarchyLinkError::SelfParent);
    }

    let mut ancestor = parent;
    let mut visited = HashSet::new();
    while visited.insert(ancestor) {
        let Some(parent_link) = world.get::<ChildOf>(ancestor) else {
            return Ok(());
        };
        ancestor = parent_link.parent();
        if ancestor == child {
            return Err(HierarchyLinkError::Cycle);
        }
    }

    Err(HierarchyLinkError::Cycle)
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
    generation: u32,
) -> ResourceRegisterOutcome {
    if !world.contains_resource::<CustomResourceRegistry>() {
        world.insert_resource(CustomResourceRegistry::default());
    }

    let registry = world.resource::<CustomResourceRegistry>();
    let cached = registry.get(type_key).filter(|_| {
        registry
            .alias_names
            .get(&type_key)
            .map(|recorded| recorded.as_deref() == qualified_name)
            .unwrap_or(false)
    });
    if let Some(id) = cached {
        world
            .resource_mut::<CustomResourceRegistry>()
            .alias_generations
            .insert(type_key, generation);
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
            world
                .resource_mut::<CustomResourceRegistry>()
                .alias_names
                .insert(type_key, Some(qualified_name.to_owned()));
            world
                .resource_mut::<CustomResourceRegistry>()
                .alias_generations
                .insert(type_key, generation);
            return ResourceRegisterOutcome::Aliased(id);
        }
    }

    let descriptor = D::create(name.to_owned());
    let id = world.register_component_with_descriptor(descriptor);

    let mut registry = world.resource_mut::<CustomResourceRegistry>();
    registry.by_type.insert(type_key, id);
    registry
        .alias_names
        .insert(type_key, qualified_name.map(str::to_owned));
    registry.alias_generations.insert(type_key, generation);
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
            0,
        )
    }

    #[test]
    fn dynamic_resource_value_uses_stable_resource_entity() {
        let mut world = World::new();
        let id = register(&mut world, 1, Some("game.Settings")).id();

        // SAFETY: TestObjectDescriptor describes u64 exactly, and this test uses the
        // registered ID only through the resource insertion helper.
        unsafe { insert_dynamic_resource_value(&mut world, id, 10_u64) };
        let entity = world.resource_entities().get(id).unwrap();
        // SAFETY: TestObjectDescriptor describes u64 exactly.
        assert_eq!(
            unsafe { world.get_resource_by_id(id).unwrap().deref::<u64>() },
            &10
        );
        assert_eq!(
            world
                .entity(entity)
                .get::<IsResource>()
                .unwrap()
                .resource_component_id(),
            id
        );

        assert!(world.remove_resource_by_id(id));
        assert_eq!(world.resource_entities().get(id), Some(entity));

        // SAFETY: same descriptor/value invariant as above.
        unsafe { insert_dynamic_resource_value(&mut world, id, 20_u64) };
        assert_eq!(world.resource_entities().get(id), Some(entity));
        // SAFETY: TestObjectDescriptor describes u64 exactly.
        assert_eq!(
            unsafe { world.get_resource_by_id(id).unwrap().deref::<u64>() },
            &20
        );
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
        let second = register(&mut world, 0x1000, Some("game.Settings"));

        assert_eq!(second, ResourceRegisterOutcome::Reused(first.id()));
        assert_eq!(
            world
                .resource::<CustomResourceRegistry>()
                .ids_by_type()
                .len(),
            1
        );
    }

    /// A type object freed while its entry survives leaves the address reusable
    /// by an unrelated class, so a matching address with a different
    /// `module.qualname` must not inherit the old resource.
    #[test]
    fn recycled_address_with_another_name_does_not_reuse() {
        let mut world = World::new();
        let first = register(&mut world, 0x1000, Some("game.Settings"));
        let second = register(&mut world, 0x1000, Some("other.Unrelated"));

        assert_ne!(first.id(), second.id());
        assert_eq!(
            world.resource::<CustomResourceRegistry>().get(0x1000),
            Some(second.id())
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

    #[test]
    fn hot_reload_aliases_are_bounded_by_generation() {
        let mut world = World::new();
        for generation in 0..100 {
            register_custom_resource_guarded::<TestObjectDescriptor>(
                &mut world,
                0x1000 + generation as usize,
                "Settings",
                Some("game.Settings"),
                generation,
            );
            world
                .resource_mut::<CustomResourceRegistry>()
                .prune_aliases(generation.saturating_sub(1));
        }
        assert_eq!(world.resource::<CustomResourceRegistry>().alias_count(), 2);
    }
}
