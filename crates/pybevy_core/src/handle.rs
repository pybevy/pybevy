//! Asset handle wrapper for PyBevy
//!
//! This module provides `PyHandle`, a Python wrapper for Bevy's `Handle<T>`.
//! It uses a dynamic dispatch pattern via `AssetBridge` to support asset types
//! from both the main crate and feature crates.
//!
//! # Design
//!
//! PyHandle stores:
//! - `kind: HandleKind` - Strong (keeps asset alive) or WeakUuid (reference only)
//! - `type_ptr: *const PyTypeObject` - Python type pointer for asset type lookup
//!
//! All type operations (TypeId lookup, Python type conversion) delegate to
//! the global `AssetBridge` registry.

use core::marker::PhantomData;
use std::any::TypeId;

use bevy::asset::{Asset, AssetId, Handle, UntypedAssetId, UntypedHandle};
use pyo3::{
    IntoPyObjectExt, exceptions::PyValueError, ffi, ffi::PyTypeObject, prelude::*, types::PyType,
};
use uuid::Uuid;

use crate::registry::global_registry;

/// Handle variant that directly maps to Bevy's Handle enum.
///
/// This design ensures no invalid states are possible:
/// - Strong handles keep assets alive and contain the AssetId internally
/// - Weak handles are UUID-only (Index-based weak handles don't exist in Bevy)
#[derive(Debug, Clone)]
enum HandleKind {
    /// Strong handle - keeps asset alive via UntypedHandle.
    /// We store UntypedHandle to access the .id() method.
    Strong(UntypedHandle),

    /// Weak UUID handle - does not keep asset alive.
    /// Only UUID-based weak references are valid in Bevy's asset system.
    WeakUuid(Uuid),
}

#[pyclass(name = "Handle", subclass, frozen, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyHandle {
    kind: HandleKind,
    type_ptr: *const PyTypeObject,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for PyHandle {}
unsafe impl Sync for PyHandle {}

impl PartialEq for PyHandle {
    fn eq(&self, other: &Self) -> bool {
        if self.type_ptr != other.type_ptr {
            return false;
        }

        // Compare by the underlying asset ID
        match (&self.kind, &other.kind) {
            (HandleKind::Strong(a), HandleKind::Strong(b)) => a.id() == b.id(),
            (HandleKind::WeakUuid(a), HandleKind::WeakUuid(b)) => a == b,
            (HandleKind::Strong(strong), HandleKind::WeakUuid(uuid))
            | (HandleKind::WeakUuid(uuid), HandleKind::Strong(strong)) => {
                // Compare strong handle's ID with weak UUID
                match strong.id() {
                    UntypedAssetId::Uuid {
                        uuid: strong_uuid, ..
                    } => strong_uuid == *uuid,
                    UntypedAssetId::Index { index, .. } => {
                        // Check if the weak UUID is a synthetic one from an index
                        if is_synthetic_index_uuid(uuid) {
                            index.to_bits() == extract_index_from_synthetic_uuid(uuid)
                        } else {
                            false
                        }
                    }
                }
            }
        }
    }
}

/// Marker for synthetic UUIDs created from index-based AssetIds
const SYNTHETIC_UUID_MARKER: u128 = 0xDEAD_BEEF_0000_0000_u128 << 64;
const SYNTHETIC_UUID_MASK: u128 = 0xFFFF_FFFF_0000_0000_u128 << 64;

/// Check if a UUID is a synthetic one created from an index
fn is_synthetic_index_uuid(uuid: &Uuid) -> bool {
    (uuid.as_u128() & SYNTHETIC_UUID_MASK) == SYNTHETIC_UUID_MARKER
}

/// Extract the index bits from a synthetic UUID
fn extract_index_from_synthetic_uuid(uuid: &Uuid) -> u64 {
    (uuid.as_u128() & 0xFFFF_FFFF_FFFF_FFFF) as u64
}

impl<A: Asset> TryFrom<PyHandle> for Handle<A> {
    type Error = PyErr;

    fn try_from(handle: PyHandle) -> Result<Self, Self::Error> {
        Self::try_from(&handle)
    }
}

