//! Inventory-based auto-registration for bridges.
//!
//! Feature crates submit bridge registrations via `inventory::submit!()`,
//! and `collect_all()` is called once at startup to register them all.

use std::sync::Arc;

use crate::{
    plugin::{PluginBridge, plugin_registry},
    registry::{AssetBridge, ComponentBridge, MessageBridge, ResourceBridge, global_registry},
};

/// A component bridge registration collected via `inventory`.
pub struct ComponentBridgeRegistration {
    /// Factory function that returns the bridge instance.
    pub create: fn() -> Arc<dyn ComponentBridge>,
}

inventory::collect!(ComponentBridgeRegistration);

/// A resource bridge registration collected via `inventory`.
pub struct ResourceBridgeRegistration {
    /// Factory function that returns the bridge instance.
    pub create: fn() -> Arc<dyn ResourceBridge>,
}

inventory::collect!(ResourceBridgeRegistration);

/// A message bridge registration collected via `inventory`.
pub struct MessageBridgeRegistration {
    /// Factory function that returns the bridge instance.
    pub create: fn() -> Arc<dyn MessageBridge>,
}

inventory::collect!(MessageBridgeRegistration);

/// A plugin bridge registration collected via `inventory`.
pub struct PluginBridgeRegistration {
    /// Factory function that returns the bridge instance.
    pub create: fn() -> Arc<dyn PluginBridge>,
}

inventory::collect!(PluginBridgeRegistration);

/// An asset bridge registration collected via `inventory`.
pub struct AssetBridgeRegistration {
    /// Factory function that returns the bridge instance.
    pub create: fn() -> Arc<dyn AssetBridge>,
}

inventory::collect!(AssetBridgeRegistration);

/// A batch metadata registration collected via `inventory`.
pub struct BatchRegistration {
    /// Registration function that registers batch metadata.
    pub register: fn(),
}

inventory::collect!(BatchRegistration);

