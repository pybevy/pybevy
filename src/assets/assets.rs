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

use bevy::prelude::World;
use pybevy_core::{
    PyMaterializable,
    handle::PyHandle,
    registry::{AssetBridge, global_registry},
};
use pybevy_mesh::{
    mesh_builder::PyMeshBuilder,
    meshable::{PyMeshable, meshable_to_mesh},
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
    // Raw pointer to World - SAFETY: Only valid when validity flag is true
    world: *mut World,
    // Runtime validity check with read/write mode - prevents use after system execution
    // and enforces Res[Assets[T]] (read-only) vs ResMut[Assets[T]] (mutable) semantics
    validity: ValidityFlagWithMode,
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
    /// The provided world pointer must be valid for the lifetime of the ValidityFlag.
    pub(crate) unsafe fn new(
        type_ptr: *const PyTypeObject,
        wrapper_class: Option<*const PyTypeObject>,
        world: *mut World,
        validity: ValidityFlag,
        is_mutable: bool,
    ) -> Self {
        let access_mode = if is_mutable {
            AccessMode::Write
        } else {
            AccessMode::Read
        };

        Self {
            type_ptr,
            wrapper_class,
            world,
            validity: validity.with_access_mode(access_mode),
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

    /// Get a reference to the world (read-only access)
    fn world_ref(&self) -> PyResult<&World> {
        self.validity.check_read()?;
        Ok(unsafe { &*self.world })
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

        Ok(unsafe { &mut *self.world })
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

        // Builder pattern handlings
        if asset.is_instance_of::<PyMeshBuilder>() {
            let bridge = self.bridge()?;
            if bridge.name() != "Mesh" {
                return Err(PyTypeError::new_err(format!(
                    "Asset type mismatch: expected `{}` but got `Mesh`",
                    bridge.name()
                )));
            }
            let py_mesh = asset.call_method0("build")?;
            let world = self.world_mut()?;
            let untyped_handle = bridge.add(world, &py_mesh, py)?;
            return Ok(PyHandle::from_untyped(untyped_handle, self.type_ptr));
        } else if asset.is_instance_of::<PyMeshable>() {
            let bridge = self.bridge()?;
            if bridge.name() != "Mesh" {
                return Err(PyTypeError::new_err(format!(
                    "Asset type mismatch: expected `{}` but got `Mesh`",
                    bridge.name()
                )));
            }
            // Some mesh builders (e.g. SphereMeshBuilder) extend PyMeshable instead of
            // PyMeshBuilder. Try build() first for builders, then meshable_to_mesh() for shapes.
            if let Ok(py_mesh) = asset.call_method0("build") {
                let world = self.world_mut()?;
                let untyped_handle = bridge.add(world, &py_mesh, py)?;
                return Ok(PyHandle::from_untyped(untyped_handle, self.type_ptr));
            }
            // Convert meshable shape to mesh entirely in Rust, bypassing Python method resolution
            let mesh = meshable_to_mesh(&asset)?;
            let world = self.world_mut()?;
            let mut assets = world
                .get_resource_mut::<bevy::asset::Assets<bevy::mesh::Mesh>>()
                .ok_or_else(|| PyRuntimeError::new_err("Assets<Mesh> resource not found"))?;
            let handle = assets.add(mesh);
            return Ok(PyHandle::from_untyped(handle.untyped(), self.type_ptr));
        } else if asset.is_instance_of::<PyMaterializable>() {
            let bridge = self.bridge()?;
            if bridge.name() != "StandardMaterial" {
                return Err(PyTypeError::new_err(format!(
                    "Asset type mismatch: expected `{}` but got `StandardMaterial`",
                    bridge.name()
                )));
            }
            let material = asset.call_method0("materialize")?;
            let world = self.world_mut()?;
            let untyped_handle = bridge.add(world, &material, py)?;
            return Ok(PyHandle::from_untyped(untyped_handle, self.type_ptr));
        }

        // Validate asset type matches container type
        let asset_type_ptr = asset.get_type().as_type_ptr() as *const PyTypeObject;
        if asset_type_ptr != self.type_ptr {
            // Check for @material auto-conversion (to_shader_material)
            if let Ok(converter) = asset.getattr("to_shader_material") {
                let converted = converter.call0()?;
                let converted_type_ptr = converted.get_type().as_type_ptr() as *const PyTypeObject;
                if converted_type_ptr == self.type_ptr {
                    let bridge = self.bridge()?;
                    let world = self.world_mut()?;
                    let untyped_handle = bridge.add(world, &converted, py)?;
                    return Ok(PyHandle::from_untyped(untyped_handle, self.type_ptr));
                }
            }

            let bridge = self.bridge()?;
            let asset_bridge = global_registry::get_asset_bridge_by_py_type(asset_type_ptr);
            let asset_name = asset_bridge.as_ref().map(|b| b.name()).unwrap_or("Unknown");
            return Err(PyTypeError::new_err(format!(
                "Asset type mismatch: expected `{}` but got `{}`",
                bridge.name(),
                asset_name
            )));
        }

        let bridge = self.bridge()?;
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
            py,
        )
    }

    pub fn get_mut(&mut self, py: Python, id: Bound<'_, PyHandle>) -> PyResult<Option<Py<PyAny>>> {
        let handle = id.extract::<PyHandle>()?;
        self.check_handle_type(&handle)?;
        let bridge = self.bridge()?;
        let validity = self.validity.clone();
        let untyped_handle = handle.to_untyped_handle()?;
        let world = self.world_mut()?;
        let raw = bridge.get_mut(world, &untyped_handle, validity, py)?;

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
        let pairs = bridge.iter_pairs(world, self.validity.clone(), py)?;
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
    fn __next__<'a>(&'a mut self, py: Python<'a>) -> PyResult<Py<PyAny>> {
        if let Some((handle, value)) = self.values.pop_front() {
            PyTuple::new(py, [Py::new(py, handle)?.into_any(), value])?.into_py_any(py)
        } else {
            Err(PyErr::new::<PyStopIteration, _>(""))
        }
    }
}