impl<A: Asset> TryFrom<&PyHandle> for Handle<A> {
    type Error = PyErr;

    fn try_from(handle: &PyHandle) -> Result<Self, Self::Error> {
        // Verify asset type matches
        let expected_type_id = TypeId::of::<A>();
        let actual_type_id = handle.asset_type_id();

        if actual_type_id != Some(expected_type_id) {
            let full_type_name = std::any::type_name::<A>();
            let short_type_name = full_type_name.rsplit("::").next().unwrap_or(full_type_name);
            let actual_name = handle.asset_type_name().unwrap_or("Unknown");
            return Err(PyValueError::new_err(format!(
                "Handle of type `{}` does not match expected type `{}`",
                actual_name, short_type_name
            )));
        }

        // Convert based on handle kind
        match &handle.kind {
            HandleKind::Strong(untyped) => Ok(untyped.clone().typed::<A>()),
            HandleKind::WeakUuid(uuid) => Ok(Handle::Uuid(*uuid, PhantomData)),
        }
    }
}

impl<A: Asset> From<Handle<A>> for PyHandle {
    fn from(handle: Handle<A>) -> Self {
        let type_ptr = Python::attach(|_py| {
            // Look up the Python type for this asset via bridge
            if let Some(bridge) = global_registry::get_asset_bridge_by_type_id(TypeId::of::<A>()) {
                bridge.py_type_ptr()
            } else {
                panic!(
                    "No AssetBridge registered for type {}. Add `bridge` to #[pyasset] attribute.",
                    std::any::type_name::<A>()
                )
            }
        });

        match &handle {
            Handle::Strong(_) => PyHandle {
                kind: HandleKind::Strong(handle.untyped()),
                type_ptr,
            },
            Handle::Uuid(uuid, _) => PyHandle {
                kind: HandleKind::WeakUuid(*uuid),
                type_ptr,
            },
        }
    }
}

impl<A: Asset> From<&Handle<A>> for PyHandle {
    fn from(handle: &Handle<A>) -> Self {
        let type_ptr = Python::attach(|_py| {
            if let Some(bridge) = global_registry::get_asset_bridge_by_type_id(TypeId::of::<A>()) {
                bridge.py_type_ptr()
            } else {
                panic!(
                    "No AssetBridge registered for type {}. Add `bridge` to #[pyasset] attribute.",
                    std::any::type_name::<A>()
                )
            }
        });

        match handle {
            Handle::Strong(_) => PyHandle {
                kind: HandleKind::Strong(handle.clone().untyped()),
                type_ptr,
            },
            Handle::Uuid(uuid, _) => PyHandle {
                kind: HandleKind::WeakUuid(*uuid),
                type_ptr,
            },
        }
    }
}

impl<A: Asset> From<&AssetId<A>> for PyHandle {
    /// Convert an AssetId to a weak PyHandle.
    ///
    /// Note: This creates a weak handle. For Index-based AssetIds, this creates
    /// a handle that cannot keep the asset alive.
    fn from(id: &AssetId<A>) -> Self {
        let type_ptr = Python::attach(|_py| {
            if let Some(bridge) = global_registry::get_asset_bridge_by_type_id(TypeId::of::<A>()) {
                bridge.py_type_ptr()
            } else {
                panic!(
                    "No AssetBridge registered for type {}. Add `bridge` to #[pyasset] attribute.",
                    std::any::type_name::<A>()
                )
            }
        });

        match id {
            AssetId::Index { index, .. } => {
                // Create a deterministic UUID from the index bits
                let index_bits = index.to_bits();
                let synthetic_uuid = Uuid::from_u128(SYNTHETIC_UUID_MARKER | index_bits as u128);
                PyHandle {
                    kind: HandleKind::WeakUuid(synthetic_uuid),
                    type_ptr,
                }
            }
            AssetId::Uuid { uuid } => PyHandle {
                kind: HandleKind::WeakUuid(*uuid),
                type_ptr,
            },
        }
    }
}

