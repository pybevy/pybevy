use bevy::{
    asset::{AssetPath, AssetServer, UntypedAssetId},
    ecs::world::unsafe_world_cell::UnsafeWorldCell,
    image::Image,
};
use pybevy_core::{handle::PyHandle, public_error::invalid_asset_type, registry::global_registry};
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

/// #1: map a file extension to a registered, loadable asset bridge name so
/// `load(path)` can infer the asset type. Conservative on purpose: only
/// formats whose loaders ship today (fonts are excluded until the Font
/// loader lands).
fn bridge_name_for_extension(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "png" | "jpg" | "jpeg" | "bmp" | "tga" | "dds" | "ktx2" | "basis" | "exr"
        | "hdr" | "webp" | "qoi" | "pam" | "ppm" | "pgm" | "pbm" => Some("Image"),
        "ogg" | "oga" | "wav" | "mp3" | "flac" => Some("AudioSource"),
        "gltf" | "glb" => Some("Gltf"),
        _ => None,
    }
}

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
    /// World cell (lifetime-erased), valid only while `validity` is active. Only
    /// ever used to read the declared `AssetServer` resource, never `&World`.
    cell: UnsafeWorldCell<'static>,
    validity: ValidityFlag,
}

unsafe impl Send for PyAssetServer {}
unsafe impl Sync for PyAssetServer {}

impl PyAssetServer {
    /// # Safety
    /// `cell` must reference the World this AssetServer belongs to and must stay
    /// valid for as long as `validity` is active.
    pub(crate) unsafe fn new(cell: UnsafeWorldCell, validity: ValidityFlag) -> Self {
        // SAFETY: layout-preserving lifetime erasure of a Copy pointer type; the
        // cell is only touched while `validity` is active.
        let cell: UnsafeWorldCell<'static> = unsafe { std::mem::transmute(cell) };
        Self { cell, validity }
    }

    /// Borrow the `AssetServer` resource through the cell.
    fn asset_server(&self) -> PyResult<&AssetServer> {
        self.validity.check()?;
        // SAFETY: `Res`/`ResMut[AssetServer]` registers AssetServer's ComponentId in
        // DynamicSystem::initialize, so read access is declared; the executor prevents a
        // concurrent writer, so this unchecked resource read is unique. AssetServer's own
        // methods use interior mutability, so shared access suffices for load/query.
        unsafe { self.cell.get_resource::<AssetServer>() }
            .ok_or_else(|| PyRuntimeError::new_err("AssetServer resource not found"))
    }
}

#[pymethods]
impl PyAssetServer {
    #[pyo3(signature = (path, asset_type=None))]
    pub fn load<'py>(
        &self,
        py: Python,
        path: Bound<'py, PyAny>,
        asset_type: Option<Bound<'py, PyType>>,
    ) -> PyResult<Py<PyAny>> {
        // #1: no explicit type -> infer it from the file extension
        let Some(asset_type) = asset_type else {
            let asset_path = extract_asset_path(&path)?;
            let ext = asset_path.path().extension().and_then(|e| e.to_str()).unwrap_or("");
            let name = bridge_name_for_extension(ext).ok_or_else(|| {
                PyTypeError::new_err(format!(
                    "can't infer the asset type of '{}' — pass it explicitly, e.g. \
                     load(path, Image). Extensions recognized: images (png/jpg/jpeg/bmp/tga/\
                     dds/ktx2/basis/exr/hdr/webp/qoi/pam/ppm/pgm/pbm), audio (ogg/oga/wav/\
                     mp3/flac), and gltf/glb",
                    asset_path
                ))
            })?;
            return self.load_by_name(py, path, name);
        };
        let type_ptr = asset_type.as_type_ptr();
        let bridge = global_registry::get_asset_bridge_by_py_type(type_ptr)
            .ok_or_else(|| PyTypeError::new_err(invalid_asset_type(&asset_type)))?;

        if !bridge.is_loadable() {
            return Err(PyTypeError::new_err(format!(
                "`{}` is not a file-loadable asset type",
                bridge.name()
            )));
        }

        let asset_server = self.asset_server()?;
        let asset_path = extract_asset_path(&path)?;
        let untyped_handle = bridge.load(asset_server, asset_path);
        let py_handle = PyHandle::from_untyped(untyped_handle, type_ptr);
        py_handle.into_py_any(py)
    }

    pub fn load_world_asset<'py>(
        &self,
        py: Python,
        path: Bound<'py, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        self.load_by_name(py, path, "WorldAsset")
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
        let bridge = global_registry::get_asset_bridge_by_py_type(type_ptr)
            .ok_or_else(|| PyTypeError::new_err(invalid_asset_type(&asset_type)))?;

        let asset_server = self.asset_server()?;
        let asset_path = extract_asset_path(&path)?;

        let untyped_handle = match bridge.name() {
            "Image" => {
                let image_settings: PyImageLoaderSettings = settings.extract()?;
                let bevy_settings: bevy::image::ImageLoaderSettings = image_settings.into();
                asset_server
                    .load_builder()
                    .with_settings(move |s: &mut bevy::image::ImageLoaderSettings| {
                        *s = bevy_settings.clone();
                    })
                    .load::<Image>(asset_path)
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

        let asset_server = self.asset_server()?;
        let asset_path = extract_asset_path(&path)?;
        let bevy_settings: bevy::image::ImageLoaderSettings = settings.into();
        let untyped_handle = asset_server
            .load_builder()
            .with_settings(move |s: &mut bevy::image::ImageLoaderSettings| {
                *s = bevy_settings.clone();
            })
            .load::<Image>(asset_path)
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
        let bridge = global_registry::get_asset_bridge_by_name("LoadedFolder")
            .ok_or_else(|| PyRuntimeError::new_err("Asset bridge for 'LoadedFolder' not found"))?;
        let asset_server = self.asset_server()?;
        let asset_path = extract_asset_path(&path)?;
        let handle = asset_server.load_folder(asset_path).untyped();
        PyHandle::from_untyped(handle, bridge.py_type_ptr()).into_py_any(py)
    }

    pub fn load_state(&self, id: &PyHandle) -> PyResult<PyLoadState> {
        let asset_server = self.asset_server()?;
        let untyped_id: UntypedAssetId = id.clone().into();
        Ok(PyLoadState::from(asset_server.load_state(untyped_id)))
    }

    pub fn is_loaded(&self, id: &PyHandle) -> PyResult<bool> {
        let asset_server = self.asset_server()?;
        let untyped_id: UntypedAssetId = id.clone().into();
        Ok(asset_server.is_loaded(untyped_id))
    }

    pub fn is_loaded_with_dependencies(&self, id: &PyHandle) -> PyResult<bool> {
        let asset_server = self.asset_server()?;
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
        let bridge = global_registry::get_asset_bridge_by_py_type(type_ptr)
            .ok_or_else(|| PyTypeError::new_err(invalid_asset_type(&asset_type)))?;

        let asset_server = self.asset_server()?;
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

        let asset_server = self.asset_server()?;
        let asset_path = extract_asset_path(&path)?;
        let untyped_handle = bridge.load(asset_server, asset_path);
        let py_handle = PyHandle::from_untyped(untyped_handle, bridge.py_type_ptr());
        py_handle.into_py_any(py)
    }
}
