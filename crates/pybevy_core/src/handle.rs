//! Asset handle wrapper for PyBevy
//!
//! This module provides `PyHandle`, a Python wrapper for Bevy's `Handle<T>`.
//! It uses a dynamic dispatch pattern via `AssetBridge` to support asset types
//! from both the main crate and feature crates.
//!
//! # Design
//!
//! PyHandle stores:
//! - `kind: HandleKind` - Strong (keeps asset alive) or Uuid (reference only)
//! - `type_ptr: *const PyTypeObject` - Python type pointer for asset type lookup
//!
//! All type operations (TypeId lookup, Python type conversion) delegate to
//! the global `AssetBridge` registry.

use core::marker::PhantomData;
use std::{
    any::TypeId,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use bevy::asset::{Asset, Handle, UntypedAssetId, UntypedHandle};
use pyo3::{
    IntoPyObjectExt,
    exceptions::{PyTypeError, PyValueError},
    ffi,
    ffi::PyTypeObject,
    prelude::*,
    types::PyType,
};
use uuid::Uuid;

use crate::{
    LogicalTypeId, PyAssetId, materialize_asset_id, public_error, registry::global_registry,
};

/// Handle variant that directly maps to Bevy's Handle enum.
///
/// This design ensures no invalid states are possible:
/// - Strong handles keep assets alive and contain the AssetId internally
/// - UUID handles are non-owning stable identifiers
#[derive(Debug, Clone)]
enum HandleKind {
    /// Strong handle - keeps asset alive via UntypedHandle.
    /// We store UntypedHandle to access the .id() method.
    Strong(UntypedHandle),

    /// UUID handle - does not keep asset alive.
    Uuid(Uuid),
}

#[pyclass(name = "Handle", subclass, frozen, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyHandle {
    kind: HandleKind,
    type_ptr: *const PyTypeObject,
    logical_type_id: Option<LogicalTypeId>,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for PyHandle {}
unsafe impl Sync for PyHandle {}

impl PartialEq for PyHandle {
    fn eq(&self, other: &Self) -> bool {
        if self.type_ptr != other.type_ptr || self.logical_type_id != other.logical_type_id {
            return false;
        }

        self.same_asset_id(other)
    }
}

impl<A: Asset> TryFrom<PyHandle> for Handle<A> {
    type Error = PyErr;

    fn try_from(handle: PyHandle) -> Result<Self, Self::Error> {
        Self::try_from(&handle)
    }
}

/// The asset type name bevy reports, without module path or generic arguments,
/// so `ForwardDecalMaterial<StandardMaterial>` reads as `ForwardDecalMaterial`.
fn short_asset_name<A: ?Sized>() -> &'static str {
    let full = std::any::type_name::<A>();
    let base = full.split('<').next().unwrap_or(full);
    base.rsplit("::").next().unwrap_or(base)
}

/// Reject a handle that does not carry asset type `A`.
///
/// Call this where the handle is supplied so the mismatch is reported there,
/// rather than later wherever the typed handle happens to be produced. Compares
/// the registered `TypeId`: an unregistered handle reports no type at all and
/// must not be waved through.
pub fn ensure_asset_type<A: Asset>(handle: &PyHandle) -> PyResult<()> {
    if handle.asset_type_id() != Some(TypeId::of::<A>()) {
        return Err(PyTypeError::new_err(public_error::asset_type_mismatch(
            handle.asset_type_name().unwrap_or("Unknown"),
            short_asset_name::<A>(),
        )));
    }
    Ok(())
}

impl<A: Asset> TryFrom<&PyHandle> for Handle<A> {
    type Error = PyErr;

    fn try_from(handle: &PyHandle) -> Result<Self, Self::Error> {
        ensure_asset_type::<A>(handle)?;

        // Convert based on handle kind
        match &handle.kind {
            HandleKind::Strong(untyped) => Ok(untyped.clone().typed::<A>()),
            HandleKind::Uuid(uuid) => Ok(Handle::Uuid(*uuid, PhantomData)),
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
                logical_type_id: None,
            },
            Handle::Uuid(uuid, _) => PyHandle {
                kind: HandleKind::Uuid(*uuid),
                type_ptr,
                logical_type_id: None,
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
                logical_type_id: None,
            },
            Handle::Uuid(uuid, _) => PyHandle {
                kind: HandleKind::Uuid(*uuid),
                type_ptr,
                logical_type_id: None,
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
                logical_type_id: None,
            }),
            UntypedHandle::Uuid { uuid, .. } => Ok(PyHandle {
                kind: HandleKind::Uuid(*uuid),
                type_ptr,
                logical_type_id: None,
            }),
        }
    }
}