impl TryFrom<&UntypedHandle> for PyHandle {
    type Error = PyErr;

    fn try_from(handle: &UntypedHandle) -> Result<Self, Self::Error> {
        let type_id = handle.type_id();
        let type_ptr = global_registry::get_asset_bridge_by_type_id(type_id)
            .map(|b| b.py_type_ptr())
            .ok_or_else(|| {
                PyValueError::new_err(format!("Unknown asset type with TypeId: {:?}", type_id))
            })?;

        match handle {
            UntypedHandle::Strong(_) => Ok(PyHandle {
                kind: HandleKind::Strong(handle.clone()),
                type_ptr,
            }),
            UntypedHandle::Uuid { uuid, .. } => Ok(PyHandle {
                kind: HandleKind::WeakUuid(*uuid),
                type_ptr,
            }),
        }
    }
}

impl From<&PyHandle> for UntypedAssetId {
    fn from(handle: &PyHandle) -> Self {
        match &handle.kind {
            HandleKind::Strong(untyped) => untyped.id(),
            HandleKind::WeakUuid(uuid) => UntypedAssetId::Uuid {
                type_id: handle
                    .asset_type_id()
                    .expect("Weak UUID handles require a registered asset type"),
                uuid: *uuid,
            },
        }
    }
}

impl From<PyHandle> for UntypedAssetId {
    fn from(handle: PyHandle) -> Self {
        UntypedAssetId::from(&handle)
    }
}

#[pymethods]
impl PyHandle {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let _ = key;
        cls.into_py_any(cls.py())
    }

    /// Create a weak handle from a UUID.
    ///
    /// This creates a handle that does NOT keep the asset alive.
    ///
    /// # Arguments
    /// * `value` - A u128 value to use as the UUID
    /// * `asset_type` - The Python type of the asset (e.g., `Mesh`, `Image`)
    #[staticmethod]
    pub fn weak_from_u128<'py>(
        py: Python,
        value: u128,
        asset_type: Bound<'py, PyType>,
    ) -> PyResult<Py<Self>> {
        let type_ptr = asset_type.as_type_ptr();

        // Verify it's a registered asset type
        if !global_registry::contains_asset_py_type(type_ptr) {
            return Err(PyValueError::new_err(format!(
                "Unknown asset type: {:?}. Asset type must be registered.",
                asset_type.name()?
            )));
        }

        Py::new(
            py,
            PyHandle {
                kind: HandleKind::WeakUuid(Uuid::from_u128(value)),
                type_ptr,
            },
        )
    }

    /// Get the Python type of the asset this handle refers to.
    pub fn asset_type_class(&self) -> PyResult<Py<PyType>> {
        Ok(Python::attach(|py| {
            if let Some(bridge) = global_registry::get_asset_bridge_by_py_type(self.type_ptr) {
                bridge.py_type(py).unbind()
            } else {
                // Return the stored type pointer directly
                unsafe {
                    Bound::from_borrowed_ptr(py, self.type_ptr as *mut ffi::PyObject)
                        .cast_into_unchecked::<PyType>()
                        .unbind()
                }
            }
        }))
    }

    /// Get a unique identifier for this handle.
    ///
    /// For UUID-based handles, returns the UUID as u128.
    /// For Index-based strong handles, returns the index bits.
    pub fn id(&self) -> u128 {
        match &self.kind {
            HandleKind::Strong(untyped) => match untyped.id() {
                UntypedAssetId::Index { index, .. } => index.to_bits() as u128,
                UntypedAssetId::Uuid { uuid, .. } => uuid.as_u128(),
            },
            HandleKind::WeakUuid(uuid) => uuid.as_u128(),
        }
    }

    /// Check if this is a strong handle (keeps the asset alive).
    pub fn is_strong(&self) -> bool {
        matches!(self.kind, HandleKind::Strong(_))
    }

    /// Check if this is a weak handle (does not keep the asset alive).
    pub fn is_weak(&self) -> bool {
        matches!(self.kind, HandleKind::WeakUuid(_))
    }

    fn __repr__(&self) -> String {
        let kind_str = match &self.kind {
            HandleKind::Strong(untyped) => {
                let id = untyped.id();
                match id {
                    UntypedAssetId::Index { index, .. } => {
                        format!("Strong(index={})", index.to_bits())
                    }
                    UntypedAssetId::Uuid { uuid, .. } => format!("Strong(uuid={})", uuid),
                }
            }
            HandleKind::WeakUuid(uuid) => format!("Weak(uuid={})", uuid),
        };
        let type_name = self.asset_type_name().unwrap_or("Unknown");
        format!("Handle[{}]({})", type_name, kind_str)
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.id().hash(&mut hasher);
        self.type_ptr.hash(&mut hasher);
        hasher.finish()
    }

    /// Compare handles across different PyHandle types.
    ///
    /// This supports comparison with:
    /// - Same PyHandle type (using Rust PartialEq)
    /// - Different PyHandle types from different crates (via duck-typing)
    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: pyo3::pyclass::CompareOp,
    ) -> PyResult<Py<PyAny>> {
        use pyo3::pyclass::CompareOp;

        let py = other.py();

        // Get comparison values from other object via duck-typing
        let other_id = match other.call_method0("id") {
            Ok(val) => match val.extract::<u128>() {
                Ok(id) => id,
                Err(_) => return py.NotImplemented().into_py_any(py),
            },
            Err(_) => return py.NotImplemented().into_py_any(py),
        };

        let other_type_class = match other.call_method0("asset_type_class") {
            Ok(val) => val,
            Err(_) => return py.NotImplemented().into_py_any(py),
        };

        // Get self's comparison values
        let self_id = self.id();
        let self_type_class = self.asset_type_class()?;

        // Compare type classes first
        let types_equal = self_type_class.bind(py).eq(&other_type_class)?;

        // Normalize IDs to handle synthetic UUID comparison
        // Synthetic UUIDs have the marker 0xDEAD_BEEF in the high bits
        let self_normalized = normalize_handle_id(self_id);
        let other_normalized = normalize_handle_id(other_id);
        let ids_equal = self_normalized == other_normalized;

        let result = match op {
            CompareOp::Eq => types_equal && ids_equal,
            CompareOp::Ne => !types_equal || !ids_equal,
            _ => return py.NotImplemented().into_py_any(py),
        };

        result.into_py_any(py)
    }
}

