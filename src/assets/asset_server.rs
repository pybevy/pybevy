use bevy::{
    asset::{
        AssetPath, AssetServer,
        saver::{SaveAssetError, SavedAsset, save_using_saver},
    },
    ecs::world::unsafe_world_cell::UnsafeWorldCell,
    image::{ImageSaver, ImageSaverSettings, SaveImageError},
    tasks::IoTaskPool,
};
use pybevy_audio::audio_source::PyAudioSource;
use pybevy_core::{
    asset_load_plan::AssetLoadPlan,
    extract_asset_id_from_any,
    handle::PyHandle,
    public_error::{
        ASSET_LOADING_TASK_POOL_MISSING, LOAD_BUILDER_TYPE_REQUIRED, LOAD_WITH_SETTINGS_DEPRECATED,
        LOADER_SETTINGS_BRIDGE_MISSING, asset_settings_type_mismatch, invalid_asset_type,
        removed_asset_load_method,
    },
    registry::global_registry,
};
use pybevy_image::{image::PyImage, image_saver_settings::PyImageSaverSettings};
use pyo3::{
    IntoPyObjectExt,
    exceptions::{
        PyAttributeError, PyDeprecationWarning, PyOSError, PyRuntimeError, PyTypeError,
        PyValueError,
    },
    prelude::*,
    types::{PyString, PyType},
};

use crate::{
    assets::{
        PyAssetPath, dependency_load_state::PyDependencyLoadState, load_builder::PyLoadBuilder,
        load_state::PyLoadState, recursive_dependency_load_state::PyRecursiveDependencyLoadState,
    },
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
#[pyclass(name = "AssetServer", module = "pybevy.assets", extends = PyResource, skip_from_py_object)]
#[derive(Debug, Clone)]
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
    pub(crate) fn asset_server(&self) -> PyResult<&AssetServer> {
        self.validity.check()?;
        // SAFETY: `Res`/`ResMut[AssetServer]` registers AssetServer's ComponentId in
        // DynamicSystem::initialize, so read access is declared; the executor prevents a
        // concurrent writer, so this unchecked resource read is unique. AssetServer's own
        // methods use interior mutability, so shared access suffices for load/query.
        unsafe { self.cell.get_resource::<AssetServer>() }
            .ok_or_else(|| PyRuntimeError::new_err("AssetServer resource not found"))
    }

    pub(crate) fn load_with_plan<'py>(
        &self,
        py: Python<'py>,
        path: Bound<'py, PyAny>,
        asset_type: Option<Bound<'py, PyType>>,
        plan: &AssetLoadPlan,
    ) -> PyResult<Py<PyAny>> {
        let server = self.loading_asset_server()?;
        let path = extract_asset_path(&path)?;
        let bridge = if let Some(asset_type) = asset_type {
            global_registry::get_asset_bridge_by_py_type(asset_type.as_type_ptr())
                .ok_or_else(|| PyTypeError::new_err(invalid_asset_type(&asset_type)))?
        } else {
            let (type_id, _) = plan
                .asset_type()
                .ok_or_else(|| PyTypeError::new_err(LOAD_BUILDER_TYPE_REQUIRED))?;
            global_registry::get_asset_bridge_by_type_id(type_id)
                .ok_or_else(|| PyTypeError::new_err(LOADER_SETTINGS_BRIDGE_MISSING))?
        };
        plan.validate_type(bridge.bevy_type_id(), bridge.name())
            .map_err(|(expected, actual)| {
                PyTypeError::new_err(asset_settings_type_mismatch(expected, actual))
            })?;
        if !bridge.is_loadable() {
            return Err(PyTypeError::new_err(format!(
                "`{}` is not a file-loadable asset type",
                bridge.name()
            )));
        }
        let handle = plan.load(server, path, bridge.bevy_type_id());
        PyHandle::from_untyped(handle, bridge.py_type_ptr()).into_py_any(py)
    }

    fn loading_asset_server(&self) -> PyResult<&AssetServer> {
        let server = self.asset_server()?;
        if IoTaskPool::try_get().is_none() {
            return Err(PyRuntimeError::new_err(ASSET_LOADING_TASK_POOL_MISSING));
        }
        Ok(server)
    }
}

