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
//! PyAssets delegates runtime validity, mutability, asset identity, and live-borrow
//! admission to the backend-neutral `AssetRuntimeCore`:
//!
//! - **Read-only access** (`Res[Assets[T]]`): Creates validity with `AccessMode::Read`
//!   - `get()` allowed, `get_mut()` raises error
//! - **Mutable access** (`ResMut[Assets[T]]`): Creates validity with `AccessMode::Write`
//!   - Both `get()` and `get_mut()` allowed
use std::{any::TypeId, collections::VecDeque, sync::Arc};

use bevy::{ecs::world::unsafe_world_cell::UnsafeWorldCell, prelude::World};
use pybevy_core::{
    AssetBorrowCounter, AssetRuntimeCore, AssetRuntimeError, LogicalTypeId, PyAssetId,
    extract_asset_id_from_any,
    handle::PyHandle,
    materialize_asset_id,
    public_error::ASSET_BRIDGE_NOT_FOUND,
    registry::{AssetBridge, global_registry},
};
use pyo3::{
    IntoPyObjectExt,
    exceptions::{PyRuntimeError, PyStopIteration, PyTypeError, PyValueError},
    ffi::{PyObject, PyTypeObject},
    prelude::*,
    types::{PyTuple, PyType},
};

use super::asset_type::PyAssetTypeParam;
use crate::ecs::{
    helpers::validity_guard::{AccessMode, ValidityFlag},
    resource::PyResource,
};

/// Wrapper for Bevy's Assets<T> resource providing Python access to asset collections.
#[pyclass(name = "Assets", extends = PyResource)]
#[derive(Debug)]
pub struct PyAssets {
    runtime: AssetRuntimeCore<TypeId>,
    /// If set, the `@material`-decorated class for auto-wrapping `get_mut()` results.
    wrapper_class: Option<*const PyTypeObject>,
    logical_type_id: Option<LogicalTypeId>,
    logical_type_name: Option<String>,
    /// World cell (lifetime-erased), valid only while the validity flag is active.
    /// Used only to reach the declared `Assets<T>` resource through the AssetBridge.
    cell: UnsafeWorldCell<'static>,
}

