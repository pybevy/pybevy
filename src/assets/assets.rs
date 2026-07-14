//! # PyAssets: Asset System Access
//!
//! This module provides Python bindings for Bevy's asset system, allowing access
//! to `Assets<T>` resources for various asset types (Mesh, Image, StandardMaterial, etc.).
//!
//! ## Design Notes
//!
//! ### Bridge-based Dispatch
//!
//! PyAssets uses the global AssetBridge registry for all type-specific operations.
//! Each asset type is registered via `#[pyasset(T, bridge)]` attribute, which generates the
//! bridge implementation. This eliminates the need for match blocks over asset types.
//!
//! ### Unified Validity and Mutability Tracking
//!
//! PyAssets uses `ValidityFlagWithMode` to track both runtime validity (prevents use
//! after system execution) and mutability (enforces `Res[Assets[T]]` vs `ResMut[Assets[T]]`):
//!
//! - **Read-only access** (`Res[Assets[T]]`): Creates validity with `AccessMode::Read`
//!   - `get()` allowed, `get_mut()` raises error
//! - **Mutable access** (`ResMut[Assets[T]]`): Creates validity with `AccessMode::Write`
//!   - Both `get()` and `get_mut()` allowed
use std::{collections::VecDeque, sync::Arc};

use bevy::{ecs::world::unsafe_world_cell::UnsafeWorldCell, prelude::World};
use pybevy_core::{
    AssetBorrowCounter,
    handle::PyHandle,
    registry::{AssetBridge, global_registry},
};
use pyo3::{
    IntoPyObjectExt,
    exceptions::{PyRuntimeError, PyStopIteration, PyTypeError},
    ffi::PyTypeObject,
    prelude::*,
    types::{PyTuple, PyType},
};

use super::asset_type::PyAssetTypeParam;
use crate::ecs::{
    helpers::validity_guard::{AccessMode, ValidityFlag, ValidityFlagWithMode},
    resource::PyResource,
};

/// Wrapper for Bevy's Assets<T> resource providing Python access to asset collections.
#[pyclass(name = "Assets", extends = PyResource)]
#[derive(Debug)]
pub struct PyAssets {
    type_ptr: *const PyTypeObject,
    /// If set, the `@material`-decorated class for auto-wrapping `get_mut()` results.
    wrapper_class: Option<*const PyTypeObject>,
    /// World cell (lifetime-erased), valid only while the validity flag is active.
    /// Used only to reach the declared `Assets<T>` resource through the AssetBridge.
    cell: UnsafeWorldCell<'static>,
    // Runtime validity check with read/write mode - prevents use after system execution
    // and enforces Res[Assets[T]] (read-only) vs ResMut[Assets[T]] (mutable) semantics
    validity: ValidityFlagWithMode,
    borrow_counter: AssetBorrowCounter,
}

// SAFETY: PyAssets is Send because:
// - The raw world pointer is protected by the ValidityFlag (Arc<AtomicBool>)
// - ValidityFlag::check() ensures the pointer is only dereferenced when valid
// - The validity flag is set to false when the system execution completes
// - PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for PyAssets {}

// SAFETY: PyAssets is Sync because:
// - Access to the underlying World is controlled by validity checking
// - The ValidityFlag uses atomic operations for thread-safe access
// - We only allow access when the validity flag is true (during system execution)
unsafe impl Sync for PyAssets {}

impl PyAssets {
    /// Create a new PyAssets wrapper
    ///
    /// # Safety
    /// The provided world cell must reference the world holding the `Assets<T>`
    /// resource and stay valid for as long as the ValidityFlag is active.
    pub(crate) unsafe fn new(
        type_ptr: *const PyTypeObject,
        wrapper_class: Option<*const PyTypeObject>,
        cell: UnsafeWorldCell,
        validity: ValidityFlag,
        is_mutable: bool,
        borrow_counter: AssetBorrowCounter,
    ) -> Self {
        let access_mode = if is_mutable {
            AccessMode::Write
        } else {
            AccessMode::Read
        };

        // SAFETY: layout-preserving lifetime erasure of a Copy pointer type; the
        // cell is only touched while `validity` is active.
        let cell: UnsafeWorldCell<'static> = unsafe { std::mem::transmute(cell) };

        Self {
            type_ptr,
            wrapper_class,
            cell,
            validity: validity.with_access_mode(access_mode),
            borrow_counter,
        }
    }

