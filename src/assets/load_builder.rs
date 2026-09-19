use bevy::{
    gltf::{Gltf, GltfLoaderSettings},
    image::{Image, ImageLoaderSettings},
};
use pybevy_core::{
    asset_load_plan::AssetLoadPlan,
    public_error::{asset_settings_type_mismatch, unsupported_loader_settings},
};
use pybevy_gltf::loader_settings::{GltfLoaderSettingsValue, PyGltfLoaderSettings};
use pybevy_image::loader_settings::PyImageLoaderSettings;
use pyo3::{IntoPyObjectExt, exceptions::PyTypeError, prelude::*, types::PyType};

use super::asset_server::PyAssetServer;

#[pyclass(
    name = "LoadBuilder",
    module = "pybevy.assets",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyLoadBuilder {
    server: PyAssetServer,
    plan: AssetLoadPlan,
}

impl PyLoadBuilder {
    pub(crate) fn new(server: PyAssetServer) -> Self {
        Self {
            server,
            plan: AssetLoadPlan::default(),
        }
    }
}

#[pymethods]
impl PyLoadBuilder {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let _ = key;
        cls.into_py_any(cls.py())
    }

    pub fn with_settings(&self, settings: Bound<'_, PyAny>) -> PyResult<Self> {
        self.server.asset_server()?;
        let plan = if let Ok(settings) = settings.extract::<PyRef<'_, PyImageLoaderSettings>>() {
            let settings = ImageLoaderSettings::try_from(&*settings)?;
            self.plan
                .with_settings::<Image, ImageLoaderSettings>("Image", move |target| {
                    *target = settings.clone()
                })
        } else if let Ok(settings) = settings.extract::<PyRef<'_, PyGltfLoaderSettings>>() {
            let settings = GltfLoaderSettingsValue::try_from(&*settings)?;
            self.plan
                .with_settings::<Gltf, GltfLoaderSettings>("Gltf", move |target| {
                    settings.apply_to(target)
                })
        } else {
            return Err(PyTypeError::new_err(unsupported_loader_settings(
                settings.get_type().name()?.to_str()?,
            )));
        }
        .map_err(|(expected, actual)| {
            PyTypeError::new_err(asset_settings_type_mismatch(expected, actual))
        })?;
        Ok(Self {
            server: self.server.clone(),
            plan,
        })
    }

    #[pyo3(signature = (path, *, asset_type=None))]
    pub fn load<'py>(
        &self,
        py: Python<'py>,
        path: Bound<'py, PyAny>,
        asset_type: Option<Bound<'py, PyType>>,
    ) -> PyResult<Py<PyAny>> {
        self.server.load_with_plan(py, path, asset_type, &self.plan)
    }

    pub fn __copy__(&self) -> PyResult<Self> {
        self.server.asset_server()?;
        Ok(self.clone())
    }
}
