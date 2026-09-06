use bevy::{gltf::GltfLoaderSettings, image::ImageSamplerDescriptor};
use pybevy_image::{image::PyRenderAssetUsages, sampler_descriptor::PyImageSamplerDescriptor};
use pyo3::prelude::*;

use crate::{
    convert_coordinates::PyGltfConvertCoordinates,
    skinned_mesh_bounds_policy::PyGltfSkinnedMeshBoundsPolicy,
};

#[pyclass(
    name = "GltfLoaderSettings",
    module = "pybevy.gltf",
    skip_from_py_object
)]
#[derive(Default)]
pub struct PyGltfLoaderSettings {
    pub(crate) inner: GltfLoaderSettings,
}

impl Clone for PyGltfLoaderSettings {
    fn clone(&self) -> Self {
        let mut inner = GltfLoaderSettings::default();
        self.apply_to(&mut inner);
        Self { inner }
    }
}

impl PyGltfLoaderSettings {
    pub fn apply_to(&self, settings: &mut GltfLoaderSettings) {
        let GltfLoaderSettings {
            load_meshes,
            load_materials,
            load_cameras,
            load_lights,
            load_animations,
            include_source,
            default_sampler,
            override_sampler,
            validate,
            convert_coordinates,
            skinned_mesh_bounds_policy,
        } = &self.inner;

        *settings = GltfLoaderSettings {
            load_meshes: *load_meshes,
            load_materials: *load_materials,
            load_cameras: *load_cameras,
            load_lights: *load_lights,
            load_animations: *load_animations,
            include_source: *include_source,
            default_sampler: default_sampler.clone(),
            override_sampler: *override_sampler,
            validate: *validate,
            convert_coordinates: *convert_coordinates,
            skinned_mesh_bounds_policy: *skinned_mesh_bounds_policy,
        };
    }
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
        validate = true,
        convert_coordinates = None,
        skinned_mesh_bounds_policy = None,
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
        validate: bool,
        convert_coordinates: Option<PyGltfConvertCoordinates>,
        skinned_mesh_bounds_policy: Option<PyGltfSkinnedMeshBoundsPolicy>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: GltfLoaderSettings {
                load_meshes: load_meshes.map(Into::into).unwrap_or_default(),
                load_materials: load_materials.map(Into::into).unwrap_or_default(),
                load_cameras,
                load_lights,
                load_animations,
                include_source,
                default_sampler: default_sampler
                    .map(ImageSamplerDescriptor::try_from)
                    .transpose()?,
                override_sampler,
                validate,
                convert_coordinates: convert_coordinates
                    .as_ref()
                    .map(TryInto::try_into)
                    .transpose()?,
                skinned_mesh_bounds_policy: skinned_mesh_bounds_policy.map(Into::into),
            },
        })
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
    pub fn set_default_sampler(&mut self, value: Option<PyImageSamplerDescriptor>) -> PyResult<()> {
        self.inner.default_sampler = value.map(ImageSamplerDescriptor::try_from).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn override_sampler(&self) -> bool {
        self.inner.override_sampler
    }

    #[setter]
    pub fn set_override_sampler(&mut self, value: bool) {
        self.inner.override_sampler = value;
    }

    #[getter]
    pub fn validate(&self) -> bool {
        self.inner.validate
    }

    #[setter]
    pub fn set_validate(&mut self, value: bool) {
        self.inner.validate = value;
    }

    #[getter]
    pub fn convert_coordinates(&self) -> Option<PyGltfConvertCoordinates> {
        self.inner.convert_coordinates.map(Into::into)
    }

    #[setter]
    pub fn set_convert_coordinates(
        &mut self,
        value: Option<PyGltfConvertCoordinates>,
    ) -> PyResult<()> {
        self.inner.convert_coordinates = value.as_ref().map(TryInto::try_into).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn skinned_mesh_bounds_policy(&self) -> Option<PyGltfSkinnedMeshBoundsPolicy> {
        self.inner.skinned_mesh_bounds_policy.map(Into::into)
    }

    #[setter]
    pub fn set_skinned_mesh_bounds_policy(&mut self, value: Option<PyGltfSkinnedMeshBoundsPolicy>) {
        self.inner.skinned_mesh_bounds_policy = value.map(Into::into);
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

impl From<&PyGltfLoaderSettings> for GltfLoaderSettings {
    fn from(py: &PyGltfLoaderSettings) -> Self {
        let mut settings = GltfLoaderSettings::default();
        py.apply_to(&mut settings);
        settings
    }
}