/// Normalize a handle ID for comparison.
///
/// Synthetic UUIDs (created from index-based AssetIds) have the marker
/// 0xDEAD_BEEF in the high bits. We extract the index to compare with
/// strong index-based handles.
fn normalize_handle_id(id: u128) -> u128 {
    let uuid = Uuid::from_u128(id);
    if is_synthetic_index_uuid(&uuid) {
        extract_index_from_synthetic_uuid(&uuid) as u128
    } else {
        id
    }
}

impl PyHandle {
    /// Get the Rust TypeId for this handle's asset type.
    pub fn asset_type_id(&self) -> Option<TypeId> {
        global_registry::get_asset_bridge_by_py_type(self.type_ptr).map(|b| b.bevy_type_id())
    }

    /// Get the name of the asset type.
    pub fn asset_type_name(&self) -> Option<&'static str> {
        global_registry::get_asset_bridge_by_py_type(self.type_ptr).map(|b| b.name())
    }

    /// Get the Python type pointer.
    pub fn type_ptr(&self) -> *const PyTypeObject {
        self.type_ptr
    }

    /// Create a default (invalid) handle for the given Python type.
    ///
    /// This matches Bevy's `Handle::default()` which creates an invalid handle
    /// that can be used as a placeholder. The handle uses a nil UUID.
    pub fn default_for_type(type_ptr: *const PyTypeObject) -> Self {
        PyHandle {
            kind: HandleKind::WeakUuid(Uuid::nil()),
            type_ptr,
        }
    }

    /// Create a PyHandle from a type pointer and UntypedHandle.
    ///
    /// Used by AssetBridge implementations to create handles with known type info.
    pub fn from_untyped(handle: UntypedHandle, type_ptr: *const PyTypeObject) -> Self {
        match &handle {
            UntypedHandle::Strong(_) => PyHandle {
                kind: HandleKind::Strong(handle),
                type_ptr,
            },
            UntypedHandle::Uuid { uuid, .. } => PyHandle {
                kind: HandleKind::WeakUuid(*uuid),
                type_ptr,
            },
        }
    }

    /// Convert to an UntypedHandle.
    ///
    /// For strong handles, returns the underlying UntypedHandle directly.
    /// For weak UUID handles, constructs an UntypedHandle::Uuid using the
    /// asset bridge's TypeId for proper Bevy asset lookup.
    ///
    /// # Errors
    /// Returns an error if the handle's asset type is not registered.
    pub fn to_untyped_handle(&self) -> PyResult<UntypedHandle> {
        match &self.kind {
            HandleKind::Strong(untyped) => Ok(untyped.clone()),
            HandleKind::WeakUuid(uuid) => {
                let bridge = global_registry::get_asset_bridge_by_py_type(self.type_ptr)
                    .ok_or_else(|| {
                        pyo3::exceptions::PyRuntimeError::new_err(
                            "Cannot convert weak handle: asset type not registered",
                        )
                    })?;
                Ok(UntypedHandle::Uuid {
                    uuid: *uuid,
                    type_id: bridge.bevy_type_id(),
                })
            }
        }
    }

    /// Check if this handle references a strong UntypedHandle.
    pub fn has_strong_handle(&self) -> bool {
        matches!(self.kind, HandleKind::Strong(_))
    }

    /// Get the UntypedHandle if this is a strong handle, None otherwise.
    pub fn try_untyped_handle(&self) -> Option<&UntypedHandle> {
        match &self.kind {
            HandleKind::Strong(h) => Some(h),
            HandleKind::WeakUuid(_) => None,
        }
    }
}

