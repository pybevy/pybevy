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
//! - `DynamicComponentRegistry` - Runtime registry for component bridges
//! - `DynamicAssetRegistry` - Runtime registry for asset bridges
//!
//! The registry system allows feature crates (pybevy_audio, pybevy_light, etc.)
//! to register their types without the core crate needing to import them at
//! compile time, enabling independent compilation and faster incremental builds.

pub mod asset;
pub mod asset_path;
pub mod component;
pub mod debug_snapshot;
pub mod entity;
pub mod handle;
pub mod hierarchy;
pub mod materializable;
pub mod message;
pub mod plugin;
pub mod registry;
pub mod reload_request;
pub mod resource;

// Storage layer — re-exported from pybevy_storage
pub use pybevy_storage::{
    component_storage, field_storage, list_storage, resource_storage, storage, storage_error,
    storage_traits, validity_guard, value_storage, view_bridge,
};

// PyF32List (was in list_storage.rs, needs pyo3)
pybevy_storage::impl_py_list!(PyF32List, "F32List", f32);

use bevy::ecs::{
    component::ComponentId,
    entity::Entity,
    hierarchy::{ChildOf, Children},
    world::{FilteredEntityMut, World},
};
use pyo3::prelude::*;

// Manual implementation of ChildOfBridge because component_bridge! macro
// uses pybevy_core:: paths which don't work inside pybevy_core itself.
pub struct ChildOfBridge;

