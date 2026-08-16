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
/// Used by convenience methods like `load_scene()`, `load_image()`, etc.
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
/// - Main crate native_component! macros
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
mod tests {
    use std::ptr;

    use super::*;

    // Note: Can't easily test registration without a real ComponentBridge impl
    // These are basic sanity tests

    #[test]
    fn test_empty_registry() {
        // Registry should be accessible
        assert!(get_bridge_by_py_type(ptr::null()).is_none());
    }

    #[test]
    fn test_null_pointer_not_found() {
        assert!(!contains_py_type(ptr::null()));
    }
}
