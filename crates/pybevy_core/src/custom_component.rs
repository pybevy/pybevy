//! Backend-agnostic custom component registration.
//!
//! This module provides shared infrastructure for registering Python-defined
//! `@component` classes as Bevy components. Interpreter adapters use these
//! functions and provide their own [`PythonObjectDescriptor`] implementation
//! for interpreter-object storage.

use std::collections::HashMap;

use bevy::{
    ecs::{
        component::{ComponentCloneBehavior, ComponentDescriptor, ComponentId, StorageType},
        world::World,
    },
    prelude::Resource,
};

use super::{
    component_layout::{ComponentStorageType, WrapperComponentSchema},
    component_wrapper::WrapperSize,
};

/// Creates a Bevy [`ComponentDescriptor`] for a wrapper-stored custom component.
///
/// Wrapper components store primitive field data as contiguous byte arrays
/// (`ComponentWrapper8/16/32/...`), enabling View API and Numba integration.
/// Each Python component class gets its own [`ComponentId`] even if multiple
/// classes use the same wrapper size.
pub fn create_wrapper_descriptor(name: String, wrapper_size: WrapperSize) -> ComponentDescriptor {
    let layout = wrapper_size.mem_layout();
    // SAFETY: Layout matches one of the ComponentWrapper* types which are all
    // Copy + Default (no drop needed), and the layout is correct for the wrapper size.
    unsafe {
        ComponentDescriptor::new_with_layout(
            name,
            StorageType::Table,
            layout,
            None, // No drop function needed for Copy types
            true, // Mutable - wrapper components can be modified
            ComponentCloneBehavior::Default,
            None,
        )
    }
}

/// Trait for backend-specific Python object storage descriptors.
///
/// When a custom component uses PyObject storage (for non-primitive field types
/// like lists, dicts, or custom classes), the backend needs to provide a
/// [`ComponentDescriptor`] with the correct layout and drop function for its
/// Python object representation.
///
/// Each adapter supplies a descriptor and drop logic appropriate to its
/// interpreter object representation.
pub trait PythonObjectDescriptor {
    /// Create a [`ComponentDescriptor`] for storing Python objects in the ECS.
    fn create(name: String) -> ComponentDescriptor;
}

/// Register a custom component with Bevy's ECS using pre-computed storage type.
///
/// This is the backend-agnostic registration function. The caller determines
/// the storage type (wrapper vs pyobject) using backend-specific introspection,
/// then passes it here. For PyObject storage, a backend-specific descriptor
/// is created via the [`PythonObjectDescriptor`] trait.
///
/// # Type Parameters
/// * `D` - The backend's [`PythonObjectDescriptor`] implementation
///
/// # Returns
/// The [`ComponentId`] assigned by Bevy for this component
pub fn register_custom_component_descriptor<D: PythonObjectDescriptor>(
    world: &mut World,
    name: String,
    storage_type: ComponentStorageType,
) -> ComponentId {
    let descriptor = match storage_type {
        ComponentStorageType::Wrapper(wrapper_size) => {
            create_wrapper_descriptor(name, wrapper_size)
        }
        ComponentStorageType::PyObject => D::create(name),
    };
    world.register_component_with_descriptor(descriptor)
}

/// Neutral registry of custom Python components, shared by both backends.
///
/// Keyed by an interpreter-neutral type identity: a `usize`. Adapters provide
/// a stable type-object identity; storing it as a `usize` (not a raw pointer)
/// keeps this resource `Send`/`Sync` with no `unsafe impl`.
///
/// The storage and schema maps (keyed by [`ComponentId`]) are the **single
/// source of truth** for the registration guard: reuse of a cached
/// `ComponentId` is allowed only when its recorded [`ComponentStorageType`] and
/// wrapper schema still match the live class. See
/// [`register_custom_component_guarded`].
#[derive(Resource, Default)]
pub struct CustomComponentRegistry {
    by_id: HashMap<usize, ComponentId>,
    /// `module.qualname` each type identity was recorded under. A type object
    /// freed while its alias survives leaves the address reusable by an
    /// unrelated class, so the address alone is not an identity.
    alias_names: HashMap<usize, Option<String>>,
    alias_generations: HashMap<usize, u32>,
    by_name: HashMap<String, ComponentId>,
    storage_types: HashMap<ComponentId, ComponentStorageType>,
    wrapper_schemas: HashMap<ComponentId, WrapperComponentSchema>,
}