fn asset_runtime_py_error(error: AssetRuntimeError) -> PyErr {
    if error.is_asset_type_mismatch() {
        PyValueError::new_err(error.to_string())
    } else {
        PyRuntimeError::new_err(error.to_string())
    }
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
        logical_type_id: Option<LogicalTypeId>,
        logical_type_name: Option<String>,
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

        let bridge = global_registry::get_asset_bridge_by_py_type(type_ptr)
            .expect("Assets[T] requires a registered asset bridge");
        let asset_name = bridge.name();
        let type_id = bridge.bevy_type_id();

        Self {
            runtime: AssetRuntimeCore::new(
                type_id,
                asset_name,
                validity.with_access_mode(access_mode),
                borrow_counter,
            ),
            wrapper_class,
            logical_type_id,
            logical_type_name,
            cell,
        }
    }

    fn type_id(&self) -> TypeId {
        *self.runtime.type_key()
    }

    /// Get the AssetBridge for this asset type.
    fn bridge(&self) -> PyResult<Arc<dyn AssetBridge>> {
        global_registry::get_asset_bridge_by_type_id(self.type_id())
            .ok_or_else(|| PyRuntimeError::new_err(ASSET_BRIDGE_NOT_FOUND))
    }

    /// Validate that an asset ID's type matches this collection's type.
    fn check_id_type(&self, id: &PyAssetId) -> PyResult<()> {
        self.runtime
            .check_asset_type(
                &id.untyped().type_id(),
                id.asset_type_name().unwrap_or("Unknown"),
            )
            .map_err(asset_runtime_py_error)?;
        if let Some(expected) = self.logical_type_id
            && id.logical_type_id() != Some(expected)
        {
            return Err(self.logical_type_mismatch(id.logical_type_id()));
        }
        Ok(())
    }

    fn logical_type_mismatch(&self, actual: Option<LogicalTypeId>) -> PyErr {
        let expected_name = self.logical_type_name.as_deref().unwrap_or("logical asset");
        let actual = actual
            .map(|identity| identity.get().to_string())
            .unwrap_or_else(|| "untyped native asset".to_owned());
        PyTypeError::new_err(format!(
            "Logical asset type mismatch: Assets[{expected_name}] cannot use logical identity {actual}"
        ))
    }

    fn object_logical_type_id(object: &Bound<'_, PyAny>) -> PyResult<Option<LogicalTypeId>> {
        let value = if let Ok(value) = object.getattr("_logical_type_id") {
            value.extract::<Option<u64>>()?
        } else if let Ok(value) = object.get_type().getattr("__pybevy_logical_type_id__") {
            Some(value.extract::<u64>()?)
        } else {
            None
        };
        Ok(value.map(LogicalTypeId::new))
    }

    fn check_object_logical_type(&self, object: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(expected) = self.logical_type_id {
            let actual = Self::object_logical_type_id(object)?;
            if actual != Some(expected) {
                return Err(self.logical_type_mismatch(actual));
            }
        }
        Ok(())
    }

    fn check_no_live_asset_borrows(&self) -> PyResult<()> {
        self.runtime
            .check_no_live_asset_borrows()
            .map_err(asset_runtime_py_error)
    }

    /// Get a reference to the world (read-only access)
    fn world_ref(&self) -> PyResult<&World> {
        self.runtime.check_read().map_err(asset_runtime_py_error)?;
        // SAFETY: momentary derivation of a shared world reference, used only to reach
        // the declared `Assets<T>` resource through the AssetBridge. `initialize`
        // declares `Assets<T>` read access; the executor prevents a concurrent writer.
        // This is the same residual-pointer class as query_runtime::world_ptr.
        Ok(unsafe { self.cell.world() })
    }

    /// Get a mutable reference to the world
    fn world_mut(&mut self) -> PyResult<&mut World> {
        self.runtime.check_write().map_err(asset_runtime_py_error)?;

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
        self.check_object_logical_type(&asset)?;

        let asset = match bridge.try_convert_input(&asset, py)? {
            Some(converted) => converted,
            None => asset,
        };
        self.check_object_logical_type(&asset)?;
        let actual_logical_type_id = Self::object_logical_type_id(&asset)?;

        // Validate asset type matches container type
        let asset_type_ptr = asset.get_type().as_type_ptr() as *const PyTypeObject;
        if asset_type_ptr != bridge.py_type_ptr() {
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
        Ok(PyHandle::from_untyped_with_logical_type(
            untyped_handle,
            bridge.py_type_ptr(),
            actual_logical_type_id,
        ))
    }

    pub fn len(&self, py: Python<'_>) -> PyResult<usize> {
        let world = self.world_ref()?;
        let bridge = self.bridge()?;
        let Some(expected) = self.logical_type_id else {
            return bridge.len(world);
        };
        let pairs = bridge.iter_pairs(
            world,
            self.runtime.validity().clone(),
            self.runtime.borrow_counter().clone(),
            py,
        )?;
        pairs.into_iter().try_fold(0, |count, (_, object)| {
            Ok(count
                + usize::from(Self::object_logical_type_id(object.bind(py))? == Some(expected)))
        })
    }

    pub fn is_empty(&self, py: Python<'_>) -> PyResult<bool> {
        let world = self.world_ref()?;
        let bridge = self.bridge()?;
        let Some(expected) = self.logical_type_id else {
            return Ok(bridge.len(world)? == 0);
        };
        let pairs = bridge.iter_pairs(
            world,
            self.runtime.validity().clone(),
            self.runtime.borrow_counter().clone(),
            py,
        )?;
        for (_, object) in pairs {
            if Self::object_logical_type_id(object.bind(py))? == Some(expected) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn contains(&self, id: &Bound<'_, PyAny>) -> PyResult<bool> {
        let py = id.py();
        let id = extract_asset_id_from_any(id)?;
        self.check_id_type(&id)?;
        let bridge = self.bridge()?;
        let world = self.world_ref()?;
        if self.logical_type_id.is_none() {
            return bridge.contains(world, id.untyped());
        }
        let raw = bridge.get(
            world,
            id.untyped(),
            self.runtime.validity().clone(),
            self.runtime.borrow_counter().clone(),
            py,
        )?;
        let Some(raw) = raw else {
            return Ok(false);
        };
        self.check_object_logical_type(raw.bind(py))?;
        Ok(true)
    }

    pub fn remove(&mut self, py: Python, id: &Bound<'_, PyAny>) -> PyResult<Option<Py<PyAny>>> {
        let id = extract_asset_id_from_any(id)?;
        self.check_id_type(&id)?;
        self.check_no_live_asset_borrows()?;
        let bridge = self.bridge()?;
        if self.logical_type_id.is_some() {
            let raw = {
                let world = self.world_ref()?;
                bridge.get(
                    world,
                    id.untyped(),
                    self.runtime.validity().clone(),
                    self.runtime.borrow_counter().clone(),
                    py,
                )?
            };
            let Some(raw) = raw else {
                return Ok(None);
            };
            self.check_object_logical_type(raw.bind(py))?;
            drop(raw);
            self.check_no_live_asset_borrows()?;
        }
        let world = self.world_mut()?;
        bridge.remove_and_return(world, id.untyped(), py)
    }

    pub fn get(&self, py: Python, id: &Bound<'_, PyAny>) -> PyResult<Option<Py<PyAny>>> {
        let id = extract_asset_id_from_any(id)?;
        self.check_id_type(&id)?;
        let bridge = self.bridge()?;
        let world = self.world_ref()?;
        let raw = bridge.get(
            world,
            id.untyped(),
            self.runtime.validity().clone(),
            self.runtime.borrow_counter().clone(),
            py,
        )?;
        if let Some(raw_obj) = &raw {
            self.check_object_logical_type(raw_obj.bind(py))?;
        }

        if let (Some(raw_obj), Some(wrapper_ptr)) = (&raw, self.wrapper_class) {
            // SAFETY: wrapper_ptr is a Python type object, stable for interpreter lifetime
            let wrapper_cls: Bound<'_, PyAny> =
                unsafe { Bound::from_borrowed_ptr(py, wrapper_ptr as *mut PyObject) };
            let wrapped = wrapper_cls.call_method1("from_ref", (raw_obj,))?;
            return Ok(Some(wrapped.unbind()));
        }
        Ok(raw)
    }

    pub fn get_mut(&mut self, py: Python, id: &Bound<'_, PyAny>) -> PyResult<Option<Py<PyAny>>> {
        let id = extract_asset_id_from_any(id)?;
        self.check_id_type(&id)?;
        self.runtime.check_write().map_err(asset_runtime_py_error)?;
        let bridge = self.bridge()?;
        let validity = self.runtime.validity().clone();
        let borrow_counter = self.runtime.borrow_counter().clone();
        let raw = bridge.get_mut(self.cell, id.untyped(), validity, borrow_counter, py)?;
        if let Some(raw_obj) = &raw {
            self.check_object_logical_type(raw_obj.bind(py))?;
        }

        // Auto-wrap with @material class if this is a redirected Assets[HologramMaterial]
        if let (Some(raw_obj), Some(wrapper_ptr)) = (&raw, self.wrapper_class) {
            // SAFETY: wrapper_ptr is a Python type object, stable for interpreter lifetime
            let wrapper_cls: Bound<'_, PyAny> =
                unsafe { Bound::from_borrowed_ptr(py, wrapper_ptr as *mut PyObject) };
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
            self.runtime.validity().clone(),
            self.runtime.borrow_counter().clone(),
            py,
        )?;
        let mut values = Vec::with_capacity(pairs.len());
        for (id, obj) in pairs {
            let actual = Self::object_logical_type_id(obj.bind(py))?;
            if self.logical_type_id.is_some() && actual != self.logical_type_id {
                continue;
            }
            values.push((PyAssetId::from_untyped_with_logical_type(id, actual), obj));
        }

        Ok(PyAssetIter {
            values: values.into(),
        })
    }
}

#[pyclass(name = "AssetIter")]
#[derive(Debug)]
pub struct PyAssetIter {
    values: VecDeque<(PyAssetId, Py<PyAny>)>,
}

#[pymethods]
impl PyAssetIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__<'a>(&'a mut self, py: Python<'a>) -> PyResult<Py<PyAny>> {
        if let Some((id, value)) = self.values.pop_front() {
            PyTuple::new(py, [materialize_asset_id(py, id)?.into_any(), value])?.into_py_any(py)
        } else {
            Err(PyErr::new::<PyStopIteration, _>(""))
        }
    }
}