impl From<&PyHandle> for UntypedAssetId {
    fn from(handle: &PyHandle) -> Self {
        handle.untyped_id()
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

    /// Create a non-owning UUID handle.
    ///
    /// This creates a handle that does not keep the asset alive.
    ///
    /// # Arguments
    /// * `value` - A u128 value to use as the UUID
    /// * `asset_type` - The Python type of the asset (e.g., `Mesh`, `Image`)
    #[staticmethod]
    pub fn uuid_from_u128<'py>(
        py: Python,
        value: u128,
        asset_type: Bound<'py, PyType>,
    ) -> PyResult<Py<Self>> {
        let (type_ptr, logical_type_id) = if asset_type.hasattr("__pybevy_asset_type__")? {
            let actual_type = asset_type
                .getattr("__pybevy_asset_type__")?
                .cast_into::<PyType>()?;
            let logical_type_id = asset_type
                .getattr("__pybevy_logical_type_id__")?
                .extract::<u64>()?;
            (
                actual_type.as_type_ptr(),
                Some(LogicalTypeId::new(logical_type_id)),
            )
        } else {
            (asset_type.as_type_ptr(), None)
        };

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
                kind: HandleKind::Uuid(Uuid::from_u128(value)),
                type_ptr,
                logical_type_id,
            },
        )
    }

    /// Get the Python type of the asset this handle refers to.
    pub fn asset_type_class(&self) -> PyResult<Py<PyType>> {
        Python::attach(|py| {
            if let Some(bridge) = global_registry::get_asset_bridge_by_py_type(self.type_ptr) {
                return Ok(bridge.py_type(py).unbind());
            }
            // SAFETY: type_ptr was captured from a live type object at handle
            // construction; asset classes are module-level, so their defining
            // module keeps them alive
            let type_obj =
                unsafe { Bound::from_borrowed_ptr(py, self.type_ptr as *mut ffi::PyObject) };
            type_obj
                .cast_into::<PyType>()
                .map(Bound::unbind)
                .map_err(|_| {
                    PyValueError::new_err(
                        "Stored asset type pointer no longer refers to a type object",
                    )
                })
        })
    }

    /// Get this handle's Bevy asset identifier.
    pub fn id(&self, py: Python<'_>) -> PyResult<Py<PyAssetId>> {
        materialize_asset_id(py, self.asset_id())
    }

    /// Check if this is a strong handle (keeps the asset alive).
    pub fn is_strong(&self) -> bool {
        matches!(self.kind, HandleKind::Strong(_))
    }

    /// Check if this is a UUID handle.
    pub fn is_uuid(&self) -> bool {
        matches!(self.kind, HandleKind::Uuid(_))
    }

    #[getter]
    fn _logical_type_id(&self) -> Option<u64> {
        self.logical_type_id.map(LogicalTypeId::get)
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
            HandleKind::Uuid(uuid) => format!("Uuid(uuid={})", uuid),
        };
        let type_name = self.asset_type_name().unwrap_or("Unknown");
        match self.logical_type_id {
            Some(logical_type_id) => format!(
                "Handle[{}; logical_type={}]({})",
                type_name,
                logical_type_id.get(),
                kind_str
            ),
            None => format!("Handle[{}]({})", type_name, kind_str),
        }
    }

    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash_asset_id(&mut hasher);
        self.type_ptr.hash(&mut hasher);
        self.logical_type_id.hash(&mut hasher);
        hasher.finish()
    }

    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: pyo3::pyclass::CompareOp,
    ) -> PyResult<Py<PyAny>> {
        use pyo3::pyclass::CompareOp;

        let Ok(other_handle) = other.extract::<Self>() else {
            return other.py().NotImplemented().into_py_any(other.py());
        };
        match op {
            CompareOp::Eq => (self == &other_handle).into_py_any(other.py()),
            CompareOp::Ne => (self != &other_handle).into_py_any(other.py()),
            _ => other.py().NotImplemented().into_py_any(other.py()),
        }
    }
}

impl PyHandle {
    fn same_asset_id(&self, other: &Self) -> bool {
        match (&self.kind, &other.kind) {
            (HandleKind::Strong(left), HandleKind::Strong(right)) => left.id() == right.id(),
            (HandleKind::Uuid(left), HandleKind::Uuid(right)) => left == right,
            (HandleKind::Strong(strong), HandleKind::Uuid(uuid))
            | (HandleKind::Uuid(uuid), HandleKind::Strong(strong)) => {
                matches!(strong.id(), UntypedAssetId::Uuid { uuid: strong_uuid, .. } if strong_uuid == *uuid)
            }
        }
    }

