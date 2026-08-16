//! Core storage primitives and runtime registries for PyBevy
//!
//! This crate provides the foundational types for PyBevy's owned/borrowed pattern
//! and the runtime type registry system that enables crate splitting.
//!
//! ## Storage Primitives
//!
//! - `ValidityFlag` / `ValidityFlagWithMode` - Runtime validity tracking
//! - `ValidityGuard` - RAII guard for system execution scope
//! - `ValueStorage<T>` - Generic storage for Copy types (Vec3, Quat, etc.)
//! - `FieldStorage<T>` - Generic storage for non-Copy types (TextureAtlas, etc.)
//! - `BorrowableStorage` / `FromBorrowedStorage` - Traits for borrowed field access
//!
//! ## Runtime Registries
//!
//! - `ComponentBridge` - Trait for component type bridges
//! - `AssetBridge` - Trait for asset type bridges
//! - `PluginBridge` - Trait for plugin type bridges
//! - `global_registry` - Static registries for all bridge types
//!
//! The registry system allows feature crates (pybevy_audio, pybevy_light, etc.)
//! to register their types without the core crate needing to import them at
//! compile time, enabling independent compilation and faster incremental builds.

pub mod added_plugins;
pub mod asset;
pub mod asset_access;
pub mod asset_cleanup;
pub mod asset_path;
pub mod component;
pub mod component_batch;
pub mod component_layout;
pub mod component_wrapper;
pub mod content_hash;
pub mod custom_component;
pub mod custom_resource;
pub mod debug_snapshot;
pub mod duration;
pub mod entity;
pub mod handle;
pub mod hierarchy;
pub extern crate inventory;
pub mod borrowed_array_anchor;
pub mod bridge_inventory;
pub mod live_sequence;
pub mod logical_type;
pub mod materializable;
pub mod message;
pub mod numpy_view_guard;
pub mod plugin;
pub mod public_error;
pub mod reflect_registration;
pub mod registry;
pub mod reload_request;
pub mod resource;
pub mod source_location;

// Storage layer — re-exported from pybevy_storage
pub use pybevy_storage::{
    batch_columns, field_storage, pyasset, pycomponent, pyresource, storage_error, storage_traits,
    validity_guard, value_storage, view_bridge,
};

#[pyclass(name = "_FloatLiveList", skip_from_py_object)]
#[derive(Clone)]
pub struct PyFloatLiveList {
    storage: FieldStorage<Vec<f32>>,
}

impl_live_scalar_list!(PyFloatLiveList, "_FloatLiveList", Vec<f32>, f32);

use std::any::TypeId;

use bevy::ecs::{
    component::{Component, ComponentId},
    entity::Entity,
    hierarchy::{ChildOf, Children},
    world::{EntityRef, EntityWorldMut, World},
};
use pyo3::{
    PyTypeInfo,
    exceptions::PyRuntimeError,
    ffi::PyTypeObject,
    prelude::*,
    types::{PyList, PyType},
};

/// Build a re-resolving [`ComponentStorage<B>`] for a native/bridge component reached
/// from `world.get`/`world.get_mut`, or `None` if the entity or component is absent.
///
/// Shared by the revalidating `ComponentBridge` extractors so the world-pointer
/// dereference and presence check live in one place.
///
/// # Safety
/// `world_ptr` must be valid for a shared borrow and free of a competing mutable borrow
/// for this call, and must stay valid while `validity` is active.
pub unsafe fn resolve_revalidating_component<B: Component>(
    entity_id: Entity,
    world_ptr: *mut World,
    validity: ValidityFlagWithMode,
) -> Option<ComponentStorage<B>> {
    // SAFETY: forwarded from this function's contract.
    let world = unsafe { &*world_ptr };
    let component_id = world.component_id::<B>()?;
    let entity_ref = world.get_entity(entity_id).ok()?;
    entity_ref.get_by_id(component_id).ok()?;
    // SAFETY: identity verified above; the handle re-resolves the address per access.
    Some(unsafe { ComponentStorage::revalidating(world_ptr, entity_id, component_id, validity) })
}

