//! Global bridge registries for type lookup without World access
//!
//! This module provides global registries that allow `try_from_py_type` to check
//! if a Python type has a registered bridge, without requiring World access.
//!
//! # Design
//!
//! The global registries complement the Bevy resource-based registries:
//! - **Global registry**: Used for type identification (is this type dynamically registered?)
//! - **Bevy resource**: Used for actual dispatch (extract, insert, etc.)
//!
//! Feature crates register their bridges in BOTH:
//! 1. Global registry (at module init or plugin build)
//! 2. Bevy resource (at plugin build)
//!
//! # Thread Safety
//!
//! Uses `OnceLock<RwLock<...>>` for thread-safe lazy initialization and access.
//! The RwLock allows concurrent reads (common case) with exclusive writes (registration).

use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, OnceLock, RwLock},
};

use pyo3::{ffi::PyTypeObject, types::PyTypeMethods};

use super::{
    AssetBridge, BatchComponent, ComponentBridge, MessageBridge, ResourceBridge,
    batchable_field::BatchFieldMeta, rust_batch::PyRustComponentBatch,
};

/// Global registry for component bridge type pointers
///
/// This allows `try_from_py_type` to identify dynamically registered types
/// without requiring World access.
static GLOBAL_COMPONENT_BRIDGES: OnceLock<RwLock<GlobalBridgeRegistry>> = OnceLock::new();

/// Internal storage for the global registry
#[derive(Default)]
struct GlobalBridgeRegistry {
    /// Maps PyTypeObject pointers to bridges.
    by_py_type: HashMap<*const PyTypeObject, Arc<dyn ComponentBridge>>,
    /// Maps Bevy TypeIds to canonical bridges.
    by_type_id: HashMap<TypeId, Arc<dyn ComponentBridge>>,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for GlobalBridgeRegistry {}
unsafe impl Sync for GlobalBridgeRegistry {}

/// Get or initialize the global registry
fn get_global_registry() -> &'static RwLock<GlobalBridgeRegistry> {
    GLOBAL_COMPONENT_BRIDGES.get_or_init(|| RwLock::new(GlobalBridgeRegistry::default()))
}

/// Register a component bridge in the global registry.
///
/// This should be called by feature crates during initialization.
/// The bridge must have a valid py_type_ptr() - null pointers are ignored.
pub fn register_component_bridge<B: ComponentBridge>(bridge: B) {
    register_component_bridge_arc(Arc::new(bridge));
}

/// Register a pre-wrapped Arc bridge (used by PyBevyPlugin for shared ownership)
pub fn register_component_bridge_arc(bridge: Arc<dyn ComponentBridge>) {
    let ptr = bridge.py_type_ptr();
    if ptr.is_null() {
        // Skip registration for bridges without valid Python types
        // (e.g., proof-of-concept bridges)
        return;
    }

    let type_id = bridge.bevy_type_id();
    let registry = get_global_registry();
    let mut guard = registry.write().expect("Global registry lock poisoned");
    guard.by_py_type.insert(ptr, bridge.clone());
    guard.by_type_id.insert(type_id, bridge);
}

/// Register an additional Python class for an existing component bridge.
///
/// Native subclasses such as value-enum variants have distinct Python type
/// pointers but share one Bevy component type and one bridge with their base.
/// Returns `false` when the canonical type has not been registered yet.
pub fn register_component_bridge_alias(
    alias_ptr: *const PyTypeObject,
    canonical_ptr: *const PyTypeObject,
) -> bool {
    if alias_ptr.is_null() || canonical_ptr.is_null() {
        return false;
    }

    let registry = get_global_registry();
    let mut guard = registry.write().expect("Global registry lock poisoned");
    let Some(bridge) = guard.by_py_type.get(&canonical_ptr).cloned() else {
        return false;
    };
    guard.by_py_type.insert(alias_ptr, bridge);
    true
}

/// Check if a Python type pointer is registered in the global registry.
///
/// Returns the bridge if found, None otherwise.
pub fn get_bridge_by_py_type(ptr: *const PyTypeObject) -> Option<Arc<dyn ComponentBridge>> {
    let registry = get_global_registry();
    let guard = registry.read().expect("Global registry lock poisoned");
    guard.by_py_type.get(&ptr).cloned()
}

/// Check if a Python type pointer is registered (without returning the bridge)
pub fn contains_py_type(ptr: *const PyTypeObject) -> bool {
    let registry = get_global_registry();
    let guard = registry.read().expect("Global registry lock poisoned");
    guard.by_py_type.contains_key(&ptr)
}

/// Get all unique registered component bridges.
///
/// Alias Python classes share a bridge with their canonical component type and
/// therefore appear only once in this enumeration.
pub fn all_component_bridges() -> Vec<Arc<dyn ComponentBridge>> {
    let registry = get_global_registry();
    let guard = registry.read().expect("Global registry lock poisoned");
    guard.by_type_id.values().cloned().collect()
}

/// Global registry for resource bridge type pointers
static GLOBAL_RESOURCE_BRIDGES: OnceLock<RwLock<GlobalResourceBridgeRegistry>> = OnceLock::new();

/// Internal storage for the global resource registry
#[derive(Default)]
struct GlobalResourceBridgeRegistry {
    /// Maps PyTypeObject pointers to bridges.
    by_py_type: HashMap<*const PyTypeObject, Arc<dyn ResourceBridge>>,
    /// Maps Bevy TypeIds to canonical bridges.
    by_type_id: HashMap<TypeId, Arc<dyn ResourceBridge>>,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for GlobalResourceBridgeRegistry {}
unsafe impl Sync for GlobalResourceBridgeRegistry {}

/// Get or initialize the global resource registry
fn get_global_resource_registry() -> &'static RwLock<GlobalResourceBridgeRegistry> {
    GLOBAL_RESOURCE_BRIDGES.get_or_init(|| RwLock::new(GlobalResourceBridgeRegistry::default()))
}

/// Register a resource bridge in the global registry.
///
/// This should be called by feature crates during initialization.
/// The bridge must have a valid py_type_ptr() - null pointers are ignored.
pub fn register_resource_bridge<B: ResourceBridge>(bridge: B) {
    register_resource_bridge_arc(Arc::new(bridge));
}