impl ComponentBridge for ChildOfBridge {
    fn bevy_type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<ChildOf>()
    }

    fn py_type_ptr(&self) -> *const pyo3::ffi::PyTypeObject {
        Python::attach(|py| {
            <hierarchy::PyChildOf as pyo3::PyTypeInfo>::type_object(py).as_type_ptr()
        })
    }

    fn py_type<'py>(&self, py: Python<'py>) -> pyo3::Bound<'py, pyo3::types::PyType> {
        <hierarchy::PyChildOf as pyo3::PyTypeInfo>::type_object(py)
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
        entity: &mut FilteredEntityMut,
        component_id: ComponentId,
        _validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Py<PyAny>> {
        // ChildOf is an immutable component (relation), so we use get_by_id for read-only access
        // and return an owned copy rather than a borrowed reference.
        let ptr = entity
            .get_by_id(component_id)
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("ChildOf not found"))?;

        let component = unsafe { ptr.deref::<ChildOf>() };
        let py_component = hierarchy::PyChildOf::try_from(component)?;
        let obj = Py::new(py, (py_component, PyComponent))?;
        Ok(obj.into_any())
    }

    fn insert(
        &self,
        world: &mut World,
        entity: Entity,
        component: &pyo3::Bound<PyAny>,
    ) -> PyResult<()> {
        let py_component = component.extract::<pyo3::PyRef<hierarchy::PyChildOf>>()?;
        let native: ChildOf = py_component.storage.as_ref()?.clone();

        world.entity_mut(entity).insert(native);
        Ok(())
    }

    fn insert_into_entity(
        &self,
        entity: &mut bevy::ecs::world::EntityWorldMut,
        component: &pyo3::Bound<PyAny>,
    ) -> PyResult<()> {
        let py_component = component.extract::<pyo3::PyRef<hierarchy::PyChildOf>>()?;
        let native: ChildOf = py_component.storage.as_ref()?.clone();

        entity.insert(native);
        Ok(())
    }

    fn extract_fn(&self) -> ExtractFn {
        #[inline(always)]
        fn extract_impl(
            entity: &mut FilteredEntityMut,
            component_id: ComponentId,
            _validity: ValidityFlagWithMode,
            py: Python,
        ) -> PyResult<Py<PyAny>> {
            // ChildOf is an immutable component (relation), so we use get_by_id for read-only access
            // and return an owned copy rather than a borrowed reference.
            let ptr = entity
                .get_by_id(component_id)
                .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("ChildOf not found"))?;

            let component = unsafe { ptr.deref::<bevy::ecs::hierarchy::ChildOf>() };
            let py_component = crate::hierarchy::PyChildOf::try_from(component)?;
            let obj = Py::new(py, (py_component, crate::component::PyComponent))?;
            Ok(obj.into_any())
        }
        extract_impl
    }

    fn entity_contains(&self, entity: &bevy::ecs::world::EntityRef) -> bool {
        entity.contains::<ChildOf>()
    }

    fn extract_from_entity_ref(
        &self,
        entity: &bevy::ecs::world::EntityRef,
        validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>> {
        if let Some(component) = entity.get::<ChildOf>() {
            let ptr = component as *const ChildOf as *mut ChildOf;
            let storage = unsafe { ComponentStorage::borrowed(ptr, validity) };
            let obj = Py::new(py, hierarchy::PyChildOf::from_borrowed(storage))?;
            Ok(Some(obj.into_any()))
        } else {
            Ok(None)
        }
    }

    fn extract_from_entity_mut(
        &self,
        entity: &mut bevy::ecs::world::EntityWorldMut,
        _validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>> {
        // ChildOf is an immutable component (relation), so we can only read it.
        // Return an owned copy instead of a borrowed reference.
        if let Some(component) = entity.get::<ChildOf>() {
            let py_component = hierarchy::PyChildOf::try_from(component)?;
            let obj = Py::new(py, (py_component, PyComponent))?;
            Ok(Some(obj.into_any()))
        } else {
            Ok(None)
        }
    }
}

// ============================================================================
// ChildrenBridge - Read-only component, auto-managed by Bevy
// ============================================================================

pub struct ChildrenBridge;

impl ComponentBridge for ChildrenBridge {
    fn bevy_type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<Children>()
    }

    fn py_type_ptr(&self) -> *const pyo3::ffi::PyTypeObject {
        Python::attach(|py| {
            <hierarchy::PyChildren as pyo3::PyTypeInfo>::type_object(py).as_type_ptr()
        })
    }

    fn py_type<'py>(&self, py: Python<'py>) -> pyo3::Bound<'py, pyo3::types::PyType> {
        <hierarchy::PyChildren as pyo3::PyTypeInfo>::type_object(py)
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
        entity: &mut FilteredEntityMut,
        component_id: ComponentId,
        _validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Py<PyAny>> {
        // Children is read-only, use get_by_id and return an owned copy
        let ptr = entity
            .get_by_id(component_id)
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Children not found"))?;

        let component = unsafe { ptr.deref::<Children>() };
        let py_component = hierarchy::PyChildren::try_from(component)?;
        let obj = Py::new(py, (py_component, PyComponent))?;
        Ok(obj.into_any())
    }

    fn insert(
        &self,
        _world: &mut World,
        _entity: Entity,
        _component: &pyo3::Bound<PyAny>,
    ) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Children cannot be spawned from Python - it is auto-managed by Bevy",
        ))
    }

    fn insert_into_entity(
        &self,
        _entity: &mut bevy::ecs::world::EntityWorldMut,
        _component: &pyo3::Bound<PyAny>,
    ) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Children cannot be spawned from Python - it is auto-managed by Bevy",
        ))
    }

    fn extract_fn(&self) -> ExtractFn {
        #[inline(always)]
        fn extract_impl(
            entity: &mut FilteredEntityMut,
            component_id: ComponentId,
            _validity: ValidityFlagWithMode,
            py: Python,
        ) -> PyResult<Py<PyAny>> {
            let ptr = entity
                .get_by_id(component_id)
                .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Children not found"))?;

            let component = unsafe { ptr.deref::<bevy::ecs::hierarchy::Children>() };
            let py_component = crate::hierarchy::PyChildren::try_from(component)?;
            let obj = Py::new(py, (py_component, crate::component::PyComponent))?;
            Ok(obj.into_any())
        }
        extract_impl
    }

    fn entity_contains(&self, entity: &bevy::ecs::world::EntityRef) -> bool {
        entity.contains::<Children>()
    }

    fn extract_from_entity_ref(
        &self,
        entity: &bevy::ecs::world::EntityRef,
        _validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>> {
        if let Some(component) = entity.get::<Children>() {
            let py_component = hierarchy::PyChildren::try_from(component)?;
            let obj = Py::new(py, (py_component, PyComponent))?;
            Ok(Some(obj.into_any()))
        } else {
            Ok(None)
        }
    }

    fn extract_from_entity_mut(
        &self,
        entity: &mut bevy::ecs::world::EntityWorldMut,
        _validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>> {
        // Children is read-only, return owned copy
        if let Some(component) = entity.get::<Children>() {
            let py_component = hierarchy::PyChildren::try_from(component)?;
            let obj = Py::new(py, (py_component, PyComponent))?;
            Ok(Some(obj.into_any()))
        } else {
            Ok(None)
        }
    }
}

// Re-export commonly used types at crate root
pub use asset::{NativeAsset, PyAsset};
pub use asset_path::PyAssetPath;
pub use component::PyComponent;
pub use debug_snapshot::{DebugSnapshot, ReloadMemorySnapshotInfo};
pub use entity::PyEntity;
pub use handle::{PyHandle, extract_handle_from_any};
pub use hierarchy::{PyChildOf, PyChildren, PyChildrenIterator};
pub use materializable::PyMaterializable;
pub use message::{PyMessage, PyMessageId};
pub use plugin::{PluginBridge, PyPlugin};
// Storage types re-exported from pybevy_storage
pub use pybevy_storage::{
    AccessMode, AssetStorage, BorrowableStorage, ComponentStorage, ComponentStorageInner,
    FieldOffset, FieldStorage, FieldStorageInner, FromBorrowedStorage, ListStorage,
    ListStorageInner, ResourceStorage, ResourceStorageInner, StorageError, ValidityFlag,
    ValidityFlagWithMode, ValidityGuard, ValueStorage, ValueStorageInner, ViewBridge,
    ViewFieldAccess, normalize_index,
};
pub use registry::{
    AssetBridge, BatchComponent, BatchFieldMeta, BatchableField, ComponentBatchInsertFn,
    ComponentBatchMeta, ComponentBridge, DynamicAssetRegistry, DynamicComponentRegistry,
    DynamicResourceRegistry, ExtractFn, MessageBridge, PluginConfigs, PyRustComponentBatch,
    ResourceBridge, batch_field_meta_for, field_offset_view_meta_for, set_field_from_numpy,
};
pub use reload_request::{
    CustomComponentEntry, CustomComponentInfo, CustomResourceEntry, CustomResourceInfo,
    LastSystemError, PendingReloadRequest, PyResourceStorage, ReloadRequestMode, ReloadResult,
};
pub use resource::PyResource;
// Re-export uuid for use in macros (asset_bridge! needs it for synthetic UUID construction)
pub use uuid;

/// Register core component bridges with global registry.
/// Call this from main crate during initialization.
pub fn register_core_bridges() {
    registry::global_registry::register_component_bridge(ChildOfBridge);
    registry::global_registry::register_component_bridge(ChildrenBridge);
    registry::rust_batch::register_rust_batch_bridge();
}