/// Resolve an [`EntityRef`] from a raw world pointer, or `None` if the entity is gone.
///
/// Shared by the owned-copy bridge extractors so the world-pointer dereference lives in
/// one place.
///
/// # Safety
/// `world_ptr` must be valid for a shared borrow for the returned reference's lifetime.
pub unsafe fn entity_ref_from_ptr<'w>(
    entity_id: Entity,
    world_ptr: *mut World,
) -> Option<EntityRef<'w>> {
    // SAFETY: forwarded from this function's contract.
    let world = unsafe { &*world_ptr };
    world.get_entity(entity_id).ok()
}

// Manual implementation of ChildOfBridge because #[pycomponent(..., bridge)]
// uses pybevy_core:: paths which don't work inside pybevy_core itself.
pub struct ChildOfBridge;

impl ComponentBridge for ChildOfBridge {
    fn bevy_type_id(&self) -> TypeId {
        TypeId::of::<ChildOf>()
    }

    fn py_type_ptr(&self) -> *const PyTypeObject {
        Python::attach(|py| <hierarchy::PyChildOf as PyTypeInfo>::type_object(py).as_type_ptr())
    }

    fn py_type<'py>(&self, py: Python<'py>) -> Bound<'py, PyType> {
        <hierarchy::PyChildOf as PyTypeInfo>::type_object(py)
    }

    fn name(&self) -> &'static str {
        "ChildOf"
    }

    fn register(&self, world: &mut World) -> ComponentId {
        world.register_component::<ChildOf>()
    }

    #[inline(always)]
    fn extract(
        &self,
        entity: &mut FilteredEntityAccess,
        component_id: ComponentId,
        _validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Py<PyAny>> {
        // ChildOf is an immutable component (relation), so we use get_by_id for read-only access
        // and return an owned copy rather than a borrowed reference.
        let ptr = entity
            .get_by_id(component_id)
            .ok_or_else(|| PyRuntimeError::new_err("ChildOf not found"))?;

        let component = unsafe { ptr.deref::<ChildOf>() };
        let py_component = hierarchy::PyChildOf::try_from(component)?;
        let obj = Py::new(py, (py_component, PyComponent))?;
        Ok(obj.into_any())
    }

    fn insert(&self, world: &mut World, entity: Entity, component: &Bound<PyAny>) -> PyResult<()> {
        let py_component = component.extract::<PyRef<hierarchy::PyChildOf>>()?;
        let native: ChildOf = py_component.storage.as_ref()?.clone();

        world.entity_mut(entity).insert(native);
        Ok(())
    }

    fn prepare_uniform(
        &self,
        component: &Bound<PyAny>,
    ) -> PyResult<Box<dyn PreparedUniformComponent>> {
        let py_component = component.extract::<PyRef<hierarchy::PyChildOf>>()?;
        let native: ChildOf = py_component.storage.as_ref()?.clone();
        Ok(Box::new(PreparedNativeUniform::new(native)))
    }

    fn insert_into_entity(
        &self,
        entity: &mut EntityWorldMut,
        component: &Bound<PyAny>,
    ) -> PyResult<()> {
        let py_component = component.extract::<PyRef<hierarchy::PyChildOf>>()?;
        let native: ChildOf = py_component.storage.as_ref()?.clone();

        entity.insert(native);
        Ok(())
    }

    fn extract_fn(&self) -> ExtractFn {
        #[inline(always)]
        fn extract_impl(
            entity: &mut FilteredEntityAccess,
            component_id: ComponentId,
            _validity: ValidityFlagWithMode,
            py: Python,
        ) -> PyResult<Py<PyAny>> {
            // ChildOf is an immutable component (relation), so we use get_by_id for read-only access
            // and return an owned copy rather than a borrowed reference.
            let ptr = entity
                .get_by_id(component_id)
                .ok_or_else(|| PyRuntimeError::new_err("ChildOf not found"))?;

            let component = unsafe { ptr.deref::<ChildOf>() };
            let py_component = crate::hierarchy::PyChildOf::try_from(component)?;
            let obj = Py::new(py, (py_component, crate::component::PyComponent))?;
            Ok(obj.into_any())
        }
        extract_impl
    }

    fn entity_contains(&self, entity: &EntityRef) -> bool {
        entity.contains::<ChildOf>()
    }

    unsafe fn extract_from_entity_ref(
        &self,
        entity_id: Entity,
        world_ptr: *mut World,
        _validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>> {
        // ChildOf is an immutable relation; return an owned copy (no borrow to dangle).
        // SAFETY: world_ptr is valid for a shared borrow here (caller contract).
        let Some(entity_ref) = (unsafe { entity_ref_from_ptr(entity_id, world_ptr) }) else {
            return Ok(None);
        };
        if let Some(component) = entity_ref.get::<ChildOf>() {
            let py_component = hierarchy::PyChildOf::try_from(component)?;
            let obj = Py::new(py, (py_component, PyComponent))?;
            Ok(Some(obj.into_any()))
        } else {
            Ok(None)
        }
    }

    unsafe fn extract_from_entity_mut(
        &self,
        entity_id: Entity,
        world_ptr: *mut World,
        _validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>> {
        // ChildOf is an immutable relation; return an owned copy instead of a borrow.
        // SAFETY: world_ptr is valid for a shared borrow here (caller contract).
        let Some(entity_ref) = (unsafe { entity_ref_from_ptr(entity_id, world_ptr) }) else {
            return Ok(None);
        };
        if let Some(component) = entity_ref.get::<ChildOf>() {
            let py_component = hierarchy::PyChildOf::try_from(component)?;
            let obj = Py::new(py, (py_component, PyComponent))?;
            Ok(Some(obj.into_any()))
        } else {
            Ok(None)
        }
    }
}

pub struct ChildrenBridge;

impl ComponentBridge for ChildrenBridge {
    fn bevy_type_id(&self) -> TypeId {
        TypeId::of::<Children>()
    }

    fn py_type_ptr(&self) -> *const PyTypeObject {
        Python::attach(|py| <hierarchy::PyChildren as PyTypeInfo>::type_object(py).as_type_ptr())
    }

    fn py_type<'py>(&self, py: Python<'py>) -> Bound<'py, PyType> {
        <hierarchy::PyChildren as PyTypeInfo>::type_object(py)
    }

    fn name(&self) -> &'static str {
        "Children"
    }

    fn register(&self, world: &mut World) -> ComponentId {
        world.register_component::<Children>()
    }

    #[inline(always)]
    fn extract(
        &self,
        entity: &mut FilteredEntityAccess,
        component_id: ComponentId,
        _validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Py<PyAny>> {
        // Children is read-only, use get_by_id and return an owned copy
        let ptr = entity
            .get_by_id(component_id)
            .ok_or_else(|| PyRuntimeError::new_err("Children not found"))?;

        let component = unsafe { ptr.deref::<Children>() };
        let py_component = hierarchy::PyChildren::try_from(component)?;
        let obj = Py::new(py, (py_component, PyComponent))?;
        Ok(obj.into_any())
    }

    fn insert(
        &self,
        _world: &mut World,
        _entity: Entity,
        _component: &Bound<PyAny>,
    ) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Children cannot be spawned from Python - it is auto-managed by Bevy",
        ))
    }

    fn insert_into_entity(
        &self,
        _entity: &mut EntityWorldMut,
        _component: &Bound<PyAny>,
    ) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Children cannot be spawned from Python - it is auto-managed by Bevy",
        ))
    }

    fn extract_fn(&self) -> ExtractFn {
        #[inline(always)]
        fn extract_impl(
            entity: &mut FilteredEntityAccess,
            component_id: ComponentId,
            _validity: ValidityFlagWithMode,
            py: Python,
        ) -> PyResult<Py<PyAny>> {
            let ptr = entity
                .get_by_id(component_id)
                .ok_or_else(|| PyRuntimeError::new_err("Children not found"))?;

            let component = unsafe { ptr.deref::<Children>() };
            let py_component = crate::hierarchy::PyChildren::try_from(component)?;
            let obj = Py::new(py, (py_component, crate::component::PyComponent))?;
            Ok(obj.into_any())
        }
        extract_impl
    }

    fn entity_contains(&self, entity: &EntityRef) -> bool {
        entity.contains::<Children>()
    }

    unsafe fn extract_from_entity_ref(
        &self,
        entity_id: Entity,
        world_ptr: *mut World,
        _validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>> {
        // Children is read-only; return an owned copy.
        // SAFETY: world_ptr is valid for a shared borrow here (caller contract).
        let Some(entity_ref) = (unsafe { entity_ref_from_ptr(entity_id, world_ptr) }) else {
            return Ok(None);
        };
        if let Some(component) = entity_ref.get::<Children>() {
            let py_component = hierarchy::PyChildren::try_from(component)?;
            let obj = Py::new(py, (py_component, PyComponent))?;
            Ok(Some(obj.into_any()))
        } else {
            Ok(None)
        }
    }

    unsafe fn extract_from_entity_mut(
        &self,
        entity_id: Entity,
        world_ptr: *mut World,
        _validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>> {
        // Children is read-only; return an owned copy.
        // SAFETY: world_ptr is valid for a shared borrow here (caller contract).
        let Some(entity_ref) = (unsafe { entity_ref_from_ptr(entity_id, world_ptr) }) else {
            return Ok(None);
        };
        if let Some(component) = entity_ref.get::<Children>() {
            let py_component = hierarchy::PyChildren::try_from(component)?;
            let obj = Py::new(py, (py_component, PyComponent))?;
            Ok(Some(obj.into_any()))
        } else {
            Ok(None)
        }
    }
}

pub use asset::{NativeAsset, PyAsset};
pub use asset_access::{ActiveAssetAccessError, ensure_no_live_asset_access};
pub use asset_cleanup::AssetCleanupRegistration;
pub use asset_path::PyAssetPath;
pub use bridge_inventory::{
    AssetBridgeRegistration, BatchRegistration, ComponentBridgeRegistration,
    MessageBridgeRegistration, PluginBridgeRegistration, ResourceBridgeRegistration,
};
pub use component::PyComponent;
pub use debug_snapshot::{DebugSnapshot, ReloadMemorySnapshotInfo, SystemProfile};
pub use duration::duration_from_py;
pub use entity::PyEntity;
pub use handle::{PyHandle, extract_handle_from_any};
pub use hierarchy::{PyChildOf, PyChildren, PyChildrenIterator};
pub use materializable::PyMaterializable;
pub use message::{PyMessage, PyMessageId};
pub use plugin::{PluginBridge, PluginBuild, PyPlugin};
pub use pybevy_storage::{
    AccessMode, AppId, AppLifecycle, AppOperation, AppStoreCore, AppStoreError, AssetBorrowCounter,
    AssetRuntimeCore, AssetRuntimeError, AssetStorage, BorrowableStorage, ComponentStorage,
    ComponentStorageInner, FieldOffset, FieldStorage, FieldStorageInner, FieldType,
    FilteredEntityAccess, FromBorrowedStorage, ResourceStorage, ResourceStorageInner, StorageError,
    ValidityFlag, ValidityFlagWithMode, ValidityGuard, ValueStorage, ValueStorageInner, ViewBridge,
    ViewFieldAccess, allocate_id, consume_unstored_id,
};
pub use reflect_registration::{ReflectTypeRegistration, register_wrapped_reflect_types};
pub use registry::{
    AssetBridge, AssetInputConverter, BatchComponent, BatchFieldMeta, BatchableField,
    ComponentBatchInsertFn, ComponentBatchMeta, ComponentBatchPrepareFn, ComponentBridge,
    ExtractFn, MessageBridge, PluginConfigs, PreparedBatchComponent, PreparedNativeBatch,
    PreparedNativeUniform, PreparedUniformComponent, PreparedUniformFn, PyRustComponentBatch,
    ResourceBridge, batch_field_meta_for, field_type_of, set_field_from_numpy,
};
pub use reload_request::{
    CustomComponentEntry, CustomComponentInfo, CustomResourceEntry, CustomResourceInfo,
    LastSystemError, PendingReloadRequest, PyResourceStorage, ReloadRequestMode, ReloadResult,
};
pub use resource::PyResource;
pub use uuid;

pub fn register_core_bridges() {
    registry::global_registry::register_component_bridge(ChildOfBridge);
    registry::global_registry::register_component_bridge(ChildrenBridge);
    registry::rust_batch::register_rust_batch_bridge();
}