    /// Get the AssetBridge for this asset type.
    fn bridge(&self) -> PyResult<Arc<dyn AssetBridge>> {
        global_registry::get_asset_bridge_by_py_type(self.type_ptr)
            .ok_or_else(|| PyRuntimeError::new_err("Asset bridge not found for type"))
    }

    /// Validate that a handle's asset type matches this collection's type.
    fn check_handle_type(&self, handle: &PyHandle) -> PyResult<()> {
        if handle.type_ptr() != self.type_ptr {
            let handle_name = handle.asset_type_name().unwrap_or("Unknown");
            let self_name = self.bridge().map(|b| b.name()).unwrap_or("Unknown");
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Handle of type `{}` does not match expected type `{}`",
                handle_name, self_name
            )));
        }
        Ok(())
    }

    fn check_no_live_asset_borrows(&self) -> PyResult<()> {
        self.validity.check_read()?;
        if self.borrow_counter.has_active() {
            let name = self.bridge().map(|b| b.name()).unwrap_or("T");
            return Err(PyRuntimeError::new_err(format!(
                "Cannot structurally mutate Assets[{name}] while borrowed asset wrappers are live"
            )));
        }
        Ok(())
    }

    /// Get a reference to the world (read-only access)
    fn world_ref(&self) -> PyResult<&World> {
        self.validity.check_read()?;
        // SAFETY: momentary derivation of a shared world reference, used only to reach
        // the declared `Assets<T>` resource through the AssetBridge. `initialize`
        // declares `Assets<T>` read access; the executor prevents a concurrent writer.
        // This is the same residual-pointer class as query_runtime::world_ptr.
        Ok(unsafe { self.cell.world() })
    }

    /// Get a mutable reference to the world
    fn world_mut(&mut self) -> PyResult<&mut World> {
        // First check validity (system still executing)
        self.validity.check_read()?;

        // Then check access mode allows writing
        if self.validity.access_mode() != AccessMode::Write {
            return Err(PyRuntimeError::new_err(
                "Mutable access required. Use ResMut[Assets[T]] instead of Res[Assets[T]] for mutations.",
            ));
        }

        // SAFETY: momentary derivation of a mutable world reference, used only to reach
        // the declared `Assets<T>` resource through the AssetBridge. `initialize`
        // declares `Assets<T>` write access; the executor prevents a concurrent access.
        // This is the same residual-pointer class as query_runtime::world_ptr.
        Ok(unsafe { self.cell.world_mut() })
    }
}

#[pymethods]
impl PyAssets {
    /// Get a class item as AssetTypeParam by asset type (e.g., Assets[Mesh])
    ///
    /// Supports `@material` redirect: `Assets[HologramMaterial]` resolves to
    /// `Assets[ShaderMaterial]` internally, storing the wrapper class for `get_mut()`.
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        // Check for @material redirect (e.g. HologramMaterial.__pybevy_asset_type__ == ShaderMaterial)
        if key.hasattr("__pybevy_asset_type__").unwrap_or(false) {
            let actual_type_obj = key.getattr("__pybevy_asset_type__")?;
            let actual_type = actual_type_obj.cast::<PyType>()?;
            let key_type = key.cast::<PyType>()?;
            let param = PyAssetTypeParam::with_redirect(actual_type, key_type)?;
            return param.into_py_any(cls.py());
        }

