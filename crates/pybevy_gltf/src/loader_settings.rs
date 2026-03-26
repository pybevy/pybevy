use bevy::gltf::GltfLoaderSettings;
use pybevy_image::{PyImageSamplerDescriptor, PyRenderAssetUsages};
use pyo3::prelude::*;

// NOTE: Bevy's GltfLoaderSettings also has a `convert_coordinates: Option<GltfConvertCoordinates>`
// field, but GltfConvertCoordinates is private in the published bevy_gltf 0.18.0 crate.
// This field will be exposed once Bevy makes the type public.
#[pyclass(name = "GltfLoaderSettings")]
#[derive(Default)]
pub struct PyGltfLoaderSettings {
    pub(crate) inner: GltfLoaderSettings,
}


#[pymethods]
impl PyGltfLoaderSettings {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        load_meshes = None,
        load_materials = None,
        load_cameras = true,
        load_lights = true,
        load_animations = true,
        include_source = false,
        default_sampler = None,
        override_sampler = false,
    ))]
    pub fn new(
        load_meshes: Option<PyRenderAssetUsages>,
        load_materials: Option<PyRenderAssetUsages>,
        load_cameras: bool,
        load_lights: bool,
        load_animations: bool,
        include_source: bool,
        default_sampler: Option<PyImageSamplerDescriptor>,
        override_sampler: bool,
    ) -> Self {
        Self {
            inner: GltfLoaderSettings {
                load_meshes: load_meshes
                    .map(Into::into)
                    .unwrap_or_default(),
                load_materials: load_materials
                    .map(Into::into)
                    .unwrap_or_default(),
                load_cameras,
                load_lights,
                load_animations,
                include_source,
                default_sampler: default_sampler.map(Into::into),
                override_sampler,
                ..Default::default()
            },
        }
    }

    #[getter]
    pub fn load_meshes(&self) -> PyRenderAssetUsages {
        self.inner.load_meshes.into()
    }

    #[setter]
    pub fn set_load_meshes(&mut self, value: PyRenderAssetUsages) {
        self.inner.load_meshes = value.into();
    }

    #[getter]
    pub fn load_materials(&self) -> PyRenderAssetUsages {
        self.inner.load_materials.into()
    }

    #[setter]
    pub fn set_load_materials(&mut self, value: PyRenderAssetUsages) {
        self.inner.load_materials = value.into();
    }

    #[getter]
    pub fn load_cameras(&self) -> bool {
        self.inner.load_cameras
    }

    #[setter]
    pub fn set_load_cameras(&mut self, value: bool) {
        self.inner.load_cameras = value;
    }

    #[getter]
    pub fn load_lights(&self) -> bool {
        self.inner.load_lights
    }

    #[setter]
    pub fn set_load_lights(&mut self, value: bool) {
        self.inner.load_lights = value;
    }

    #[getter]
    pub fn load_animations(&self) -> bool {
        self.inner.load_animations
    }

    #[setter]
    pub fn set_load_animations(&mut self, value: bool) {
        self.inner.load_animations = value;
    }

    #[getter]
    pub fn include_source(&self) -> bool {
        self.inner.include_source
    }

    #[setter]
    pub fn set_include_source(&mut self, value: bool) {
        self.inner.include_source = value;
    }

    #[getter]
    pub fn default_sampler(&self) -> Option<PyImageSamplerDescriptor> {
        self.inner.default_sampler.clone().map(Into::into)
    }

    #[setter]
    pub fn set_default_sampler(&mut self, value: Option<PyImageSamplerDescriptor>) {
        self.inner.default_sampler = value.map(Into::into);
    }

    #[getter]
    pub fn override_sampler(&self) -> bool {
        self.inner.override_sampler
    }

    #[setter]
    pub fn set_override_sampler(&mut self, value: bool) {
        self.inner.override_sampler = value;
    }

    pub fn __repr__(&self) -> String {
        format!(
            "GltfLoaderSettings(load_cameras={}, load_lights={}, load_animations={})",
            self.inner.load_cameras, self.inner.load_lights, self.inner.load_animations
        )
    }
}

impl From<GltfLoaderSettings> for PyGltfLoaderSettings {
    fn from(inner: GltfLoaderSettings) -> Self {
        Self { inner }
    }
}

impl From<PyGltfLoaderSettings> for GltfLoaderSettings {
    fn from(py: PyGltfLoaderSettings) -> Self {
        py.inner
    }
}