/// Register a pre-wrapped Arc resource bridge (used by inventory auto-registration)
pub fn register_resource_bridge_arc(bridge: Arc<dyn ResourceBridge>) {
    let ptr = bridge.py_type_ptr();
    if ptr.is_null() {
        return;
    }

    let type_id = bridge.bevy_type_id();
    let registry = get_global_resource_registry();
    let mut guard = registry
        .write()
        .expect("Global resource registry lock poisoned");
    guard.by_py_type.insert(ptr, bridge.clone());
    guard.by_type_id.insert(type_id, bridge);
}

/// Register an additional Python class for an existing resource bridge.
///
/// Native subclasses such as data-enum variants have distinct Python type
/// pointers but share one Bevy resource type and one bridge with their base.
/// Returns `false` when the canonical type has not been registered yet.
pub fn register_resource_bridge_alias(
    alias_ptr: *const PyTypeObject,
    canonical_ptr: *const PyTypeObject,
) -> bool {
    if alias_ptr.is_null() || canonical_ptr.is_null() {
        return false;
    }

    let registry = get_global_resource_registry();
    let mut guard = registry
        .write()
        .expect("Global resource registry lock poisoned");
    let Some(bridge) = guard.by_py_type.get(&canonical_ptr).cloned() else {
        return false;
    };
    guard.by_py_type.insert(alias_ptr, bridge);
    true
}

/// Check if a Python type pointer is registered as a resource in the global registry
///
/// Returns the bridge if found, None otherwise.
pub fn get_resource_bridge_by_py_type(ptr: *const PyTypeObject) -> Option<Arc<dyn ResourceBridge>> {
    let registry = get_global_resource_registry();
    let guard = registry
        .read()
        .expect("Global resource registry lock poisoned");
    guard.by_py_type.get(&ptr).cloned()
}

/// Check if a Python type pointer is registered as a resource (without returning the bridge)
pub fn contains_resource_py_type(ptr: *const PyTypeObject) -> bool {
    let registry = get_global_resource_registry();
    let guard = registry
        .read()
        .expect("Global resource registry lock poisoned");
    guard.by_py_type.contains_key(&ptr)
}

/// Get all unique registered resource bridges.
///
/// Alias Python classes share a bridge with their canonical resource type and
/// therefore appear only once in this enumeration.
pub fn all_resource_bridges() -> Vec<Arc<dyn ResourceBridge>> {
    let registry = get_global_resource_registry();
    let guard = registry
        .read()
        .expect("Global resource registry lock poisoned");
    guard.by_type_id.values().cloned().collect()
}

/// Global registry for asset bridge type pointers
static GLOBAL_ASSET_BRIDGES: OnceLock<RwLock<GlobalAssetBridgeRegistry>> = OnceLock::new();

/// Internal storage for the global asset registry
#[derive(Default)]
struct GlobalAssetBridgeRegistry {
    /// Maps PyTypeObject pointers to bridges.
    by_py_type: HashMap<*const PyTypeObject, Arc<dyn AssetBridge>>,
    /// Maps TypeIds to bridges for lookups from Bevy types.
    by_type_id: HashMap<TypeId, Arc<dyn AssetBridge>>,
    /// Maps bridge names to bridges for lookups by asset type name.
    by_name: HashMap<&'static str, Arc<dyn AssetBridge>>,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for GlobalAssetBridgeRegistry {}
unsafe impl Sync for GlobalAssetBridgeRegistry {}

/// Get or initialize the global asset registry
fn get_global_asset_registry() -> &'static RwLock<GlobalAssetBridgeRegistry> {
    GLOBAL_ASSET_BRIDGES.get_or_init(|| RwLock::new(GlobalAssetBridgeRegistry::default()))
}

/// Register an asset bridge in the global registry.
///
/// This should be called by feature crates during initialization.
/// The bridge must have a valid py_type_ptr() - null pointers are ignored.
pub fn register_asset_bridge<B: AssetBridge>(bridge: B) {
    register_asset_bridge_arc(Arc::new(bridge));
}

/// Register a pre-wrapped Arc asset bridge (used by inventory auto-registration)
pub fn register_asset_bridge_arc(bridge: Arc<dyn AssetBridge>) {
    let ptr = bridge.py_type_ptr();
    if ptr.is_null() {
        return;
    }

    let type_id = bridge.bevy_type_id();
    let name = bridge.name();
    let registry = get_global_asset_registry();
    let mut guard = registry
        .write()
        .expect("Global asset registry lock poisoned");
    guard.by_py_type.insert(ptr, bridge.clone());
    guard.by_name.insert(name, bridge.clone());
    guard.by_type_id.insert(type_id, bridge);
}

/// Check if a Python type pointer is registered as an asset in the global registry
///
/// Returns the bridge if found, None otherwise.
pub fn get_asset_bridge_by_py_type(ptr: *const PyTypeObject) -> Option<Arc<dyn AssetBridge>> {
    let registry = get_global_asset_registry();
    let guard = registry
        .read()
        .expect("Global asset registry lock poisoned");
    guard.by_py_type.get(&ptr).cloned()
}

/// Check if a Python type pointer is registered as an asset (without returning the bridge)
pub fn contains_asset_py_type(ptr: *const PyTypeObject) -> bool {
    let registry = get_global_asset_registry();
    let guard = registry
        .read()
        .expect("Global asset registry lock poisoned");
    guard.by_py_type.contains_key(&ptr)
}

/// Get an asset bridge by Bevy TypeId
///
/// Returns the bridge if found, None otherwise.
/// Used by From<Handle<A>> for PyHandle to get the Python type info.
pub fn get_asset_bridge_by_type_id(type_id: TypeId) -> Option<Arc<dyn AssetBridge>> {
    let registry = get_global_asset_registry();
    let guard = registry
        .read()
        .expect("Global asset registry lock poisoned");
    guard.by_type_id.get(&type_id).cloned()
}

/// Get an asset bridge by its name
///
/// Returns the bridge if found, None otherwise.
/// Used by named native operations such as folder loading.
pub fn get_asset_bridge_by_name(name: &str) -> Option<Arc<dyn AssetBridge>> {
    let registry = get_global_asset_registry();
    let guard = registry
        .read()
        .expect("Global asset registry lock poisoned");
    guard.by_name.get(name).cloned()
}