    fn hash_asset_id(&self, state: &mut impl std::hash::Hasher) {
        match &self.kind {
            HandleKind::Strong(strong) => match strong.id() {
                UntypedAssetId::Index { index, .. } => index.hash(state),
                UntypedAssetId::Uuid { uuid, .. } => uuid.hash(state),
            },
            HandleKind::Uuid(uuid) => uuid.hash(state),
        }
    }

    pub fn asset_id(&self) -> PyAssetId {
        PyAssetId::from_untyped_with_logical_type(self.untyped_id(), self.logical_type_id)
    }

    #[cfg(test)]
    fn raw_id(&self) -> u128 {
        match &self.kind {
            HandleKind::Strong(untyped) => match untyped.id() {
                UntypedAssetId::Index { index, .. } => index.to_bits() as u128,
                UntypedAssetId::Uuid { uuid, .. } => uuid.as_u128(),
            },
            HandleKind::Uuid(uuid) => uuid.as_u128(),
        }
    }

    pub fn untyped_id(&self) -> UntypedAssetId {
        match &self.kind {
            HandleKind::Strong(untyped) => untyped.id(),
            HandleKind::Uuid(uuid) => UntypedAssetId::Uuid {
                type_id: self
                    .asset_type_id()
                    .expect("UUID handles require a registered asset type"),
                uuid: *uuid,
            },
        }
    }

    pub fn logical_type_id(&self) -> Option<LogicalTypeId> {
        self.logical_type_id
    }

    pub fn with_logical_type_id(mut self, logical_type_id: Option<LogicalTypeId>) -> Self {
        self.logical_type_id = logical_type_id;
        self
    }

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

    /// Create a PyHandle from a type pointer and UntypedHandle.
    ///
    /// Used by AssetBridge implementations to create handles with known type info.
    pub fn from_untyped(handle: UntypedHandle, type_ptr: *const PyTypeObject) -> Self {
        Self::from_untyped_with_logical_type(handle, type_ptr, None)
    }

    pub fn from_untyped_with_logical_type(
        handle: UntypedHandle,
        type_ptr: *const PyTypeObject,
        logical_type_id: Option<LogicalTypeId>,
    ) -> Self {
        match &handle {
            UntypedHandle::Strong(_) => PyHandle {
                kind: HandleKind::Strong(handle),
                type_ptr,
                logical_type_id,
            },
            UntypedHandle::Uuid { uuid, .. } => PyHandle {
                kind: HandleKind::Uuid(*uuid),
                type_ptr,
                logical_type_id,
            },
        }
    }