impl CustomComponentRegistry {
    /// Get the `ComponentId` registered for a type-identity handle, if any
    /// (includes hot-reload pointer aliases).
    pub fn get(&self, type_id: usize) -> Option<ComponentId> {
        self.by_id.get(&type_id).copied()
    }

    /// The full type-id -> `ComponentId` map (aliases included). Used by the
    /// system executor to pre-build the lookup passed to `register_with_world`.
    pub fn ids_by_type(&self) -> &HashMap<usize, ComponentId> {
        &self.by_id
    }

    /// Logical component names and their stable Bevy component IDs.
    ///
    /// Hot-reload aliases are collapsed by full `module.qualname`, so each
    /// logical Python component appears once.
    pub fn ids_by_qualified_name(&self) -> impl Iterator<Item = (&str, ComponentId)> + '_ {
        self.by_name.iter().map(|(name, id)| (name.as_str(), *id))
    }

    /// The storage type a `ComponentId` was registered with, if known.
    pub fn storage_type(&self, id: ComponentId) -> Option<ComponentStorageType> {
        self.storage_types.get(&id).copied()
    }

    /// The stable field schema for a wrapper-stored component.
    pub fn wrapper_schema(&self, id: ComponentId) -> Option<&WrapperComponentSchema> {
        self.wrapper_schemas.get(&id)
    }

    /// Resolve a live logical custom component by its qualified Python name.
    pub fn id_by_qualified_name(&self, name: &str) -> Option<ComponentId> {
        self.by_name.get(name).copied()
    }

    /// Number of interpreter type identities retained for live/rollback reload generations.
    pub fn alias_count(&self) -> usize {
        self.by_id.len()
    }

    /// Remove exact type identities older than the hot-reload rollback window.
    ///
    /// Logical name and storage mappings remain: a later definition of the same
    /// component can still reuse its stable Bevy `ComponentId`.
    pub fn prune_aliases(&mut self, minimum_generation: u32) -> Vec<usize> {
        let removed = self
            .alias_generations
            .iter()
            .filter_map(|(type_id, generation)| {
                (*generation < minimum_generation).then_some(*type_id)
            })
            .collect::<Vec<_>>();
        for type_id in &removed {
            self.by_id.remove(type_id);
            self.alias_names.remove(type_id);
            self.alias_generations.remove(type_id);
        }
        removed
    }
}

/// What a call to [`register_custom_component_guarded`] did, so the caller can
/// keep its adapter-local side tables in sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterOutcome {
    /// The exact type-id was already registered with unchanged storage and schema;
    /// `id` is the cached `ComponentId` and nothing was mutated.
    Reused(ComponentId),
    /// Hot reload: a new type-id was aliased to an existing, name-matched,
    /// storage- and schema-compatible `ComponentId`.
    Aliased(ComponentId),
    /// A fresh `ComponentId` was registered. `evicted` is a prior `ComponentId`
    /// orphaned by a storage- or schema-incompatible name collision, whose backend
    /// side-table entries the caller must drop.
    Registered {
        id: ComponentId,
        evicted: Option<ComponentId>,
    },
}

impl RegisterOutcome {
    /// The `ComponentId` to use for this component, regardless of variant.
    pub fn id(&self) -> ComponentId {
        match *self {
            RegisterOutcome::Reused(id)
            | RegisterOutcome::Aliased(id)
            | RegisterOutcome::Registered { id, .. } => id,
        }
    }
}