/// Extract a PyHandle from a Python object.
///
/// With the unified PyHandle type, this is simply a direct extraction.
pub fn extract_handle_from_any(obj: &Bound<'_, PyAny>) -> PyResult<PyHandle> {
    Ok(obj.extract::<PyHandle>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_uuid() {
        let uuid = Uuid::from_u128(SYNTHETIC_UUID_MARKER | 12345);
        assert!(is_synthetic_index_uuid(&uuid));
        assert_eq!(extract_index_from_synthetic_uuid(&uuid), 12345);
    }

    #[test]
    fn test_non_synthetic_uuid() {
        let uuid = Uuid::from_u128(12345);
        assert!(!is_synthetic_index_uuid(&uuid));
    }

    #[test]
    fn synthetic_uuid_zero_index() {
        let uuid = Uuid::from_u128(SYNTHETIC_UUID_MARKER);
        assert!(is_synthetic_index_uuid(&uuid));
        assert_eq!(extract_index_from_synthetic_uuid(&uuid), 0);
    }

    #[test]
    fn synthetic_uuid_max_index() {
        let max_u64 = u64::MAX;
        let uuid = Uuid::from_u128(SYNTHETIC_UUID_MARKER | max_u64 as u128);
        assert!(is_synthetic_index_uuid(&uuid));
        assert_eq!(extract_index_from_synthetic_uuid(&uuid), max_u64);
    }

    #[test]
    fn nil_uuid_not_synthetic() {
        let uuid = Uuid::nil();
        assert!(!is_synthetic_index_uuid(&uuid));
    }

    #[test]
    fn normalize_strips_synthetic_marker() {
        let index: u64 = 42;
        let synthetic = SYNTHETIC_UUID_MARKER | index as u128;
        assert_eq!(normalize_handle_id(synthetic), 42);
    }

    #[test]
    fn normalize_preserves_regular_uuid() {
        let regular: u128 = 0x1234_5678_9ABC_DEF0;
        assert_eq!(normalize_handle_id(regular), regular);
    }

    #[test]
    fn normalize_idempotent_for_plain_index() {
        let plain: u128 = 7;
        assert_eq!(normalize_handle_id(plain), 7);
    }
}