/// Global registry for component TypeId lookups
/// This allows methods like VisibilityClass.contains() to work with
/// any component type (both feature crate and main crate components)
static GLOBAL_TYPE_ID_REGISTRY: OnceLock<RwLock<TypeIdRegistry>> = OnceLock::new();

/// Internal storage for the TypeId registry
#[derive(Default)]
struct TypeIdRegistry {
    /// Maps PyTypeObject pointers to TypeIds.
    by_py_type: HashMap<*const PyTypeObject, TypeId>,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for TypeIdRegistry {}
unsafe impl Sync for TypeIdRegistry {}

/// Get or initialize the TypeId registry
fn get_type_id_registry() -> &'static RwLock<TypeIdRegistry> {
    GLOBAL_TYPE_ID_REGISTRY.get_or_init(|| RwLock::new(TypeIdRegistry::default()))
}

/// Register a component's TypeId in the global registry.
///
/// This should be called by both:
/// - Feature crate #[pycomponent(..., bridge)] attributes
/// - Main crate `#[pycomponent(...)]`, `#[pyresource(...)]`, and `#[pyasset(...)]` storage macros
pub fn register_type_id<P: pyo3::PyTypeInfo, B: 'static>() {
    pyo3::Python::attach(|py| {
        let ptr = P::type_object(py).as_type_ptr();
        let type_id = TypeId::of::<B>();
        let registry = get_type_id_registry();
        let mut guard = registry.write().expect("TypeId registry lock poisoned");
        guard.by_py_type.insert(ptr, type_id);
    });
}

/// Get the TypeId for a Python type pointer
///
/// First checks the component bridge registry, then falls back to the TypeId registry.
/// Returns None if the type is not registered.
pub fn get_type_id_by_py_type(ptr: *const PyTypeObject) -> Option<TypeId> {
    // First check bridge registry (feature crate components)
    if let Some(bridge) = get_bridge_by_py_type(ptr) {
        return Some(bridge.bevy_type_id());
    }

    // Fall back to TypeId registry (main crate components)
    let registry = get_type_id_registry();
    let guard = registry.read().expect("TypeId registry lock poisoned");

    guard.by_py_type.get(&ptr).copied()
}

/// Global registry for message bridge type pointers
static GLOBAL_MESSAGE_BRIDGES: OnceLock<RwLock<GlobalMessageBridgeRegistry>> = OnceLock::new();

/// Internal storage for the global message registry
#[derive(Default)]
struct GlobalMessageBridgeRegistry {
    /// Maps PyTypeObject pointers to bridges.
    by_py_type: HashMap<*const PyTypeObject, Arc<dyn MessageBridge>>,
    /// Maps TypeIds to bridges for lookups from Bevy types.
    by_type_id: HashMap<TypeId, Arc<dyn MessageBridge>>,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for GlobalMessageBridgeRegistry {}
unsafe impl Sync for GlobalMessageBridgeRegistry {}

/// Get or initialize the global message registry
fn get_global_message_registry() -> &'static RwLock<GlobalMessageBridgeRegistry> {
    GLOBAL_MESSAGE_BRIDGES.get_or_init(|| RwLock::new(GlobalMessageBridgeRegistry::default()))
}

/// Register a message bridge in the global registry.
///
/// This should be called by feature crates during initialization.
/// The bridge must have a valid py_type_ptr() - null pointers are ignored.
pub fn register_message_bridge<B: MessageBridge>(bridge: B) {
    register_message_bridge_arc(Arc::new(bridge));
}

/// Register a pre-wrapped Arc message bridge (used by inventory auto-registration)
pub fn register_message_bridge_arc(bridge: Arc<dyn MessageBridge>) {
    let ptr = bridge.py_type_ptr();
    if ptr.is_null() {
        return;
    }

    let type_id = bridge.bevy_type_id();
    let registry = get_global_message_registry();
    let mut guard = registry
        .write()
        .expect("Global message registry lock poisoned");
    guard.by_py_type.insert(ptr, bridge.clone());
    guard.by_type_id.insert(type_id, bridge);
}

/// Check if a Python type pointer is registered as a message in the global registry
///
/// Returns the bridge if found, None otherwise.
pub fn get_message_bridge_by_py_type(ptr: *const PyTypeObject) -> Option<Arc<dyn MessageBridge>> {
    let registry = get_global_message_registry();
    let guard = registry
        .read()
        .expect("Global message registry lock poisoned");
    guard.by_py_type.get(&ptr).cloned()
}

/// Check if a Python type pointer is registered as a message (without returning the bridge)
pub fn contains_message_py_type(ptr: *const PyTypeObject) -> bool {
    let registry = get_global_message_registry();
    let guard = registry
        .read()
        .expect("Global message registry lock poisoned");
    guard.by_py_type.contains_key(&ptr)
}

/// Get a message bridge by Bevy TypeId
///
/// Returns the bridge if found, None otherwise.
/// Used for iterating messages by type.
pub fn get_message_bridge_by_type_id(type_id: TypeId) -> Option<Arc<dyn MessageBridge>> {
    let registry = get_global_message_registry();
    let guard = registry
        .read()
        .expect("Global message registry lock poisoned");
    guard.by_type_id.get(&type_id).cloned()
}

/// Get all registered message bridges
///
/// Returns an iterator over all registered bridges.
/// Used by PyMessages to iterate all message types.
pub fn all_message_bridges() -> Vec<Arc<dyn MessageBridge>> {
    let registry = get_global_message_registry();
    let guard = registry
        .read()
        .expect("Global message registry lock poisoned");
    guard.by_type_id.values().cloned().collect()
}

/// Global registry for batch component type pointers
static GLOBAL_BATCH_BRIDGES: OnceLock<RwLock<GlobalBatchBridgeRegistry>> = OnceLock::new();

/// Internal storage for the global batch registry
#[derive(Default)]
struct GlobalBatchBridgeRegistry {
    /// Maps PyTypeObject pointers to bridges.
    by_py_type: HashMap<*const PyTypeObject, Arc<dyn BatchComponent>>,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for GlobalBatchBridgeRegistry {}
unsafe impl Sync for GlobalBatchBridgeRegistry {}

/// Get or initialize the global batch registry
fn get_global_batch_registry() -> &'static RwLock<GlobalBatchBridgeRegistry> {
    GLOBAL_BATCH_BRIDGES.get_or_init(|| RwLock::new(GlobalBatchBridgeRegistry::default()))
}

