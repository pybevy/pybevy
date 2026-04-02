use bevy::{
    asset::{AssetPath, AssetServer, UntypedAssetId},
    image::Image,
    prelude::World,
};
use pybevy_core::{handle::PyHandle, registry::global_registry};
use pybevy_image::loader_settings::PyImageLoaderSettings;
use pyo3::{
    IntoPyObjectExt,
    exceptions::{PyRuntimeError, PyTypeError},
    prelude::*,
    types::{PyString, PyType},
};

use crate::{
    assets::{PyAssetPath, load_state::PyLoadState},
    ecs::{helpers::validity_guard::ValidityFlag, resource::PyResource},
};

/// Extract an AssetPath from a Python object (str or AssetPath).
fn extract_asset_path(path: &Bound<'_, PyAny>) -> PyResult<AssetPath<'static>> {
    if path.is_instance_of::<PyString>() {
        Ok(AssetPath::from(path.extract::<String>()?))
    } else if path.is_instance_of::<PyAssetPath>() {
        let py_path = path.extract::<PyAssetPath>()?;
        Ok(AssetPath::from(&py_path))
    } else {
        Err(PyTypeError::new_err(format!(
            "Expected str or AssetPath, got {:?}",
            path.get_type()
        )))
    }
}

/// Python wrapper for Bevy's AssetServer
#[pyclass(name = "AssetServer", extends = PyResource)]
#[derive(Debug)]
pub struct PyAssetServer {
    world: *const World,
    validity: ValidityFlag,
}

unsafe impl Send for PyAssetServer {}
unsafe impl Sync for PyAssetServer {}

impl PyAssetServer {
    pub(crate) unsafe fn new(world: *const World, validity: ValidityFlag) -> Self {
        Self { world, validity }
    }

    fn world_ref(&self) -> PyResult<&World> {
        self.validity.check()?;
        Ok(unsafe { &*self.world })
    }
}

#[pymethods]
impl PyAssetServer {
    pub fn load<'py>(
        &self,
        py: Python,
        path: Bound<'py, PyAny>,
        asset_type: Bound<'py, PyType>,
    ) -> PyResult<Py<PyAny>> {
        let type_ptr = asset_type.as_type_ptr();
        let bridge = global_registry::get_asset_bridge_by_py_type(type_ptr).ok_or_else(|| {
            PyTypeError::new_err(format!(
                "Invalid asset type. Expected a subclass of `Asset`, but got `{}`",
                asset_type
            ))
        })?;

        if !bridge.is_loadable() {
            return Err(PyTypeError::new_err(format!(
                "`{}` is not a file-loadable asset type",
                bridge.name()
            )));
        }

        let world = self.world_ref()?;
        let asset_server = world.resource::<AssetServer>();
        let asset_path = extract_asset_path(&path)?;
        let untyped_handle = bridge.load(asset_server, asset_path);
        let py_handle = PyHandle::from_untyped(untyped_handle, type_ptr);
        py_handle.into_py_any(py)
    }

    pub fn load_scene<'py>(&self, py: Python, path: Bound<'py, PyAny>) -> PyResult<Py<PyAny>> {
        self.load_by_name(py, path, "Scene")
    }

    pub fn load_image<'py>(&self, py: Python, path: Bound<'py, PyAny>) -> PyResult<Py<PyAny>> {
        self.load_by_name(py, path, "Image")
    }

    #[pyo3(signature = (path, asset_type, settings))]
    pub fn load_with_settings<'py>(
        &self,
        py: Python,
        path: Bound<'py, PyAny>,
        asset_type: Bound<'py, PyType>,
        settings: Bound<'py, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let type_ptr = asset_type.as_type_ptr();
        let bridge = global_registry::get_asset_bridge_by_py_type(type_ptr).ok_or_else(|| {
            PyTypeError::new_err(format!(
                "Invalid asset type. Expected a subclass of `Asset`, but got `{}`",
                asset_type
            ))
        })?;

        let world = self.world_ref()?;
        let asset_server = world.resource::<AssetServer>();
        let asset_path = extract_asset_path(&path)?;

        let untyped_handle = match bridge.name() {
            "Image" => {
                let image_settings: PyImageLoaderSettings = settings.extract()?;
                let bevy_settings: bevy::image::ImageLoaderSettings = image_settings.into();
                asset_server
                    .load_with_settings::<Image, _>(asset_path, move |s| {
                        *s = bevy_settings.clone();
                    })
                    .untyped()
            }
            name => {
                return Err(PyTypeError::new_err(format!(
                    "`{}` does not support loader settings (supported: Image)",
                    name
                )));
            }
        };

        let py_handle = PyHandle::from_untyped(untyped_handle, type_ptr);
        py_handle.into_py_any(py)
    }

    #[pyo3(signature = (path, settings))]
    pub fn load_image_with_settings<'py>(
        &self,
        py: Python,
        path: Bound<'py, PyAny>,
        settings: PyImageLoaderSettings,
    ) -> PyResult<Py<PyAny>> {
        let bridge = global_registry::get_asset_bridge_by_name("Image")
            .ok_or_else(|| PyRuntimeError::new_err("Asset bridge for 'Image' not found"))?;

        let world = self.world_ref()?;
        let asset_server = world.resource::<AssetServer>();
        let asset_path = extract_asset_path(&path)?;
        let bevy_settings: bevy::image::ImageLoaderSettings = settings.into();
        let untyped_handle = asset_server
            .load_with_settings::<Image, _>(asset_path, move |s| {
                *s = bevy_settings.clone();
            })
            .untyped();
        let py_handle = PyHandle::from_untyped(untyped_handle, bridge.py_type_ptr());
        py_handle.into_py_any(py)
    }

    pub fn load_mesh<'py>(&self, py: Python, path: Bound<'py, PyAny>) -> PyResult<Py<PyAny>> {
        self.load_by_name(py, path, "Mesh")
    }

    pub fn load_audio<'py>(&self, py: Python, path: Bound<'py, PyAny>) -> PyResult<Py<PyAny>> {
        self.load_by_name(py, path, "AudioSource")
    }

    pub fn load_folder<'py>(&self, py: Python, path: Bound<'py, PyAny>) -> PyResult<Py<PyAny>> {
        self.load_by_name(py, path, "LoadedFolder")
    }

    pub fn load_state(&self, id: &PyHandle) -> PyResult<PyLoadState> {
        let world = self.world_ref()?;
        let asset_server = world.resource::<AssetServer>();
        let untyped_id: UntypedAssetId = id.clone().into();
        Ok(PyLoadState::from(asset_server.load_state(untyped_id)))
    }

    pub fn is_loaded(&self, id: &PyHandle) -> PyResult<bool> {
        let world = self.world_ref()?;
        let asset_server = world.resource::<AssetServer>();
        let untyped_id: UntypedAssetId = id.clone().into();
        Ok(asset_server.is_loaded(untyped_id))
    }

    pub fn is_loaded_with_dependencies(&self, id: &PyHandle) -> PyResult<bool> {
        let world = self.world_ref()?;
        let asset_server = world.resource::<AssetServer>();
        let untyped_id: UntypedAssetId = id.clone().into();
        Ok(asset_server.is_loaded_with_dependencies(untyped_id))
    }

    pub fn get_handle<'py>(
        &self,
        py: Python,
        path: Bound<'py, PyAny>,
        asset_type: Bound<'py, PyType>,
    ) -> PyResult<Option<Py<PyAny>>> {
        let type_ptr = asset_type.as_type_ptr();
        let bridge = global_registry::get_asset_bridge_by_py_type(type_ptr).ok_or_else(|| {
            PyTypeError::new_err(format!(
                "Invalid asset type. Expected a subclass of `Asset`, but got `{}`",
                asset_type
            ))
        })?;

        let world = self.world_ref()?;
        let asset_server = world.resource::<AssetServer>();
        let asset_path = extract_asset_path(&path)?;

        match bridge.get_handle(asset_server, asset_path) {
            Some(untyped_handle) => {
                let py_handle = PyHandle::from_untyped(untyped_handle, type_ptr);
                Ok(Some(py_handle.into_py_any(py)?))
            }
            None => Ok(None),
        }
    }
}

impl PyAssetServer {
    /// Helper to load by bridge name (for convenience methods).
    fn load_by_name<'py>(
        &self,
        py: Python,
        path: Bound<'py, PyAny>,
        name: &str,
    ) -> PyResult<Py<PyAny>> {
        let bridge = global_registry::get_asset_bridge_by_name(name).ok_or_else(|| {
            PyRuntimeError::new_err(format!("Asset bridge for '{}' not found", name))
        })?;

        let world = self.world_ref()?;
        let asset_server = world.resource::<AssetServer>();
        let asset_path = extract_asset_path(&path)?;
        let untyped_handle = bridge.load(asset_server, asset_path);
        let py_handle = PyHandle::from_untyped(untyped_handle, bridge.py_type_ptr());
        py_handle.into_py_any(py)
    }
}
