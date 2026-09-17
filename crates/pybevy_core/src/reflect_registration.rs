//! Inventory-based TypeRegistry registration for bridged bevy types.
//!
//! Bridge macros submit one `ReflectTypeRegistration` per wrapped bevy type
//! so every exposed type reaches `AppTypeRegistry` even in builds where
//! bevy's `reflect_auto_register` feature is disabled. Types without a
//! `Reflect` derive opt out via the macros' `no_reflect` option.

use std::{any::TypeId, sync::OnceLock};

use bevy::{
    app::App,
    ecs::{reflect::AppTypeRegistry, world::World},
    reflect::TypeRegistry,
};

/// A reflect type registration collected via `inventory`.
pub struct ReflectTypeRegistration {
    /// Registers the bevy type (and its field type dependencies).
    pub register: fn(&mut TypeRegistry),
    /// The one type `register` adds when it only calls
    /// `TypeRegistry::register` on it, which is a no-op once that type is
    /// present. `None` when `register` does anything else, so it always runs.
    pub registers_only: Option<fn() -> TypeId>,
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

/// Whether `registration` can still change a registry that already holds
/// `present`. Hand-written entries always can; a single-type entry cannot
/// once its type is present, because `TypeRegistry::register` returns early.
fn still_applies(registration: &ReflectTypeRegistration, present: Option<&TypeRegistry>) -> bool {
    match (registration.registers_only, present) {
        (Some(type_id), Some(present)) => !present.contains(type_id()),
        _ => true,
    }
}

/// Registrations that can still change the registry a fresh `App::new()`
/// carries. The inventory and `App::new()` are deterministic per process,
/// so this is computed once.
fn registrations_beyond_new_app() -> &'static [fn(&mut TypeRegistry)] {
    static EFFECTIVE: OnceLock<Vec<fn(&mut TypeRegistry)>> = OnceLock::new();
    EFFECTIVE.get_or_init(|| {
        let app = App::new();
        let present = app
            .world()
            .get_resource::<AppTypeRegistry>()
            .map(|registry| registry.read());
        inventory::iter::<ReflectTypeRegistration>
            .into_iter()
            .filter(|registration| still_applies(registration, present.as_deref()))
            .map(|registration| registration.register)
            .collect()
    })
}

/// [`register_wrapped_reflect_types`] for a World whose `AppTypeRegistry`
/// still holds exactly what `App::new()` installed.
///
/// Registrations that only repeat types `App::new()` already registered are
/// skipped, which leaves the registry identical to the full registration.
pub fn register_wrapped_reflect_types_for_new_app(world: &World) {
    let Some(registry) = world.get_resource::<AppTypeRegistry>() else {
        return;
    };
    let mut registry = registry.write();
    for register in registrations_beyond_new_app() {
        register(&mut registry);
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
            registers_only: Some(|| TypeId::of::<RoutingReflectProbe>()),
        }
    }

    #[derive(Reflect, Default)]
    #[reflect(no_auto_register)]
    struct ManualReflectProbe;

    inventory::submit! {
        ReflectTypeRegistration {
            register: |registry: &mut TypeRegistry| {
                registry.register::<ManualReflectProbe>();
            },
            registers_only: Some(|| TypeId::of::<ManualReflectProbe>()),
        }
    }

    /// Type data a hand-written registration installs on a type that may
    /// already be registered, the shape of the world-serialization materializer.
    #[derive(Clone)]
    struct ProbeMarker;

    fn install_probe_marker(registry: &mut TypeRegistry) {
        registry.register::<RoutingReflectProbe>();
        registry
            .get_mut(TypeId::of::<RoutingReflectProbe>())
            .expect("the routing probe was just registered")
            .insert(ProbeMarker);
    }

    inventory::submit! {
        ReflectTypeRegistration {
            register: install_probe_marker,
            registers_only: None,
        }
    }

    #[test]
    fn single_type_entries_are_skipped_only_once_their_type_is_present() {
        let routing = ReflectTypeRegistration {
            register: |registry| registry.register::<RoutingReflectProbe>(),
            registers_only: Some(|| TypeId::of::<RoutingReflectProbe>()),
        };
        let manual = ReflectTypeRegistration {
            register: |registry| registry.register::<ManualReflectProbe>(),
            registers_only: Some(|| TypeId::of::<ManualReflectProbe>()),
        };
        let hand_written = ReflectTypeRegistration {
            register: install_probe_marker,
            registers_only: None,
        };

        let mut present = TypeRegistry::new();
        present.register::<RoutingReflectProbe>();

        assert!(!still_applies(&routing, Some(&present)));
        assert!(still_applies(&manual, Some(&present)));
        assert!(still_applies(&hand_written, Some(&present)));
        assert!(still_applies(&routing, None));
        assert!(still_applies(&manual, None));
        assert!(still_applies(&hand_written, None));
    }

    fn registered_type_ids(world: &World) -> Vec<TypeId> {
        let registry = world.resource::<AppTypeRegistry>().read();
        let mut ids: Vec<TypeId> = registry.iter().map(|reg| reg.type_id()).collect();
        ids.sort();
        ids
    }

    #[test]
    fn new_app_fast_path_matches_full_registration() {
        let full = App::new();
        register_wrapped_reflect_types(full.world());

        let fast = App::new();
        register_wrapped_reflect_types_for_new_app(fast.world());

        let untouched = App::new();
        let full_ids = registered_type_ids(full.world());
        assert_eq!(registered_type_ids(fast.world()), full_ids);
        assert!(full_ids.contains(&TypeId::of::<RoutingReflectProbe>()));
        assert!(full_ids.contains(&TypeId::of::<ManualReflectProbe>()));
        assert!(
            !registered_type_ids(untouched.world()).contains(&TypeId::of::<ManualReflectProbe>()),
            "the manual probe must be absent from App::new() for this test to prove parity"
        );
        for (label, app) in [("full", &full), ("fast", &fast)] {
            let registry = app.world().resource::<AppTypeRegistry>().read();
            assert!(
                registry
                    .get(TypeId::of::<RoutingReflectProbe>())
                    .is_some_and(|registration| registration.contains::<ProbeMarker>()),
                "{label} registration must run hand-written entries on auto-registered types"
            );
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
                before + 2,
                "the two probe types add exactly two registrations"
            );
        }

        register_wrapped_reflect_types(&world);
        let registry = world.resource::<AppTypeRegistry>().read();
        assert_eq!(
            registry.iter().count(),
            before + 2,
            "a second call must not duplicate registrations"
        );
        assert!(registry.contains(TypeId::of::<RoutingReflectProbe>()));
    }
}