/// Register a custom Python component behind the shared storage-flip guard.
///
/// The caller (holding the interpreter handle) precomputes the backend-specific
/// leaves - `type_id` (type-object identity), `qualified_name` (`module.qualname`,
/// for hot-reload aliasing), and `storage_type` (wrapper vs PyObject, recomputed
/// from the *live* class every spawn) - and passes them here. This function owns
/// the interpreter-agnostic orchestration:
///
/// 1. **Fast path / RCE guard.** If this exact `type_id` is cached *and* its
///    recorded storage type and schema are unchanged, reuse the cached `ComponentId`.
///    Otherwise fall through: reusing a cached id across a PyObject<->Wrapper
///    flip (or a wrapper-size change) would write data shaped for the new layout
///    into a column laid out - and dropped - as the old one (type confusion / an
///    out-of-bounds copy).
/// 2. **Hot-reload alias.** If a previous generation registered the same
///    `qualified_name` with compatible storage and field schema, alias the new `type_id`
///    to that `ComponentId` so entities from before the reload stay queryable.
/// 3. **Fresh registration.** Otherwise allocate a new `ComponentId` (evicting a
///    storage-incompatible name collision first) via
///    [`register_custom_component_descriptor`].
pub fn register_custom_component_guarded<D: PythonObjectDescriptor>(
    world: &mut World,
    type_id: usize,
    name: &str,
    qualified_name: Option<&str>,
    storage_type: ComponentStorageType,
    wrapper_schema: Option<&WrapperComponentSchema>,
    generation: u32,
) -> RegisterOutcome {
    if !world.contains_resource::<CustomComponentRegistry>() {
        world.insert_resource(CustomComponentRegistry::default());
    }

    // Fast path: same class object, storage type unchanged -> reuse. The storage
    // type is recomputed from the live class on every spawn, so a mismatch here
    // means untrusted Python flipped `__pybevy_storage__` / `__annotations__` on
    // the same class between spawns; fall through to a fresh registration.
    let cached = world
        .resource::<CustomComponentRegistry>()
        .by_id
        .get(&type_id)
        .copied()
        .filter(|_| {
            world
                .resource::<CustomComponentRegistry>()
                .alias_names
                .get(&type_id)
                .map(|recorded| recorded.as_deref() == qualified_name)
                .unwrap_or(false)
        });
    if let Some(id) = cached {
        let stored = world
            .resource::<CustomComponentRegistry>()
            .storage_types
            .get(&id)
            .copied();
        let stored_schema = world
            .resource::<CustomComponentRegistry>()
            .wrapper_schemas
            .get(&id);
        if stored == Some(storage_type) && stored_schema == wrapper_schema {
            world
                .resource_mut::<CustomComponentRegistry>()
                .alias_generations
                .insert(type_id, generation);
            return RegisterOutcome::Reused(id);
        }
        // Storage flipped on this class object: do NOT reuse `id`. The name-based
        // branch below (qualified_name is stable for a given class) evicts it.
    }

    // Hot-reload path: a previous generation may have registered the same
    // qualified name under a different `ComponentId` (Python re-executes
    // `@component`, minting a new type object each time).
    let mut evicted: Option<ComponentId> = None;
    if let Some(qname) = qualified_name {
        let existing = world
            .resource::<CustomComponentRegistry>()
            .by_name
            .get(qname)
            .copied();
        if let Some(existing_id) = existing {
            let existing_storage = world
                .resource::<CustomComponentRegistry>()
                .storage_types
                .get(&existing_id)
                .copied();
            let existing_schema = world
                .resource::<CustomComponentRegistry>()
                .wrapper_schemas
                .get(&existing_id);
            if existing_storage == Some(storage_type) && existing_schema == wrapper_schema {
                // Storage- and schema-compatible: alias the new type-id onto
                // the existing ComponentId.
                world
                    .resource_mut::<CustomComponentRegistry>()
                    .by_id
                    .insert(type_id, existing_id);
                world
                    .resource_mut::<CustomComponentRegistry>()
                    .alias_names
                    .insert(type_id, qualified_name.map(str::to_owned));
                world
                    .resource_mut::<CustomComponentRegistry>()
                    .alias_generations
                    .insert(type_id, generation);
                return RegisterOutcome::Aliased(existing_id);
            }
            // Storage changed across reload: the old column can't hold the new
            // layout. Orphan the old id and register fresh.
            evicted = Some(existing_id);
        }
    }

    // Drop the orphaned entry from the neutral maps before inserting the fresh
    // one so lookups don't resolve to a column the new id no longer owns. The
    // `by_name` entry (if any) is overwritten below.
    if let Some(stale) = evicted {
        let mut reg = world.resource_mut::<CustomComponentRegistry>();
        reg.storage_types.remove(&stale);
        reg.wrapper_schemas.remove(&stale);
        let stale_type_ids = reg
            .by_id
            .iter()
            .filter_map(|(type_id, id)| (*id == stale).then_some(*type_id))
            .collect::<Vec<_>>();
        reg.by_id.retain(|_, id| *id != stale);
        for type_id in stale_type_ids {
            reg.alias_names.remove(&type_id);
            reg.alias_generations.remove(&type_id);
        }
    }

    let id = register_custom_component_descriptor::<D>(world, name.to_string(), storage_type);

    let mut reg = world.resource_mut::<CustomComponentRegistry>();
    reg.by_id.insert(type_id, id);
    reg.alias_names
        .insert(type_id, qualified_name.map(str::to_owned));
    reg.alias_generations.insert(type_id, generation);
    reg.storage_types.insert(id, storage_type);
    if let Some(schema) = wrapper_schema {
        reg.wrapper_schemas.insert(id, schema.clone());
    }
    if let Some(qname) = qualified_name {
        reg.by_name.insert(qname.to_string(), id);
    }

    RegisterOutcome::Registered { id, evicted }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_layout::{FieldInfo, PrimitiveType, WrapperComponentSchema};

    #[test]
    fn test_create_wrapper_descriptor_w8() {
        let desc = create_wrapper_descriptor("TestW8".to_string(), WrapperSize::W8);
        assert_eq!(desc.storage_type(), StorageType::Table);
    }

    #[test]
    fn test_create_wrapper_descriptor_w32() {
        let desc = create_wrapper_descriptor("TestW32".to_string(), WrapperSize::W32);
        assert_eq!(desc.storage_type(), StorageType::Table);
    }

    #[test]
    fn test_create_wrapper_descriptor_w1024() {
        let desc = create_wrapper_descriptor("TestW1024".to_string(), WrapperSize::W1024);
        assert_eq!(desc.storage_type(), StorageType::Table);
    }

    /// Dummy descriptor for testing the generic registration function
    struct TestObjectDescriptor;

    impl PythonObjectDescriptor for TestObjectDescriptor {
        fn create(name: String) -> ComponentDescriptor {
            // Use a simple u64 layout for testing
            unsafe {
                ComponentDescriptor::new_with_layout(
                    name,
                    StorageType::Table,
                    std::alloc::Layout::new::<u64>(),
                    None,
                    false,
                    ComponentCloneBehavior::Default,
                    None,
                )
            }
        }
    }

    #[test]
    fn test_register_wrapper_component() {
        let mut world = World::new();
        let id = register_custom_component_descriptor::<TestObjectDescriptor>(
            &mut world,
            "WrapperComp".to_string(),
            ComponentStorageType::Wrapper(WrapperSize::W16),
        );
        // Should get a valid ComponentId
        assert!(id.index() > 0);
    }

    #[test]
    fn test_register_pyobject_component() {
        let mut world = World::new();
        let id = register_custom_component_descriptor::<TestObjectDescriptor>(
            &mut world,
            "PyObjComp".to_string(),
            ComponentStorageType::PyObject,
        );
        assert!(id.index() > 0);
    }

    #[test]
    fn test_register_two_components_different_ids() {
        let mut world = World::new();
        let id1 = register_custom_component_descriptor::<TestObjectDescriptor>(
            &mut world,
            "CompA".to_string(),
            ComponentStorageType::Wrapper(WrapperSize::W8),
        );
        let id2 = register_custom_component_descriptor::<TestObjectDescriptor>(
            &mut world,
            "CompB".to_string(),
            ComponentStorageType::Wrapper(WrapperSize::W8),
        );
        // Same wrapper size but different names → different ComponentIds
        assert_ne!(id1, id2);
    }

    fn w8() -> ComponentStorageType {
        ComponentStorageType::Wrapper(WrapperSize::W8)
    }
    fn w32() -> ComponentStorageType {
        ComponentStorageType::Wrapper(WrapperSize::W32)
    }
    fn pyobj() -> ComponentStorageType {
        ComponentStorageType::PyObject
    }

    fn schema(storage: ComponentStorageType) -> Option<WrapperComponentSchema> {
        match storage {
            ComponentStorageType::Wrapper(wrapper_size) => Some(WrapperComponentSchema {
                fields: Vec::new(),
                data_size: 0,
                wrapper_size,
            }),
            ComponentStorageType::PyObject => None,
        }
    }

    fn guarded(
        world: &mut World,
        type_id: usize,
        storage: ComponentStorageType,
    ) -> RegisterOutcome {
        let wrapper_schema = schema(storage);
        register_custom_component_guarded::<TestObjectDescriptor>(
            world,
            type_id,
            "Foo",
            Some("m.Foo"),
            storage,
            wrapper_schema.as_ref(),
            0,
        )
    }

    #[test]
    fn guarded_first_registration_is_fresh() {
        let mut world = World::new();
        let out = guarded(&mut world, 0x1000, w8());
        assert!(matches!(
            out,
            RegisterOutcome::Registered { evicted: None, .. }
        ));
        let reg = world.resource::<CustomComponentRegistry>();
        assert_eq!(reg.get(0x1000), Some(out.id()));
        assert_eq!(reg.storage_type(out.id()), Some(w8()));
    }

    #[test]
    fn guarded_same_ptr_same_storage_reuses() {
        let mut world = World::new();
        let a = guarded(&mut world, 0x1000, w8());
        let b = guarded(&mut world, 0x1000, w8());
        assert_eq!(b, RegisterOutcome::Reused(a.id()));
    }

    #[test]
    fn guarded_storage_flip_rehomes_and_evicts() {
        // Same class object (same type_id + qualified_name), storage flips
        // wrapper -> pyobject: allocate a fresh id and report the old one
        // evicted (the RCE guard). Reusing the cached id would be type confusion.
        let mut world = World::new();
        let a = guarded(&mut world, 0x1000, w8());
        let b = guarded(&mut world, 0x1000, pyobj());
        assert!(matches!(b, RegisterOutcome::Registered { evicted: Some(e), .. } if e == a.id()));
        assert_ne!(a.id(), b.id());
        let reg = world.resource::<CustomComponentRegistry>();
        assert_eq!(reg.get(0x1000), Some(b.id()));
        // The orphaned id's storage entry is dropped.
        assert_eq!(reg.storage_type(a.id()), None);
    }

    #[test]
    fn guarded_hot_reload_pyobject_aliases() {
        // New type object (new type_id), same qualified_name + PyObject storage:
        // alias so pre-reload entities stay queryable.
        let mut world = World::new();
        let a = guarded(&mut world, 0x1000, pyobj());
        let b = guarded(&mut world, 0x2000, pyobj());
        assert_eq!(b, RegisterOutcome::Aliased(a.id()));
        let reg = world.resource::<CustomComponentRegistry>();
        assert_eq!(reg.get(0x2000), Some(a.id()));
        assert_eq!(reg.get(0x1000), Some(a.id()));
    }

    #[test]
    fn guarded_hot_reload_same_wrapper_size_aliases() {
        // A same-size wrapper name-collision aliases: the column layout is
        // identical, so reusing the ComponentId is safe.
        let mut world = World::new();
        let a = guarded(&mut world, 0x1000, w8());
        let b = guarded(&mut world, 0x2000, w8());
        assert_eq!(b, RegisterOutcome::Aliased(a.id()));
    }

    #[test]
    fn guarded_hot_reload_same_size_changed_schema_rehomes() {
        let mut world = World::new();
        let schema_a = WrapperComponentSchema {
            fields: vec![FieldInfo {
                name: "x".to_string(),
                offset: 0,
                field_type: PrimitiveType::F64,
            }],
            data_size: 8,
            wrapper_size: WrapperSize::W8,
        };
        let schema_b = WrapperComponentSchema {
            fields: vec![FieldInfo {
                name: "enabled".to_string(),
                offset: 0,
                field_type: PrimitiveType::Bool,
            }],
            data_size: 1,
            wrapper_size: WrapperSize::W8,
        };
        let a = register_custom_component_guarded::<TestObjectDescriptor>(
            &mut world,
            0x1000,
            "Foo",
            Some("m.Foo"),
            w8(),
            Some(&schema_a),
            0,
        );
        let b = register_custom_component_guarded::<TestObjectDescriptor>(
            &mut world,
            0x2000,
            "Foo",
            Some("m.Foo"),
            w8(),
            Some(&schema_b),
            1,
        );

        assert!(matches!(b, RegisterOutcome::Registered { evicted: Some(id), .. } if id == a.id()));
        assert_ne!(a.id(), b.id());
        assert_eq!(
            world
                .resource::<CustomComponentRegistry>()
                .wrapper_schema(b.id()),
            Some(&schema_b)
        );
    }

    #[test]
    fn guarded_hot_reload_changed_storage_rehomes() {
        // New type object, same qualified_name, storage changed (w8 -> w32):
        // fresh id, evict the old one.
        let mut world = World::new();
        let a = guarded(&mut world, 0x1000, w8());
        let b = guarded(&mut world, 0x2000, w32());
        assert!(matches!(b, RegisterOutcome::Registered { evicted: Some(e), .. } if e == a.id()));
        assert_ne!(a.id(), b.id());
        assert_eq!(
            world.resource::<CustomComponentRegistry>().get(0x2000),
            Some(b.id())
        );
    }

    /// A freed type object leaves its address reusable. Reusing the cached
    /// ComponentId for whatever class lands there next put unrelated components
    /// in one column, so `Query[Foo]` yielded `Bar` instances.
    #[test]
    fn guarded_rejects_a_recycled_address_belonging_to_another_class() {
        let mut world = World::new();
        let foo = register_custom_component_guarded::<TestObjectDescriptor>(
            &mut world,
            0x1000,
            "Foo",
            Some("m.Foo"),
            pyobj(),
            None,
            0,
        );
        // Same address, different class: must not reuse Foo's ComponentId.
        let bar = register_custom_component_guarded::<TestObjectDescriptor>(
            &mut world,
            0x1000,
            "Bar",
            Some("m.Bar"),
            pyobj(),
            None,
            0,
        );
        assert_ne!(foo.id(), bar.id());
        assert_eq!(
            world.resource::<CustomComponentRegistry>().get(0x1000),
            Some(bar.id())
        );
    }

    #[test]
    fn guarded_reuse_still_requires_only_a_matching_name() {
        let mut world = World::new();
        let a = guarded(&mut world, 0x1000, pyobj());
        let b = guarded(&mut world, 0x1000, pyobj());
        assert_eq!(b, RegisterOutcome::Reused(a.id()));
    }

    /// A class with no qualified name keeps the previous address-only reuse:
    /// there is nothing to verify against, and refusing would re-register on
    /// every spawn.
    #[test]
    fn guarded_unnamed_classes_still_reuse_by_address() {
        let mut world = World::new();
        let first = register_custom_component_guarded::<TestObjectDescriptor>(
            &mut world,
            0x1000,
            "Foo",
            None,
            pyobj(),
            None,
            0,
        );
        let second = register_custom_component_guarded::<TestObjectDescriptor>(
            &mut world,
            0x1000,
            "Foo",
            None,
            pyobj(),
            None,
            0,
        );
        assert_eq!(second, RegisterOutcome::Reused(first.id()));
    }

    #[test]
    fn guarded_hot_reload_aliases_are_bounded_by_generation() {
        let mut world = World::new();
        for generation in 0..100 {
            register_custom_component_guarded::<TestObjectDescriptor>(
                &mut world,
                0x1000 + generation as usize,
                "Foo",
                Some("m.Foo"),
                pyobj(),
                None,
                generation,
            );
            world
                .resource_mut::<CustomComponentRegistry>()
                .prune_aliases(generation.saturating_sub(1));
        }

        let registry = world.resource::<CustomComponentRegistry>();
        assert_eq!(registry.alias_count(), 2);
        assert!(registry.get(0x1000 + 98).is_some());
        assert!(registry.get(0x1000 + 99).is_some());
    }
}