/// Collect all inventory-registered bridges and register them in the global registries.
///
/// Call this once at startup (e.g., in `init_module`).
pub fn collect_all() {
    for reg in inventory::iter::<ComponentBridgeRegistration> {
        let bridge = (reg.create)();
        global_registry::register_component_bridge_arc(bridge);
    }

    for reg in inventory::iter::<ResourceBridgeRegistration> {
        let bridge = (reg.create)();
        global_registry::register_resource_bridge_arc(bridge);
    }

    for reg in inventory::iter::<MessageBridgeRegistration> {
        let bridge = (reg.create)();
        global_registry::register_message_bridge_arc(bridge);
    }

    for reg in inventory::iter::<AssetBridgeRegistration> {
        let bridge = (reg.create)();
        global_registry::register_asset_bridge_arc(bridge);
    }

    for reg in inventory::iter::<PluginBridgeRegistration> {
        let bridge = (reg.create)();
        plugin_registry::register_plugin_bridge_arc(bridge);
    }

    for reg in inventory::iter::<BatchRegistration> {
        (reg.register)();
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::{
        any::TypeId,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
    };

    use bevy::{
        asset::{AssetPath, AssetServer, UntypedAssetId, UntypedHandle},
        ecs::{
            component::ComponentId,
            entity::Entity,
            world::{EntityRef, EntityWorldMut, World, unsafe_world_cell::UnsafeWorldCell},
        },
    };
    use pyo3::{ffi::PyTypeObject, prelude::*, types::PyType};

    use super::*;
    use crate::{
        AssetBorrowCounter, AssetEventRecord, AssetLoadFailedRecord, ExtractFn,
        FilteredEntityAccess, ValidityFlagWithMode,
    };

    // Unique fake pointers/TypeIds: registry keys are opaque and never dereferenced.
    const COMPONENT_PTR: usize = 0x7101;
    const RESOURCE_PTR: usize = 0x7102;
    const MESSAGE_PTR: usize = 0x7103;
    const ASSET_PTR: usize = 0x7104;
    const PLUGIN_PTR: usize = 0x7105;

    static COMPONENT_FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);
    static RESOURCE_FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);
    static MESSAGE_FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);
    static ASSET_FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);
    static PLUGIN_FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);
    static BATCH_REGISTERED: AtomicBool = AtomicBool::new(false);

    struct InvComponentBridge;

    impl ComponentBridge for InvComponentBridge {
        fn bevy_type_id(&self) -> TypeId {
            TypeId::of::<InvComponentBridge>()
        }

        fn py_type_ptr(&self) -> *const PyTypeObject {
            COMPONENT_PTR as *const PyTypeObject
        }

        fn py_type<'py>(&self, _py: Python<'py>) -> Bound<'py, PyType> {
            unreachable!("identity-only fake bridge")
        }

        fn name(&self) -> &'static str {
            "InvComponentBridge"
        }

        fn can_insert(&self) -> bool {
            false
        }

        fn register(&self, _world: &mut World) -> ComponentId {
            ComponentId::new(0)
        }

        fn extract(
            &self,
            _entity: &mut FilteredEntityAccess,
            _component_id: ComponentId,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Py<PyAny>> {
            unreachable!("identity-only fake bridge")
        }

        fn extract_fn(&self) -> ExtractFn {
            fake_component_extract
        }

        fn insert(
            &self,
            _world: &mut World,
            _entity: Entity,
            _component: &Bound<'_, PyAny>,
        ) -> PyResult<()> {
            unreachable!("identity-only fake bridge")
        }

        fn insert_into_entity(
            &self,
            _entity: &mut EntityWorldMut,
            _component: &Bound<'_, PyAny>,
        ) -> PyResult<()> {
            unreachable!("identity-only fake bridge")
        }

        fn entity_contains(&self, _entity: &EntityRef) -> bool {
            false
        }

        unsafe fn extract_from_entity_ref(
            &self,
            _entity: Entity,
            _world: *mut World,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!("identity-only fake bridge")
        }

        unsafe fn extract_from_entity_mut(
            &self,
            _entity: Entity,
            _world: *mut World,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!("identity-only fake bridge")
        }
    }

    fn fake_component_extract(
        _entity: &mut FilteredEntityAccess,
        _component_id: ComponentId,
        _validity: ValidityFlagWithMode,
        _py: Python,
    ) -> PyResult<Py<PyAny>> {
        unreachable!("identity-only fake bridge")
    }

    struct InvResourceBridge;

    impl ResourceBridge for InvResourceBridge {
        fn bevy_type_id(&self) -> TypeId {
            TypeId::of::<InvResourceBridge>()
        }

        fn py_type_ptr(&self) -> *const PyTypeObject {
            RESOURCE_PTR as *const PyTypeObject
        }

        fn py_type<'py>(&self, _py: Python<'py>) -> Bound<'py, PyType> {
            unreachable!("identity-only fake bridge")
        }

        fn name(&self) -> &'static str {
            "InvResourceBridge"
        }

        fn is_mutable(&self) -> bool {
            false
        }

        fn preserve_on_reload(&self) -> bool {
            false
        }

        fn extract(
            &self,
            _entity: &mut FilteredEntityAccess,
            _component_id: ComponentId,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Py<PyAny>> {
            unreachable!("identity-only fake bridge")
        }

        fn entity_contains(&self, _entity: &EntityRef) -> bool {
            false
        }

        unsafe fn extract_from_entity_ref(
            &self,
            _entity: Entity,
            _world: *mut World,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!("identity-only fake bridge")
        }

        unsafe fn extract_from_entity_mut(
            &self,
            _entity: Entity,
            _world: *mut World,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!("identity-only fake bridge")
        }

        fn get(
            &self,
            _world: &World,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Py<PyAny>> {
            unreachable!("identity-only fake bridge")
        }

        fn get_mut(
            &self,
            _world: &mut World,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Py<PyAny>> {
            unreachable!("identity-only fake bridge")
        }

        unsafe fn get_from_cell(
            &self,
            _cell: UnsafeWorldCell<'_>,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Py<PyAny>> {
            unreachable!("identity-only fake bridge")
        }

        unsafe fn get_mut_from_cell(
            &self,
            _cell: UnsafeWorldCell<'_>,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Py<PyAny>> {
            unreachable!("identity-only fake bridge")
        }

        fn insert(&self, _world: &mut World, _resource: &Bound<'_, PyAny>) -> PyResult<()> {
            unreachable!("identity-only fake bridge")
        }

        fn take(&self, _world: &mut World, _py: Python) -> PyResult<Option<Py<PyAny>>> {
            unreachable!("identity-only fake bridge")
        }

        fn remove(&self, _world: &mut World) {}

        fn contains_in_world(&self, _world: &World) -> bool {
            false
        }

        fn resource_id(&self, _world: &World) -> Option<ComponentId> {
            None
        }

        fn register_resource_id(&self, _world: &mut World) -> ComponentId {
            ComponentId::new(0)
        }

        fn reset_to_default(&self, _world: &mut World) -> bool {
            false
        }
    }

    struct InvMessageBridge;

    impl MessageBridge for InvMessageBridge {
        fn bevy_type_id(&self) -> TypeId {
            TypeId::of::<InvMessageBridge>()
        }

        fn py_type_ptr(&self) -> *const PyTypeObject {
            MESSAGE_PTR as *const PyTypeObject
        }

        fn py_type<'py>(&self, _py: Python<'py>) -> Bound<'py, PyType> {
            unreachable!("identity-only fake bridge")
        }

        fn name(&self) -> &'static str {
            "InvMessageBridge"
        }

        fn iter_to_python(&self, _py: Python, _world: &mut World) -> PyResult<Vec<Py<PyAny>>> {
            unreachable!("identity-only fake bridge")
        }

        fn clear(&self, _world: &mut World) -> PyResult<()> {
            unreachable!("identity-only fake bridge")
        }

        fn is_empty(&self, _world: &mut World) -> PyResult<bool> {
            unreachable!("identity-only fake bridge")
        }

        fn len(&self, _world: &mut World) -> PyResult<usize> {
            unreachable!("identity-only fake bridge")
        }

        fn resource_id(&self, _world: &World) -> Option<ComponentId> {
            None
        }

        fn register_resource_id(&self, _world: &mut World) -> ComponentId {
            ComponentId::new(0)
        }
    }

    struct InvAssetBridge;

    impl AssetBridge for InvAssetBridge {
        fn bevy_type_id(&self) -> TypeId {
            TypeId::of::<InvAssetBridge>()
        }

        fn py_type_ptr(&self) -> *const PyTypeObject {
            ASSET_PTR as *const PyTypeObject
        }

        fn py_type<'py>(&self, _py: Python<'py>) -> Bound<'py, PyType> {
            unreachable!("identity-only fake bridge")
        }

        fn name(&self) -> &'static str {
            "InvAssetBridge"
        }

        fn assets_type_id(&self) -> TypeId {
            unreachable!("asset resource identity is not exercised by this fake bridge")
        }

        fn resource_id(&self, _world: &World) -> Option<ComponentId> {
            None
        }

        fn register_resource_id(&self, _world: &mut World) -> ComponentId {
            ComponentId::new(0)
        }

        fn register_event_resource_id(&self, _world: &mut World) -> ComponentId {
            ComponentId::new(0)
        }

        fn read_events(
            &self,
            _world: &World,
            _cursor: &mut Option<Box<dyn std::any::Any + Send + Sync>>,
        ) -> Vec<AssetEventRecord> {
            Vec::new()
        }

        fn clear_events(&self, _world: &mut World) {}

        fn events_is_empty(&self, _world: &World) -> bool {
            true
        }

        fn event_count(&self, _world: &World) -> usize {
            0
        }

        fn register_load_failed_resource_id(&self, _world: &mut World) -> ComponentId {
            ComponentId::new(0)
        }

        fn read_load_failed_events(
            &self,
            _world: &World,
            _cursor: &mut Option<Box<dyn std::any::Any + Send + Sync>>,
        ) -> Vec<AssetLoadFailedRecord> {
            Vec::new()
        }

        fn clear_load_failed_events(&self, _world: &mut World) {}

        fn load_failed_events_is_empty(&self, _world: &World) -> bool {
            true
        }

        fn load_failed_event_count(&self, _world: &World) -> usize {
            0
        }

        fn get(
            &self,
            _world: &World,
            _id: UntypedAssetId,
            _validity: ValidityFlagWithMode,
            _borrow_counter: AssetBorrowCounter,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!("identity-only fake bridge")
        }

        fn get_mut(
            &self,
            _world: UnsafeWorldCell<'_>,
            _id: UntypedAssetId,
            _validity: ValidityFlagWithMode,
            _borrow_counter: AssetBorrowCounter,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!("identity-only fake bridge")
        }

        fn add(
            &self,
            _world: &mut World,
            _asset: &Bound<'_, PyAny>,
            _py: Python,
        ) -> PyResult<UntypedHandle> {
            unreachable!("identity-only fake bridge")
        }

        fn remove(&self, _world: &mut World, _id: UntypedAssetId) -> PyResult<bool> {
            unreachable!("identity-only fake bridge")
        }

        fn remove_and_return(
            &self,
            _world: &mut World,
            _id: UntypedAssetId,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!("identity-only fake bridge")
        }

        fn len(&self, _world: &World) -> PyResult<usize> {
            unreachable!("identity-only fake bridge")
        }

        fn contains(&self, _world: &World, _id: UntypedAssetId) -> PyResult<bool> {
            unreachable!("identity-only fake bridge")
        }

        fn iter_pairs(
            &self,
            _world: &World,
            _validity: ValidityFlagWithMode,
            _borrow_counter: AssetBorrowCounter,
            _py: Python,
        ) -> PyResult<Vec<(UntypedAssetId, Py<PyAny>)>> {
            unreachable!("identity-only fake bridge")
        }

        fn load(&self, _server: &AssetServer, _path: AssetPath<'_>) -> UntypedHandle {
            unreachable!("identity-only fake bridge")
        }

        fn get_handle(&self, _server: &AssetServer, _path: AssetPath<'_>) -> Option<UntypedHandle> {
            None
        }

        fn clear_programmatic(&self, _world: &mut World, _verbose: bool) {}
    }

    struct InvPluginBridge;

    impl PluginBridge for InvPluginBridge {
        fn py_type_id(&self) -> TypeId {
            TypeId::of::<InvPluginBridge>()
        }

        fn py_type_ptr(&self) -> *const PyTypeObject {
            PLUGIN_PTR as *const PyTypeObject
        }

        fn py_type<'py>(&self, _py: Python<'py>) -> Bound<'py, PyType> {
            unreachable!("identity-only fake bridge")
        }

        fn build(&self, _py_plugin: &Bound<'_, PyAny>, _app: &mut bevy::app::App) -> PyResult<()> {
            unreachable!("identity-only fake bridge")
        }

        fn is_added(&self, _app: &bevy::app::App) -> bool {
            false
        }

        fn name(&self) -> &'static str {
            "InvPluginBridge"
        }
    }

    inventory::submit! {
        ComponentBridgeRegistration {
            create: || {
                COMPONENT_FACTORY_CALLS.fetch_add(1, Ordering::SeqCst);
                Arc::new(InvComponentBridge)
            },
        }
    }

    inventory::submit! {
        ResourceBridgeRegistration {
            create: || {
                RESOURCE_FACTORY_CALLS.fetch_add(1, Ordering::SeqCst);
                Arc::new(InvResourceBridge)
            },
        }
    }

    inventory::submit! {
        MessageBridgeRegistration {
            create: || {
                MESSAGE_FACTORY_CALLS.fetch_add(1, Ordering::SeqCst);
                Arc::new(InvMessageBridge)
            },
        }
    }

    inventory::submit! {
        AssetBridgeRegistration {
            create: || {
                ASSET_FACTORY_CALLS.fetch_add(1, Ordering::SeqCst);
                Arc::new(InvAssetBridge)
            },
        }
    }

    inventory::submit! {
        PluginBridgeRegistration {
            create: || {
                PLUGIN_FACTORY_CALLS.fetch_add(1, Ordering::SeqCst);
                Arc::new(InvPluginBridge)
            },
        }
    }

    inventory::submit! {
        BatchRegistration {
            register: || BATCH_REGISTERED.store(true, Ordering::SeqCst),
        }
    }

    fn comp_bridge() -> Arc<dyn ComponentBridge> {
        global_registry::get_bridge_by_py_type(COMPONENT_PTR as *const PyTypeObject).unwrap()
    }

    fn res_bridge() -> Arc<dyn ResourceBridge> {
        global_registry::get_resource_bridge_by_py_type(RESOURCE_PTR as *const PyTypeObject)
            .unwrap()
    }

    fn msg_bridge_by_ptr() -> Arc<dyn MessageBridge> {
        global_registry::get_message_bridge_by_py_type(MESSAGE_PTR as *const PyTypeObject).unwrap()
    }

    fn msg_bridge_by_type_id() -> Arc<dyn MessageBridge> {
        global_registry::get_message_bridge_by_type_id(TypeId::of::<InvMessageBridge>()).unwrap()
    }

    fn asset_bridge_by_ptr() -> Arc<dyn AssetBridge> {
        global_registry::get_asset_bridge_by_py_type(ASSET_PTR as *const PyTypeObject).unwrap()
    }

    fn asset_bridge_by_type_id() -> Arc<dyn AssetBridge> {
        global_registry::get_asset_bridge_by_type_id(TypeId::of::<InvAssetBridge>()).unwrap()
    }

    fn asset_bridge_by_name() -> Arc<dyn AssetBridge> {
        global_registry::get_asset_bridge_by_name("InvAssetBridge").unwrap()
    }

    fn plugin_bridge() -> Arc<dyn PluginBridge> {
        plugin_registry::get_by_py_type(PLUGIN_PTR as *const PyTypeObject).unwrap()
    }

    #[test]
    fn collect_all_dispatches_every_collection_to_its_registry() {
        let (comp_before, res_before) = (
            COMPONENT_FACTORY_CALLS.load(Ordering::SeqCst),
            RESOURCE_FACTORY_CALLS.load(Ordering::SeqCst),
        );
        let (msg_before, asset_before, plugin_before) = (
            MESSAGE_FACTORY_CALLS.load(Ordering::SeqCst),
            ASSET_FACTORY_CALLS.load(Ordering::SeqCst),
            PLUGIN_FACTORY_CALLS.load(Ordering::SeqCst),
        );
        assert!(
            !BATCH_REGISTERED.load(Ordering::SeqCst),
            "batch fn must not run yet"
        );

        collect_all();

        assert_eq!(
            COMPONENT_FACTORY_CALLS.load(Ordering::SeqCst),
            comp_before + 1
        );
        assert_eq!(
            RESOURCE_FACTORY_CALLS.load(Ordering::SeqCst),
            res_before + 1
        );
        assert_eq!(MESSAGE_FACTORY_CALLS.load(Ordering::SeqCst), msg_before + 1);
        assert_eq!(ASSET_FACTORY_CALLS.load(Ordering::SeqCst), asset_before + 1);
        assert_eq!(
            PLUGIN_FACTORY_CALLS.load(Ordering::SeqCst),
            plugin_before + 1
        );
        assert!(
            BATCH_REGISTERED.load(Ordering::SeqCst),
            "batch registration must run"
        );

        // Every collection lands in its own registry under its exact keys.
        assert_eq!(comp_bridge().name(), "InvComponentBridge");
        assert_eq!(res_bridge().name(), "InvResourceBridge");
        assert_eq!(msg_bridge_by_ptr().name(), "InvMessageBridge");
        assert_eq!(asset_bridge_by_ptr().name(), "InvAssetBridge");
        assert_eq!(plugin_bridge().name(), "InvPluginBridge");

        // One stored Arc answers every key of the same bridge.
        assert!(Arc::ptr_eq(&msg_bridge_by_ptr(), &msg_bridge_by_type_id()));
        assert!(Arc::ptr_eq(
            &asset_bridge_by_ptr(),
            &asset_bridge_by_type_id()
        ));
        assert!(Arc::ptr_eq(&asset_bridge_by_ptr(), &asset_bridge_by_name()));

        // Unrelated keys stay absent.
        assert!(
            global_registry::get_message_bridge_by_py_type(0x7999 as *const PyTypeObject).is_none()
        );
        assert!(global_registry::get_asset_bridge_by_name("InvComponentBridge").is_none());
        assert!(!plugin_registry::has_plugin(0x7999 as *const PyTypeObject));
    }
}