#[pymethods]
impl PyAssetServer {
    #[pyo3(signature = (path, *, asset_type))]
    pub fn load<'py>(
        &self,
        py: Python<'py>,
        path: Bound<'py, PyAny>,
        asset_type: Bound<'py, PyType>,
    ) -> PyResult<Py<PyAny>> {
        self.load_with_plan(py, path, Some(asset_type), &AssetLoadPlan::default())
    }

    pub fn load_builder(&self) -> PyResult<PyLoadBuilder> {
        self.asset_server()?;
        Ok(PyLoadBuilder::new(self.clone()))
    }

    pub fn load_image<'py>(&self, py: Python<'py>, path: Bound<'py, PyAny>) -> PyResult<Py<PyAny>> {
        self.load(py, path, py.get_type::<PyImage>())
    }

    pub fn load_audio<'py>(&self, py: Python<'py>, path: Bound<'py, PyAny>) -> PyResult<Py<PyAny>> {
        self.load(py, path, py.get_type::<PyAudioSource>())
    }

    fn __getattr__(&self, py: Python<'_>, name: &str) -> PyResult<()> {
        let replacement = match name {
            "load_mesh" => "load(path, asset_type=Mesh)",
            "load_world_asset" => "load(path, asset_type=WorldAsset)",
            "load_image_with_settings" => "load_builder().with_settings(settings).load(path)",
            _ => {
                return Err(PyAttributeError::new_err(format!(
                    "'AssetServer' object has no attribute '{name}'"
                )));
            }
        };
        let error = PyAttributeError::new_err(removed_asset_load_method(name, replacement));
        // Explicit metadata prevents Python from suggesting an unrelated method.
        error.value(py).setattr("name", name)?;
        Err(error)
    }

    #[pyo3(signature = (path, settings, *, asset_type=None))]
    pub fn load_with_settings<'py>(
        &self,
        py: Python<'py>,
        path: Bound<'py, PyAny>,
        settings: Bound<'py, PyAny>,
        asset_type: Option<Bound<'py, PyType>>,
    ) -> PyResult<Py<PyAny>> {
        self.asset_server()?;
        PyErr::warn(
            py,
            &py.get_type::<PyDeprecationWarning>(),
            LOAD_WITH_SETTINGS_DEPRECATED,
            1,
        )?;
        self.load_builder()?
            .with_settings(settings)?
            .load(py, path, asset_type)
    }

    pub fn load_folder<'py>(&self, py: Python, path: Bound<'py, PyAny>) -> PyResult<Py<PyAny>> {
        let bridge = global_registry::get_asset_bridge_by_name("LoadedFolder")
            .ok_or_else(|| PyRuntimeError::new_err("Asset bridge for 'LoadedFolder' not found"))?;
        let asset_server = self.loading_asset_server()?;
        let asset_path = extract_asset_path(&path)?;
        let handle = asset_server.load_folder(asset_path).untyped();
        PyHandle::from_untyped(handle, bridge.py_type_ptr()).into_py_any(py)
    }

    pub fn load_state(&self, py: Python<'_>, id: &Bound<'_, PyAny>) -> PyResult<Py<PyLoadState>> {
        let asset_server = self.asset_server()?;
        PyLoadState::from_state(
            asset_server.load_state(extract_asset_id_from_any(id)?.untyped()),
            py,
        )
    }

    pub fn dependency_load_state(
        &self,
        py: Python<'_>,
        id: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyDependencyLoadState>> {
        let asset_server = self.asset_server()?;
        PyDependencyLoadState::from_state(
            asset_server.dependency_load_state(extract_asset_id_from_any(id)?.untyped()),
            py,
        )
    }

    pub fn recursive_dependency_load_state(
        &self,
        py: Python<'_>,
        id: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyRecursiveDependencyLoadState>> {
        let asset_server = self.asset_server()?;
        PyRecursiveDependencyLoadState::from_state(
            asset_server.recursive_dependency_load_state(extract_asset_id_from_any(id)?.untyped()),
            py,
        )
    }

    pub fn is_loaded(&self, id: &Bound<'_, PyAny>) -> PyResult<bool> {
        let asset_server = self.asset_server()?;
        Ok(asset_server.is_loaded(extract_asset_id_from_any(id)?.untyped()))
    }

    pub fn is_loaded_with_dependencies(&self, id: &Bound<'_, PyAny>) -> PyResult<bool> {
        let asset_server = self.asset_server()?;
        Ok(asset_server.is_loaded_with_dependencies(extract_asset_id_from_any(id)?.untyped()))
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

    #[pyo3(signature = (image, path, settings = None))]
    pub fn save_image(
        &self,
        py: Python,
        image: &PyImage,
        path: &Bound<'_, PyAny>,
        settings: Option<&PyImageSaverSettings>,
    ) -> PyResult<()> {
        let asset_server = self.asset_server()?;
        let asset_path = extract_asset_path(path)?;
        let image = image.storage.as_ref()?.clone();
        let settings: ImageSaverSettings = settings
            .map(|s| ImageSaverSettings::from(s.clone()))
            .unwrap_or_default();

        // Save path is pure file IO, never touches the World: blocking cannot deadlock
        let result = py.detach(|| {
            bevy::platform::future::block_on(save_using_saver(
                asset_server.clone(),
                &ImageSaver,
                &asset_path,
                SavedAsset::from_asset(&image),
                &settings,
            ))
        });
        result.map_err(save_asset_error_to_py)?;
        Ok(())
    }
}

fn save_asset_error_to_py(error: SaveAssetError) -> PyErr {
    match &error {
        SaveAssetError::MissingSource(_) | SaveAssetError::MissingWriter(_) => {
            PyRuntimeError::new_err(error.to_string())
        }
        SaveAssetError::WriterError(_) => PyOSError::new_err(error.to_string()),
        SaveAssetError::SaverError(bevy_error) => {
            match bevy_error.downcast_ref::<SaveImageError>() {
                Some(SaveImageError::IoError(_) | SaveImageError::ImageError(_)) => {
                    PyOSError::new_err(error.to_string())
                }
                _ => PyValueError::new_err(error.to_string()),
            }
        }
    }
}