    /// Convert to an UntypedHandle.
    ///
    /// For strong handles, returns the underlying UntypedHandle directly.
    /// For UUID handles, constructs an UntypedHandle::Uuid using the
    /// asset bridge's TypeId for proper Bevy asset lookup.
    ///
    /// # Errors
    /// Returns an error if the handle's asset type is not registered.
    pub fn to_untyped_handle(&self) -> PyResult<UntypedHandle> {
        match &self.kind {
            HandleKind::Strong(untyped) => Ok(untyped.clone()),
            HandleKind::Uuid(uuid) => {
                let bridge = global_registry::get_asset_bridge_by_py_type(self.type_ptr)
                    .ok_or_else(|| {
                        pyo3::exceptions::PyRuntimeError::new_err(
                            "Cannot convert UUID handle: asset type not registered",
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
            HandleKind::Uuid(_) => None,
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
    use std::sync::Once;

    use pyo3::types::{PyInt, PyList};

    use super::*;

    static INIT: Once = Once::new();

    fn setup_python() {
        INIT.call_once(|| {
            Python::initialize();
        });
    }

    /// Regression test: a type_ptr that does not point at a type object must
    /// surface an error, not an unchecked cast to PyType.
    #[test]
    fn asset_type_class_rejects_non_type_pointer() {
        setup_python();
        Python::attach(|py| {
            let not_a_type = PyList::empty(py);
            let handle = PyHandle {
                kind: HandleKind::Uuid(Uuid::from_u128(1)),
                type_ptr: not_a_type.as_ptr() as *const PyTypeObject,
                logical_type_id: None,
            };
            let result = handle.asset_type_class();
            assert!(result.is_err(), "instance pointer must not cast to PyType");
        });
    }

    /// The no-bridge fallback must still revive a genuine type object.
    #[test]
    fn asset_type_class_falls_back_to_stored_type_pointer() {
        setup_python();
        Python::attach(|py| {
            let int_type = py.get_type::<PyInt>();
            let handle = PyHandle {
                kind: HandleKind::Uuid(Uuid::from_u128(2)),
                type_ptr: int_type.as_type_ptr(),
                logical_type_id: None,
            };
            let cls = handle
                .asset_type_class()
                .expect("real type object must revive via fallback");
            assert!(cls.bind(py).is(&int_type));
        });
    }

    // The following tests exercise pure-Rust dispatch logic on PyHandle that
    // does not require a Python interpreter: kind matching, id extraction, and
    // registry lookups with an unregistered (null) type pointer return None.

    #[test]
    fn uuid_handle_is_not_strong() {
        let h = PyHandle {
            kind: HandleKind::Uuid(Uuid::from_u128(42)),
            type_ptr: std::ptr::null(),
            logical_type_id: None,
        };
        assert!(h.is_uuid());
        assert!(!h.is_strong());
    }

    #[test]
    fn raw_id_returns_uuid_for_uuid_handle() {
        let h = PyHandle {
            kind: HandleKind::Uuid(Uuid::from_u128(0xDEAD_BEEF)),
            type_ptr: std::ptr::null(),
            logical_type_id: None,
        };
        assert_eq!(h.raw_id(), 0xDEAD_BEEF);
    }

    #[test]
    fn test_asset_type_id_none_for_null_ptr() {
        let h = PyHandle {
            kind: HandleKind::Uuid(Uuid::from_u128(0)),
            type_ptr: std::ptr::null(),
            logical_type_id: None,
        };
        assert!(h.asset_type_id().is_none());
    }

    #[test]
    fn test_asset_type_name_none_for_null_ptr() {
        let h = PyHandle {
            kind: HandleKind::Uuid(Uuid::from_u128(0)),
            type_ptr: std::ptr::null(),
            logical_type_id: None,
        };
        assert!(h.asset_type_name().is_none());
    }

    #[test]
    fn uuid_handle_has_no_strong_storage() {
        let h = PyHandle {
            kind: HandleKind::Uuid(Uuid::from_u128(0)),
            type_ptr: std::ptr::null(),
            logical_type_id: None,
        };
        assert!(!h.has_strong_handle());
        assert!(h.try_untyped_handle().is_none());
    }

    #[test]
    fn uuid_kind_reports_uuid_identity() {
        // from_untyped with no Python: construct an UntypedHandle::Uuid directly
        // is not possible without an AssetRegistry, so we only verify the UUID
        // branch via the kind field. Strong handles need an Arc<AssetServer> so
        // they cannot be built in a Python-free unit test.
        let h = PyHandle {
            kind: HandleKind::Uuid(Uuid::from_u128(7)),
            type_ptr: std::ptr::null(),
            logical_type_id: None,
        };
        assert_eq!(h.raw_id(), 7);
        assert!(h.is_uuid());
    }

    #[test]
    fn logical_type_identity_participates_in_handle_equality_and_hashing() {
        let raw = PyHandle {
            kind: HandleKind::Uuid(Uuid::from_u128(42)),
            type_ptr: std::ptr::null(),
            logical_type_id: None,
        };
        let first = raw
            .clone()
            .with_logical_type_id(Some(LogicalTypeId::new(1)));
        let first_clone = first.clone();
        let second = raw
            .clone()
            .with_logical_type_id(Some(LogicalTypeId::new(2)));

        assert_eq!(first, first_clone);
        assert_eq!(first.__hash__(), first_clone.__hash__());
        assert_ne!(raw, first);
        assert_ne!(first, second);
    }

    #[test]
    fn tagging_handle_preserves_native_identity_and_type() {
        let type_ptr = std::ptr::dangling();
        let raw = PyHandle {
            kind: HandleKind::Uuid(Uuid::from_u128(99)),
            type_ptr,
            logical_type_id: None,
        };
        let tagged = raw
            .clone()
            .with_logical_type_id(Some(LogicalTypeId::new(7)));

        assert_eq!(tagged.raw_id(), raw.raw_id());
        assert_eq!(tagged.type_ptr(), raw.type_ptr());
        assert_eq!(tagged.logical_type_id(), Some(LogicalTypeId::new(7)));
        assert!(tagged.is_uuid());
    }

    #[test]
    fn tagged_handle_repr_reports_logical_identity() {
        let handle = PyHandle {
            kind: HandleKind::Uuid(Uuid::from_u128(5)),
            type_ptr: std::ptr::null(),
            logical_type_id: Some(LogicalTypeId::new(11)),
        };

        assert!(handle.__repr__().contains("logical_type=11"));
    }
}
