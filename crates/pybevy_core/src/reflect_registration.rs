//! Inventory-based TypeRegistry registration for bridged bevy types.
//!
//! Bridge macros submit one `ReflectTypeRegistration` per wrapped bevy type
//! so every exposed type reaches `AppTypeRegistry` even in builds where
//! bevy's `reflect_auto_register` feature is disabled. Types without a
//! `Reflect` derive opt out via the macros' `no_reflect` option.

use bevy::{
    ecs::{reflect::AppTypeRegistry, world::World},
    reflect::TypeRegistry,
};

/// A reflect type registration collected via `inventory`.
pub struct ReflectTypeRegistration {
    /// Registers the bevy type (and its field type dependencies).
    pub register: fn(&mut TypeRegistry),
}

inventory::collect!(ReflectTypeRegistration);

/// Register every bridged bevy type into the world's `AppTypeRegistry`.
///
/// Idempotent, and a no-op when the registry resource is absent. Called at
/// app build so MCP/editor reflection finds wrapped types by name without
/// relying on bevy's `reflect_auto_register`.
pub fn register_wrapped_reflect_types(world: &World) {
    let Some(registry) = world.get_resource::<AppTypeRegistry>() else {
        return;
    };
    let mut registry = registry.write();
    for reg in inventory::iter::<ReflectTypeRegistration> {
        (reg.register)(&mut registry);
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::any::TypeId;

    use bevy::{ecs::world::World, reflect::Reflect};

    use super::*;

    #[derive(Reflect, Default)]
    struct RoutingReflectProbe;

    inventory::submit! {
        ReflectTypeRegistration {
            register: |registry: &mut TypeRegistry| {
                registry.register::<RoutingReflectProbe>();
            },
        }
    }

    #[test]
    fn test_no_op_when_registry_absent() {
        let world = World::new();
        // World::new() has no AppTypeRegistry resource; must return early and
        // not panic.
        register_wrapped_reflect_types(&world);
    }

    #[test]
    fn test_iterates_registrations_when_registry_present() {
        let mut world = World::new();
        world.insert_resource(AppTypeRegistry::default());
        // Reaching the inventory loop must not panic even when the registry is
        // present (the loop body runs for every submitted registration).
        register_wrapped_reflect_types(&world);
    }

    #[test]
    fn wrapped_registration_is_resolvable_by_name_and_idempotent() {
        let mut world = World::new();
        world.insert_resource(AppTypeRegistry::default());

        let before = world.resource::<AppTypeRegistry>().read().iter().count();
        assert!(
            !world
                .resource::<AppTypeRegistry>()
                .read()
                .contains(TypeId::of::<RoutingReflectProbe>())
        );

        register_wrapped_reflect_types(&world);
        {
            let registry = world.resource::<AppTypeRegistry>().read();
            assert!(registry.contains(TypeId::of::<RoutingReflectProbe>()));
            let registration = registry
                .get(TypeId::of::<RoutingReflectProbe>())
                .expect("the submitted reflect type must be registered");
            assert_eq!(
                registration.type_info().type_path_table().short_path(),
                "RoutingReflectProbe"
            );
            assert!(
                registry
                    .get_with_short_type_path("RoutingReflectProbe")
                    .is_some(),
                "MCP/editor name lookups must resolve the wrapped type"
            );
            assert_eq!(
                registry.iter().count(),
                before + 1,
                "the probe adds exactly one registration"
            );
        }

        register_wrapped_reflect_types(&world);
        let registry = world.resource::<AppTypeRegistry>().read();
        assert_eq!(
            registry.iter().count(),
            before + 1,
            "a second call must not duplicate registrations"
        );
        assert!(registry.contains(TypeId::of::<RoutingReflectProbe>()));
    }
}