        let key_type = key.cast::<PyType>()?;
        let param = PyAssetTypeParam::try_from_py_type(key_type)?;
        param.into_py_any(cls.py())
    }

    pub fn add(&mut self, asset: Bound<'_, PyAny>) -> PyResult<PyHandle> {
        let py = asset.py();
        let bridge = self.bridge()?;
        self.check_no_live_asset_borrows()?;

        let asset = match bridge.try_convert_input(&asset, py)? {
            Some(converted) => converted,
            None => asset,
        };

        // Validate asset type matches container type
        let asset_type_ptr = asset.get_type().as_type_ptr() as *const PyTypeObject;
        if asset_type_ptr != self.type_ptr {
            let asset_bridge = global_registry::get_asset_bridge_by_py_type(asset_type_ptr);
            let asset_name = asset_bridge.as_ref().map(|b| b.name()).unwrap_or("Unknown");
            return Err(PyTypeError::new_err(format!(
                "Asset type mismatch: expected `{}` but got `{}`",
                bridge.name(),
                asset_name
            )));
        }

        let world = self.world_mut()?;
        let untyped_handle = bridge.add(world, &asset, py)?;
        Ok(PyHandle::from_untyped(untyped_handle, self.type_ptr))
    }

    pub fn len(&self) -> PyResult<usize> {
        let world = self.world_ref()?;
        let bridge = self.bridge()?;
        bridge.len(world)
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.len()? == 0)
    }

    pub fn contains(&self, id: Bound<'_, PyHandle>) -> PyResult<bool> {
        let handle = id.extract::<PyHandle>()?;
        self.check_handle_type(&handle)?;
        let bridge = self.bridge()?;
        let world = self.world_ref()?;
        bridge.contains(world, &handle.to_untyped_handle()?)
    }

    pub fn remove(&mut self, py: Python, id: Bound<'_, PyHandle>) -> PyResult<Option<Py<PyAny>>> {
        let handle = id.extract::<PyHandle>()?;
        self.check_handle_type(&handle)?;
        self.check_no_live_asset_borrows()?;
        let bridge = self.bridge()?;
        let world = self.world_mut()?;
        bridge.remove_and_return(world, &handle.to_untyped_handle()?, py)
    }

    pub fn get(&self, py: Python, id: Bound<'_, PyHandle>) -> PyResult<Option<Py<PyAny>>> {
        let handle = id.extract::<PyHandle>()?;
        self.check_handle_type(&handle)?;
        let bridge = self.bridge()?;
        let world = self.world_ref()?;
        bridge.get(
            world,
            &handle.to_untyped_handle()?,
            self.validity.clone(),
            self.borrow_counter.clone(),
            py,
        )
    }

    pub fn get_mut(&mut self, py: Python, id: Bound<'_, PyHandle>) -> PyResult<Option<Py<PyAny>>> {
        let handle = id.extract::<PyHandle>()?;
        self.check_handle_type(&handle)?;
        let bridge = self.bridge()?;
        let validity = self.validity.clone();
        let borrow_counter = self.borrow_counter.clone();
        let untyped_handle = handle.to_untyped_handle()?;
        let world = self.world_mut()?;
        let raw = bridge.get_mut(world, &untyped_handle, validity, borrow_counter, py)?;

        // Auto-wrap with @material class if this is a redirected Assets[HologramMaterial]
        if let (Some(raw_obj), Some(wrapper_ptr)) = (&raw, self.wrapper_class) {
            // SAFETY: wrapper_ptr is a Python type object, stable for interpreter lifetime
            let wrapper_cls: pyo3::Bound<'_, PyAny> = unsafe {
                pyo3::Bound::from_borrowed_ptr(py, wrapper_ptr as *mut pyo3::ffi::PyObject)
            };
            let wrapped = wrapper_cls.call_method1("from_mut", (raw_obj,))?;
            return Ok(Some(wrapped.unbind()));
        }

        Ok(raw)
    }

    pub fn __iter__(&self, py: Python) -> PyResult<PyAssetIter> {
        let bridge = self.bridge()?;
        let world = self.world_ref()?;
        let pairs = bridge.iter_pairs(
            world,
            self.validity.clone(),
            self.borrow_counter.clone(),
            py,
        )?;
        let type_ptr = self.type_ptr;
        let values = pairs
            .into_iter()
            .map(|(untyped_handle, obj)| {
                let py_handle = PyHandle::from_untyped(untyped_handle, type_ptr);
                (py_handle, obj)
            })
            .collect();

        Ok(PyAssetIter { values })
    }
}

#[pyclass(name = "AssetIter")]
#[derive(Debug)]
pub struct PyAssetIter {
    values: VecDeque<(PyHandle, Py<PyAny>)>,
}

#[pymethods]
impl PyAssetIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__<'a>(&'a mut self, py: Python<'a>) -> PyResult<Py<PyAny>> {
        if let Some((handle, value)) = self.values.pop_front() {
            PyTuple::new(py, [Py::new(py, handle)?.into_any(), value])?.into_py_any(py)
        } else {
            Err(PyErr::new::<PyStopIteration, _>(""))
        }
    }
}