/// Register a batch component bridge in the global registry
pub fn register_batch_bridge(py_type_ptr: *const PyTypeObject, bridge: Arc<dyn BatchComponent>) {
    if py_type_ptr.is_null() {
        return;
    }

    let registry = get_global_batch_registry();
    let mut guard = registry
        .write()
        .expect("Global batch registry lock poisoned");
    guard.by_py_type.insert(py_type_ptr, bridge);
}

/// Check if a Python type pointer is registered as a batch component
///
/// Returns the bridge if found, None otherwise.
pub fn get_batch_bridge_by_py_type(ptr: *const PyTypeObject) -> Option<Arc<dyn BatchComponent>> {
    let registry = get_global_batch_registry();
    let guard = registry
        .read()
        .expect("Global batch registry lock poisoned");
    guard.by_py_type.get(&ptr).cloned()
}

/// Function pointer type for macro-generated batch insert functions.
pub type ComponentBatchInsertFn = for<'py> fn(
    pyo3::Python<'py>,
    &PyRustComponentBatch,
    &[bevy::ecs::entity::Entity],
    &mut bevy::ecs::world::World,
) -> pyo3::PyResult<()>;

/// Function pointer type for macro-generated owned batch preparation.
pub type ComponentBatchPrepareFn =
    for<'py> fn(
        pyo3::Python<'py>,
        &PyRustComponentBatch,
    ) -> pyo3::PyResult<Box<dyn super::PreparedBatchComponent>>;

/// Metadata for a Rust component's batch spawning capability.
///
/// Registered by macro-generated code; looked up by RustComponentBatchBridge
/// during insert_bulk.
pub struct ComponentBatchMeta {
    pub component_name: &'static str,
    pub fields: &'static [BatchFieldMeta],
    pub insert_fn: ComponentBatchInsertFn,
    pub prepare_fn: ComponentBatchPrepareFn,
}

// SAFETY: ComponentBatchMeta contains only static references and function pointers
unsafe impl Send for ComponentBatchMeta {}
unsafe impl Sync for ComponentBatchMeta {}

/// Global registry for component batch metadata, keyed by Python type pointer (as usize).
static GLOBAL_COMPONENT_BATCH_META: OnceLock<RwLock<HashMap<usize, &'static ComponentBatchMeta>>> =
    OnceLock::new();

fn get_component_batch_meta_registry()
-> &'static RwLock<HashMap<usize, &'static ComponentBatchMeta>> {
    GLOBAL_COMPONENT_BATCH_META.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Register batch metadata for a Rust component.
///
/// Called by macro-generated registration functions in feature crates.
/// `py_type_ptr` is the Python type pointer as usize for the component's pyclass.
pub fn register_component_batch_meta(py_type_ptr: usize, meta: &'static ComponentBatchMeta) {
    let registry = get_component_batch_meta_registry();
    let mut guard = registry
        .write()
        .expect("Component batch meta registry lock poisoned");
    guard.insert(py_type_ptr, meta);
}

/// Look up batch metadata for a Rust component by its Python type pointer.
pub fn get_component_batch_meta(py_type_ptr: usize) -> Option<&'static ComponentBatchMeta> {
    let registry = get_component_batch_meta_registry();
    let guard = registry
        .read()
        .expect("Component batch meta registry lock poisoned");
    guard.get(&py_type_ptr).copied()
}

/// Function type for writing a Python message to the ECS custom message system.
/// Registered by the main pybevy crate, called from pybevy_agent.
type MessageWriteFn = dyn Fn(&mut bevy::ecs::world::World, pyo3::Python, &pyo3::Bound<pyo3::PyAny>) -> Result<(), String>
    + Send
    + Sync;

static MESSAGE_WRITE_FN: OnceLock<Box<MessageWriteFn>> = OnceLock::new();

/// Register the function that writes Python messages to ECS CustomMessage slots.
/// Called once at startup by the main pybevy crate.
pub fn register_message_write_fn(
    f: impl Fn(
        &mut bevy::ecs::world::World,
        pyo3::Python,
        &pyo3::Bound<pyo3::PyAny>,
    ) -> Result<(), String>
    + Send
    + Sync
    + 'static,
) {
    let _ = MESSAGE_WRITE_FN.set(Box::new(f));
}

/// Write a Python message instance to the appropriate ECS CustomMessage slot.
/// Returns an error if the message write function hasn't been registered or if the
/// message type isn't registered in the MessageRegistry.
pub fn write_python_message(
    world: &mut bevy::ecs::world::World,
    py: pyo3::Python,
    msg: &pyo3::Bound<pyo3::PyAny>,
) -> Result<(), String> {
    let f = MESSAGE_WRITE_FN
        .get()
        .ok_or("Message write function not registered")?;
    f(world, py, msg)
}

/// Function type for running a Python function as a one-shot ECS system.
/// Registered by the main pybevy crate, called from pybevy_agent.
type RunSystemOnceFn = dyn Fn(&mut bevy::ecs::world::World, pyo3::Python, &pyo3::Bound<pyo3::PyAny>) -> Result<(), String>
    + Send
    + Sync;

static RUN_SYSTEM_ONCE_FN: OnceLock<Box<RunSystemOnceFn>> = OnceLock::new();

/// Register the function that runs a Python function as a one-shot ECS system.
/// Called once at startup by the main pybevy crate.
pub fn register_run_system_once_fn(
    f: impl Fn(
        &mut bevy::ecs::world::World,
        pyo3::Python,
        &pyo3::Bound<pyo3::PyAny>,
    ) -> Result<(), String>
    + Send
    + Sync
    + 'static,
) {
    let _ = RUN_SYSTEM_ONCE_FN.set(Box::new(f));
}

/// Run a Python function as a one-shot ECS system with full param injection.
/// The function's type annotations are used to determine which system params to inject.
pub fn run_system_once(
    world: &mut bevy::ecs::world::World,
    py: pyo3::Python,
    func: &pyo3::Bound<pyo3::PyAny>,
) -> Result<(), String> {
    let f = RUN_SYSTEM_ONCE_FN
        .get()
        .ok_or("run_system_once function not registered")?;
    f(world, py, func)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::{
        any::{Any, TypeId},
        ptr,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
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
    use pyo3::{
        ffi::PyTypeObject,
        prelude::*,
        types::{PyDict, PyInt, PySet, PyType},
    };

    use super::*;
    use crate::{
        AssetBorrowCounter, AssetEventRecord, AssetLoadFailedRecord, ExtractFn,
        FilteredEntityAccess, PreparedBatchComponent, ValidityFlagWithMode,
    };

    struct FakeComponent;

    struct FakeBridge {
        type_id: TypeId,
        ptr: usize,
        name: &'static str,
    }

    fn fake_extract(
        _entity: &mut FilteredEntityAccess,
        _component_id: ComponentId,
        _validity: ValidityFlagWithMode,
        _py: Python,
    ) -> PyResult<Py<PyAny>> {
        unreachable!("fake bridge extract must never run")
    }

    impl ComponentBridge for FakeBridge {
        fn bevy_type_id(&self) -> TypeId {
            self.type_id
        }

        fn py_type_ptr(&self) -> *const PyTypeObject {
            self.ptr as *const PyTypeObject
        }

        fn py_type<'py>(&self, _py: Python<'py>) -> Bound<'py, PyType> {
            unreachable!()
        }

        fn name(&self) -> &'static str {
            self.name
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
            unreachable!()
        }

        fn extract_fn(&self) -> ExtractFn {
            fake_extract
        }

        fn insert(
            &self,
            _world: &mut World,
            _entity: Entity,
            _component: &Bound<PyAny>,
        ) -> PyResult<()> {
            unreachable!()
        }

        fn insert_into_entity(
            &self,
            _entity: &mut EntityWorldMut,
            _component: &Bound<PyAny>,
        ) -> PyResult<()> {
            unreachable!()
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
            unreachable!()
        }

        unsafe fn extract_from_entity_mut(
            &self,
            _entity: Entity,
            _world: *mut World,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!()
        }
    }

    #[test]
    fn test_empty_registry() {
        // Registry should be accessible
        assert!(get_bridge_by_py_type(ptr::null()).is_none());
    }

    #[test]
    fn test_null_pointer_not_found() {
        assert!(!contains_py_type(ptr::null()));
    }

    #[test]
    fn component_registry_registration_lookup_and_alias() {
        let canonical = 0x1001_usize;
        let alias = 0x1002_usize;
        let unknown = 0x9999_usize;
        let null: *const PyTypeObject = ptr::null();

        // A bridge without a real Python type is ignored.
        register_component_bridge_arc(Arc::new(FakeBridge {
            type_id: TypeId::of::<FakeComponent>(),
            ptr: 0,
            name: "Null",
        }));
        assert!(get_bridge_by_py_type(null).is_none());

        // Canonical registration is retrievable by pointer and by TypeId.
        register_component_bridge(FakeBridge {
            type_id: TypeId::of::<FakeComponent>(),
            ptr: canonical,
            name: "Fake",
        });
        assert!(contains_py_type(canonical as *const PyTypeObject));
        assert_eq!(
            get_bridge_by_py_type(canonical as *const PyTypeObject)
                .unwrap()
                .name(),
            "Fake"
        );
        assert_eq!(
            get_type_id_by_py_type(canonical as *const PyTypeObject),
            Some(TypeId::of::<FakeComponent>())
        );

        // Unknown pointers are absent.
        assert!(get_bridge_by_py_type(unknown as *const PyTypeObject).is_none());
        assert!(!contains_py_type(unknown as *const PyTypeObject));

        // Alias registration rejects null and unregistered canonicals, then links.
        assert!(!register_component_bridge_alias(
            null,
            canonical as *const PyTypeObject
        ));
        assert!(!register_component_bridge_alias(
            alias as *const PyTypeObject,
            null
        ));
        assert!(!register_component_bridge_alias(
            alias as *const PyTypeObject,
            unknown as *const PyTypeObject
        ));
        assert!(register_component_bridge_alias(
            alias as *const PyTypeObject,
            canonical as *const PyTypeObject
        ));
        assert_eq!(
            get_bridge_by_py_type(alias as *const PyTypeObject)
                .unwrap()
                .name(),
            "Fake"
        );

        let names: Vec<_> = all_component_bridges()
            .into_iter()
            .map(|bridge| bridge.name())
            .collect();
        assert!(names.contains(&"Fake"));
    }

    #[test]
    fn type_id_registry_round_trips() {
        Python::initialize();
        register_type_id::<PyInt, u32>();
        let ptr = Python::attach(|py| py.get_type::<PyInt>().as_type_ptr());
        assert_eq!(get_type_id_by_py_type(ptr), Some(TypeId::of::<u32>()));
    }

    struct FakeResource {
        type_id: TypeId,
        ptr: usize,
        name: &'static str,
    }

    impl ResourceBridge for FakeResource {
        fn bevy_type_id(&self) -> TypeId {
            self.type_id
        }

        fn py_type_ptr(&self) -> *const PyTypeObject {
            self.ptr as *const PyTypeObject
        }

        fn py_type<'py>(&self, _py: Python<'py>) -> Bound<'py, PyType> {
            unreachable!()
        }

        fn name(&self) -> &'static str {
            self.name
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
            unreachable!()
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
            unreachable!()
        }

        unsafe fn extract_from_entity_mut(
            &self,
            _entity: Entity,
            _world: *mut World,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!()
        }

        fn get(
            &self,
            _world: &World,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Py<PyAny>> {
            unreachable!()
        }

        fn get_mut(
            &self,
            _world: &mut World,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Py<PyAny>> {
            unreachable!()
        }

        fn clone_owned(&self, _world: &World, _py: Python) -> PyResult<Py<PyAny>> {
            unreachable!()
        }

        fn commit_owned(&self, _world: &mut World, _resource: &Bound<PyAny>) -> PyResult<()> {
            unreachable!()
        }

        unsafe fn get_from_cell(
            &self,
            _cell: UnsafeWorldCell<'_>,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Py<PyAny>> {
            unreachable!()
        }

        unsafe fn get_mut_from_cell(
            &self,
            _cell: UnsafeWorldCell<'_>,
            _validity: ValidityFlagWithMode,
            _py: Python,
        ) -> PyResult<Py<PyAny>> {
            unreachable!()
        }

        fn insert(&self, _world: &mut World, _resource: &Bound<PyAny>) -> PyResult<()> {
            unreachable!()
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

    struct FakeMessage {
        type_id: TypeId,
        ptr: usize,
        name: &'static str,
    }

    impl MessageBridge for FakeMessage {
        fn bevy_type_id(&self) -> TypeId {
            self.type_id
        }

        fn py_type_ptr(&self) -> *const PyTypeObject {
            self.ptr as *const PyTypeObject
        }

        fn py_type<'py>(&self, _py: Python<'py>) -> Bound<'py, PyType> {
            unreachable!()
        }

        fn name(&self) -> &'static str {
            self.name
        }

        fn iter_to_python(&self, _py: Python, _world: &mut World) -> PyResult<Vec<Py<PyAny>>> {
            unreachable!()
        }

        fn clear(&self, _world: &mut World) -> PyResult<()> {
            unreachable!()
        }

        fn is_empty(&self, _world: &mut World) -> PyResult<bool> {
            unreachable!()
        }

        fn len(&self, _world: &mut World) -> PyResult<usize> {
            unreachable!()
        }

        fn resource_id(&self, _world: &World) -> Option<ComponentId> {
            None
        }

        fn register_resource_id(&self, _world: &mut World) -> ComponentId {
            ComponentId::new(0)
        }
    }

    struct FakeBatch;

    impl BatchComponent for FakeBatch {
        fn name(&self) -> &'static str {
            "FakeBatch"
        }

        fn component_type_ptr(&self, _py: Python, _batch: &Bound<PyAny>) -> PyResult<usize> {
            unreachable!()
        }

        fn count(&self, _py: Python, _batch: &Bound<PyAny>) -> PyResult<usize> {
            unreachable!()
        }

        fn prepare(
            &self,
            _py: Python,
            _batch: &Bound<PyAny>,
        ) -> PyResult<Box<dyn PreparedBatchComponent>> {
            unreachable!()
        }

        fn insert_bulk(
            &self,
            _py: Python,
            _batch: &Bound<PyAny>,
            _entities: &[Entity],
            _world: &mut World,
        ) -> PyResult<()> {
            unreachable!()
        }
    }

    #[test]
    fn resource_registry_registration_lookup_and_alias() {
        let canonical = 0x2001_usize;
        let alias = 0x2002_usize;
        let canonical_ptr = canonical as *const PyTypeObject;

        register_resource_bridge_arc(Arc::new(FakeResource {
            type_id: TypeId::of::<FakeResource>(),
            ptr: 0,
            name: "RNull",
        }));
        assert!(get_resource_bridge_by_py_type(ptr::null()).is_none());

        register_resource_bridge(FakeResource {
            type_id: TypeId::of::<FakeResource>(),
            ptr: canonical,
            name: "FakeResource",
        });
        assert!(contains_resource_py_type(canonical_ptr));
        assert_eq!(
            get_resource_bridge_by_py_type(canonical_ptr)
                .unwrap()
                .name(),
            "FakeResource"
        );

        assert!(!register_resource_bridge_alias(ptr::null(), canonical_ptr));
        assert!(!register_resource_bridge_alias(
            alias as *const PyTypeObject,
            0x2999 as *const PyTypeObject
        ));
        assert!(register_resource_bridge_alias(
            alias as *const PyTypeObject,
            canonical_ptr
        ));
        assert_eq!(
            get_resource_bridge_by_py_type(alias as *const PyTypeObject)
                .unwrap()
                .name(),
            "FakeResource"
        );

        let names: Vec<_> = all_resource_bridges()
            .into_iter()
            .map(|bridge| bridge.name())
            .collect();
        assert!(names.contains(&"FakeResource"));
    }

    #[test]
    fn message_registry_registration_lookup() {
        let value = 0x3001_usize;
        let ptr = value as *const PyTypeObject;

        register_message_bridge_arc(Arc::new(FakeMessage {
            type_id: TypeId::of::<FakeMessage>(),
            ptr: 0,
            name: "MNull",
        }));
        assert!(get_message_bridge_by_py_type(ptr::null()).is_none());

        register_message_bridge(FakeMessage {
            type_id: TypeId::of::<FakeMessage>(),
            ptr: value,
            name: "FakeMessage",
        });
        assert!(contains_message_py_type(ptr));
        assert_eq!(
            get_message_bridge_by_type_id(TypeId::of::<FakeMessage>())
                .unwrap()
                .name(),
            "FakeMessage"
        );

        let names: Vec<_> = all_message_bridges()
            .into_iter()
            .map(|bridge| bridge.name())
            .collect();
        assert!(names.contains(&"FakeMessage"));
    }

    #[test]
    fn batch_registry_registration_lookup() {
        let value = 0x4001_usize;
        let ptr = value as *const PyTypeObject;

        register_batch_bridge(ptr, Arc::new(FakeBatch));
        assert_eq!(
            get_batch_bridge_by_py_type(ptr).unwrap().name(),
            "FakeBatch"
        );
        assert!(get_batch_bridge_by_py_type(0x4999 as *const PyTypeObject).is_none());
    }

    struct FakeAsset {
        type_id: TypeId,
        ptr: usize,
        name: &'static str,
    }

    impl AssetBridge for FakeAsset {
        fn bevy_type_id(&self) -> TypeId {
            self.type_id
        }

        fn py_type_ptr(&self) -> *const PyTypeObject {
            self.ptr as *const PyTypeObject
        }

        fn py_type<'py>(&self, _py: Python<'py>) -> Bound<'py, PyType> {
            unreachable!()
        }

        fn name(&self) -> &'static str {
            self.name
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
            _cursor: &mut Option<Box<dyn Any + Send + Sync>>,
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
            _cursor: &mut Option<Box<dyn Any + Send + Sync>>,
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
            unreachable!()
        }

        fn get_mut(
            &self,
            _world: UnsafeWorldCell<'_>,
            _id: UntypedAssetId,
            _validity: ValidityFlagWithMode,
            _borrow_counter: AssetBorrowCounter,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!()
        }

        fn add(
            &self,
            _world: &mut World,
            _asset: &Bound<PyAny>,
            _py: Python,
        ) -> PyResult<UntypedHandle> {
            unreachable!()
        }

        fn remove(&self, _world: &mut World, _id: UntypedAssetId) -> PyResult<bool> {
            unreachable!()
        }

        fn remove_and_return(
            &self,
            _world: &mut World,
            _id: UntypedAssetId,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!()
        }

        fn len(&self, _world: &World) -> PyResult<usize> {
            unreachable!()
        }

        fn contains(&self, _world: &World, _id: UntypedAssetId) -> PyResult<bool> {
            unreachable!()
        }

        fn iter_pairs(
            &self,
            _world: &World,
            _validity: ValidityFlagWithMode,
            _borrow_counter: AssetBorrowCounter,
            _py: Python,
        ) -> PyResult<Vec<(UntypedAssetId, Py<PyAny>)>> {
            unreachable!()
        }

        fn load(&self, _server: &AssetServer, _path: AssetPath<'_>) -> UntypedHandle {
            unreachable!()
        }

        fn get_handle(&self, _server: &AssetServer, _path: AssetPath<'_>) -> Option<UntypedHandle> {
            None
        }

        fn clear_programmatic(&self, _world: &mut World, _verbose: bool) {}
    }

    #[test]
    fn asset_registry_registration_lookup() {
        let value = 0x5001_usize;
        let ptr = value as *const PyTypeObject;

        register_asset_bridge_arc(Arc::new(FakeAsset {
            type_id: TypeId::of::<FakeAsset>(),
            ptr: 0,
            name: "ANull",
        }));
        assert!(get_asset_bridge_by_py_type(ptr::null()).is_none());

        register_asset_bridge(FakeAsset {
            type_id: TypeId::of::<FakeAsset>(),
            ptr: value,
            name: "FakeAsset",
        });
        assert!(contains_asset_py_type(ptr));
        assert_eq!(
            get_asset_bridge_by_py_type(ptr).unwrap().name(),
            "FakeAsset"
        );
        assert_eq!(
            get_asset_bridge_by_type_id(TypeId::of::<FakeAsset>())
                .unwrap()
                .name(),
            "FakeAsset"
        );
        assert_eq!(
            get_asset_bridge_by_name("FakeAsset").unwrap().name(),
            "FakeAsset"
        );
        assert!(get_asset_bridge_by_name("MissingAsset").is_none());
    }

    static MESSAGE_WRITE_CALLED: AtomicBool = AtomicBool::new(false);
    static RUN_SYSTEM_ONCE_CALLED: AtomicBool = AtomicBool::new(false);

    #[test]
    fn registered_message_write_and_system_once_fns_are_invoked() {
        Python::initialize();
        register_message_write_fn(|_world, _py, _message| {
            MESSAGE_WRITE_CALLED.store(true, Ordering::SeqCst);
            Ok(())
        });
        register_run_system_once_fn(|_world, _py, _function| {
            RUN_SYSTEM_ONCE_CALLED.store(true, Ordering::SeqCst);
            Ok(())
        });

        Python::attach(|py| {
            let mut world = World::new();
            let payload = py.None();
            write_python_message(&mut world, py, payload.bind(py)).unwrap();
            run_system_once(&mut world, py, payload.bind(py)).unwrap();
        });

        assert!(MESSAGE_WRITE_CALLED.load(Ordering::SeqCst));
        assert!(RUN_SYSTEM_ONCE_CALLED.load(Ordering::SeqCst));
    }

    fn fake_batch_insert(
        _py: Python<'_>,
        _batch: &PyRustComponentBatch,
        _entities: &[Entity],
        _world: &mut World,
    ) -> PyResult<()> {
        Ok(())
    }

    fn fake_batch_prepare(
        _py: Python<'_>,
        _batch: &PyRustComponentBatch,
    ) -> PyResult<Box<dyn PreparedBatchComponent>> {
        unreachable!()
    }

    #[test]
    fn component_batch_meta_round_trips() {
        static META: ComponentBatchMeta = ComponentBatchMeta {
            component_name: "FakeBatchComponent",
            fields: &[],
            insert_fn: fake_batch_insert,
            prepare_fn: fake_batch_prepare,
        };

        register_component_batch_meta(0x6001, &META);
        assert_eq!(
            get_component_batch_meta(0x6001).unwrap().component_name,
            "FakeBatchComponent"
        );
        assert!(get_component_batch_meta(0x6999).is_none());
    }

    // Routing-batch dummies: distinct types so key->value identity is checked
    // against a second registered type, never against the key's own value.
    struct RoutingCompA;
    struct RoutingCompB;
    struct RoutingCompOld;
    struct RoutingCompNew;
    struct RoutingAssetA;
    struct RoutingAssetB;
    struct RoutingBridgeWins;
    struct RoutingTypeIdFallback;
    struct RoutingTypeIdFallbackTwo;

    fn routing_component_bridge(type_id: TypeId, ptr: usize, name: &'static str) -> FakeBridge {
        FakeBridge { type_id, ptr, name }
    }

    #[test]
    fn two_distinct_component_types_keep_separate_keys() {
        let a_ptr: *const PyTypeObject = 0x8101_usize as *const PyTypeObject;
        let b_ptr: *const PyTypeObject = 0x8102_usize as *const PyTypeObject;

        register_component_bridge(routing_component_bridge(
            TypeId::of::<RoutingCompA>(),
            0x8101,
            "RoutingComponentA",
        ));
        register_component_bridge(routing_component_bridge(
            TypeId::of::<RoutingCompB>(),
            0x8102,
            "RoutingComponentB",
        ));

        assert_eq!(
            get_bridge_by_py_type(a_ptr).unwrap().name(),
            "RoutingComponentA"
        );
        assert_eq!(
            get_bridge_by_py_type(b_ptr).unwrap().name(),
            "RoutingComponentB"
        );
        assert!(contains_py_type(a_ptr));
        assert!(contains_py_type(b_ptr));
        assert!(!contains_py_type(0x8199 as *const PyTypeObject));
        assert!(get_bridge_by_py_type(0x8199 as *const PyTypeObject).is_none());

        assert_eq!(
            get_type_id_by_py_type(a_ptr),
            Some(TypeId::of::<RoutingCompA>())
        );
        assert_eq!(
            get_type_id_by_py_type(b_ptr),
            Some(TypeId::of::<RoutingCompB>())
        );

        let names: Vec<&str> = all_component_bridges()
            .iter()
            .map(|bridge| bridge.name())
            .collect();
        assert_eq!(
            names
                .iter()
                .filter(|name| **name == "RoutingComponentA")
                .count(),
            1
        );
        assert_eq!(
            names
                .iter()
                .filter(|name| **name == "RoutingComponentB")
                .count(),
            1
        );
    }

    #[test]
    fn alias_registration_deduplicates_and_retargets() {
        let canonical: *const PyTypeObject = 0x8201_usize as *const PyTypeObject;
        let alias: *const PyTypeObject = 0x8202_usize as *const PyTypeObject;
        let other: *const PyTypeObject = 0x8203_usize as *const PyTypeObject;

        register_component_bridge(routing_component_bridge(
            TypeId::of::<RoutingCompA>(),
            0x8201,
            "RoutingAliasCanonical",
        ));
        let before: Vec<&str> = all_component_bridges()
            .iter()
            .map(|bridge| bridge.name())
            .collect();
        assert_eq!(
            before
                .iter()
                .filter(|name| **name == "RoutingAliasCanonical")
                .count(),
            1
        );

        assert!(register_component_bridge_alias(alias, canonical));
        let by_alias = get_bridge_by_py_type(alias).unwrap();
        let by_canonical = get_bridge_by_py_type(canonical).unwrap();
        assert!(Arc::ptr_eq(&by_alias, &by_canonical));
        let after: Vec<&str> = all_component_bridges()
            .iter()
            .map(|bridge| bridge.name())
            .collect();
        assert_eq!(
            after
                .iter()
                .filter(|name| **name == "RoutingAliasCanonical")
                .count(),
            1,
            "an alias shares its canonical bridge and adds no new enumeration entry"
        );

        register_component_bridge(routing_component_bridge(
            TypeId::of::<RoutingCompB>(),
            0x8203,
            "RoutingAliasSecondCanonical",
        ));
        assert!(register_component_bridge_alias(alias, other));
        assert_eq!(
            get_bridge_by_py_type(alias).unwrap().name(),
            "RoutingAliasSecondCanonical",
            "re-aliasing retargets the alias at the new canonical bridge"
        );
    }

    #[test]
    fn component_reregistration_replaces_the_stored_bridge() {
        let ptr: *const PyTypeObject = 0x8301_usize as *const PyTypeObject;

        register_component_bridge(routing_component_bridge(
            TypeId::of::<RoutingCompOld>(),
            0x8301,
            "RoutingReplacedOld",
        ));
        assert_eq!(
            get_bridge_by_py_type(ptr).unwrap().name(),
            "RoutingReplacedOld"
        );

        register_component_bridge(routing_component_bridge(
            TypeId::of::<RoutingCompNew>(),
            0x8301,
            "RoutingReplacedNew",
        ));
        assert_eq!(
            get_bridge_by_py_type(ptr).unwrap().name(),
            "RoutingReplacedNew",
            "a re-registration under the same pointer stores the new bridge"
        );
        assert_eq!(
            get_type_id_by_py_type(ptr),
            Some(TypeId::of::<RoutingCompNew>())
        );
    }

    #[test]
    fn type_id_lookup_prefers_bridges_then_falls_back_to_typeid_registry() {
        Python::initialize();
        Python::attach(|py| {
            let dict_ptr = py.get_type::<PyDict>().as_type_ptr();
            let set_ptr = py.get_type::<PySet>().as_type_ptr();

            register_component_bridge(routing_component_bridge(
                TypeId::of::<RoutingBridgeWins>(),
                dict_ptr as usize,
                "RoutingBridgeWins",
            ));
            register_type_id::<PyDict, RoutingTypeIdFallback>();
            assert_eq!(
                get_type_id_by_py_type(dict_ptr),
                Some(TypeId::of::<RoutingBridgeWins>()),
                "a bridge-registered type answers from the bridge registry, not the TypeId registry"
            );

            register_type_id::<PySet, RoutingTypeIdFallbackTwo>();
            assert!(get_bridge_by_py_type(set_ptr).is_none());
            assert_eq!(
                get_type_id_by_py_type(set_ptr),
                Some(TypeId::of::<RoutingTypeIdFallbackTwo>()),
                "a type known only to the TypeId registry resolves through the fallback"
            );

            register_type_id::<PySet, RoutingBridgeWins>();
            assert_eq!(
                get_type_id_by_py_type(set_ptr),
                Some(TypeId::of::<RoutingBridgeWins>()),
                "re-registering a TypeId entry replaces the stored TypeId"
            );

            assert!(get_type_id_by_py_type(ptr::null()).is_none());
        });
    }

    #[test]
    fn two_distinct_asset_types_keep_separate_name_and_typeid_keys() {
        let a_ptr: *const PyTypeObject = 0x8501_usize as *const PyTypeObject;
        let b_ptr: *const PyTypeObject = 0x8502_usize as *const PyTypeObject;

        register_asset_bridge(FakeAsset {
            type_id: TypeId::of::<RoutingAssetA>(),
            ptr: 0x8501,
            name: "RoutingAssetA",
        });
        register_asset_bridge(FakeAsset {
            type_id: TypeId::of::<RoutingAssetB>(),
            ptr: 0x8502,
            name: "RoutingAssetB",
        });

        assert_eq!(
            get_asset_bridge_by_py_type(a_ptr).unwrap().name(),
            "RoutingAssetA"
        );
        assert_eq!(
            get_asset_bridge_by_py_type(b_ptr).unwrap().name(),
            "RoutingAssetB"
        );
        assert_eq!(
            get_asset_bridge_by_name("RoutingAssetA").unwrap().name(),
            "RoutingAssetA"
        );
        assert_eq!(
            get_asset_bridge_by_name("RoutingAssetB").unwrap().name(),
            "RoutingAssetB"
        );
        assert!(get_asset_bridge_by_name("RoutingAssetMissing").is_none());
        assert_eq!(
            get_asset_bridge_by_type_id(TypeId::of::<RoutingAssetA>())
                .unwrap()
                .name(),
            "RoutingAssetA"
        );
        assert_eq!(
            get_asset_bridge_by_type_id(TypeId::of::<RoutingAssetB>())
                .unwrap()
                .name(),
            "RoutingAssetB"
        );
        assert!(get_asset_bridge_by_type_id(TypeId::of::<RoutingCompA>()).is_none());
    }
}
