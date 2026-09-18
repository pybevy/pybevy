use bevy::{
    gltf::{GltfLoaderSettings, convert_coordinates::GltfConvertCoordinates},
    image::ImageSamplerDescriptor,
};
use pybevy_core::{FieldStorage, FromBorrowedStorage, ValueStorage, computed_owned};
use pybevy_image::{image::PyRenderAssetUsages, sampler_descriptor::PyImageSamplerDescriptor};
use pybevy_macros::pyfield;
use pyo3::prelude::*;

use crate::{
    convert_coordinates::PyGltfConvertCoordinates,
    skinned_mesh_bounds_policy::PyGltfSkinnedMeshBoundsPolicy,
};

#[derive(Default)]
pub struct GltfLoaderSettingsValue(pub GltfLoaderSettings);

impl GltfLoaderSettingsValue {
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
        } = &self.0;

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

impl Clone for GltfLoaderSettingsValue {
    fn clone(&self) -> Self {
        let mut settings = GltfLoaderSettings::default();
        self.apply_to(&mut settings);
        Self(settings)
    }
}

#[pyfield]
#[pyclass(
    name = "GltfLoaderSettings",
    module = "pybevy.gltf",
    skip_from_py_object
)]
pub struct PyGltfLoaderSettings {
    storage: FieldStorage<GltfLoaderSettingsValue>,
}

impl Default for PyGltfLoaderSettings {
    fn default() -> Self {
        Self::from_owned(GltfLoaderSettingsValue::default())
    }
}

impl PyGltfLoaderSettings {
    pub fn apply_to(&self, settings: &mut GltfLoaderSettings) -> PyResult<()> {
        self.as_ref()?.apply_to(settings);
        Ok(())
    }
}

#[pymethods]
impl PyGltfLoaderSettings {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        *,
        load_meshes = PyRenderAssetUsages::default(),
        load_materials = PyRenderAssetUsages::default(),
        load_cameras = true,
        load_lights = true,
        load_animations = true,
        include_source = false,
        default_sampler = None,
        override_sampler = false,
        validate = true,
        convert_coordinates = None,
        skinned_mesh_bounds_policy = None
    ))]
    pub fn new(
        load_meshes: PyRenderAssetUsages,
        load_materials: PyRenderAssetUsages,
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
        Ok(Self::from_owned(GltfLoaderSettingsValue(
            GltfLoaderSettings {
                load_meshes: load_meshes.try_into()?,
                load_materials: load_materials.try_into()?,
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
        )))
    }

    #[getter]
    pub fn load_meshes(&self) -> PyResult<PyRenderAssetUsages> {
        Ok(self.storage.borrow_resolved_field_as(
            |settings| &settings.0.load_meshes,
            |settings| &mut settings.0.load_meshes,
        )?)
    }

    #[setter]
    pub fn set_load_meshes(&mut self, value: PyRenderAssetUsages) -> PyResult<()> {
        let usage = value.try_into()?;
        self.as_mut()?.0.load_meshes = usage;
        Ok(())
    }

    #[getter]
    pub fn load_materials(&self) -> PyResult<PyRenderAssetUsages> {
        Ok(self.storage.borrow_resolved_field_as(
            |settings| &settings.0.load_materials,
            |settings| &mut settings.0.load_materials,
        )?)
    }

    #[setter]
    pub fn set_load_materials(&mut self, value: PyRenderAssetUsages) -> PyResult<()> {
        let usage = value.try_into()?;
        self.as_mut()?.0.load_materials = usage;
        Ok(())
    }

    #[getter]
    pub fn load_cameras(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.0.load_cameras)
    }

    #[setter]
    pub fn set_load_cameras(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.0.load_cameras = value;
        Ok(())
    }

    #[getter]
    pub fn load_lights(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.0.load_lights)
    }

    #[setter]
    pub fn set_load_lights(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.0.load_lights = value;
        Ok(())
    }

    #[getter]
    pub fn load_animations(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.0.load_animations)
    }

    #[setter]
    pub fn set_load_animations(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.0.load_animations = value;
        Ok(())
    }

    #[getter]
    pub fn include_source(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.0.include_source)
    }

    #[setter]
    pub fn set_include_source(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.0.include_source = value;
        Ok(())
    }

    #[getter]
    pub fn default_sampler(&self) -> PyResult<Option<PyImageSamplerDescriptor>> {
        let storage: Option<FieldStorage<ImageSamplerDescriptor>> = self
            .storage
            .borrow_optional_field(|settings: &GltfLoaderSettingsValue| {
                &settings.0.default_sampler
            })?;
        Ok(storage.map(FromBorrowedStorage::from_borrowed))
    }

    #[setter]
    pub fn set_default_sampler(&mut self, value: Option<PyImageSamplerDescriptor>) -> PyResult<()> {
        let sampler = value.map(ImageSamplerDescriptor::try_from).transpose()?;
        self.as_mut()?.0.default_sampler = sampler;
        Ok(())
    }

    #[getter]
    pub fn override_sampler(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.0.override_sampler)
    }

    #[setter]
    pub fn set_override_sampler(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.0.override_sampler = value;
        Ok(())
    }

    #[getter]
    pub fn validate(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.0.validate)
    }

    #[setter]
    pub fn set_validate(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.0.validate = value;
        Ok(())
    }

    #[getter]
    pub fn convert_coordinates(&self) -> PyResult<Option<PyGltfConvertCoordinates>> {
        let storage: Option<ValueStorage<GltfConvertCoordinates>> = self
            .storage
            .borrow_optional_field(|settings: &GltfLoaderSettingsValue| {
                &settings.0.convert_coordinates
            })?;
        Ok(storage.map(PyGltfConvertCoordinates::from_borrowed))
    }

    #[setter]
    pub fn set_convert_coordinates(
        &mut self,
        value: Option<PyGltfConvertCoordinates>,
    ) -> PyResult<()> {
        let converted = value.as_ref().map(TryInto::try_into).transpose()?;
        self.as_mut()?.0.convert_coordinates = converted;
        Ok(())
    }

    #[getter]
    pub fn skinned_mesh_bounds_policy(&self) -> PyResult<Option<PyGltfSkinnedMeshBoundsPolicy>> {
        Ok(computed_owned(
            self.as_ref()?.0.skinned_mesh_bounds_policy.map(Into::into),
        ))
    }

    #[setter]
    pub fn set_skinned_mesh_bounds_policy(
        &mut self,
        value: Option<PyGltfSkinnedMeshBoundsPolicy>,
    ) -> PyResult<()> {
        self.as_mut()?.0.skinned_mesh_bounds_policy = value.map(Into::into);
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let settings = self.as_ref()?;
        let flag = |value: bool| if value { "True" } else { "False" };
        Ok(format!(
            "GltfLoaderSettings(load_cameras={}, load_lights={}, load_animations={}, \
             include_source={}, override_sampler={}, validate={})",
            flag(settings.0.load_cameras),
            flag(settings.0.load_lights),
            flag(settings.0.load_animations),
            flag(settings.0.include_source),
            flag(settings.0.override_sampler),
            flag(settings.0.validate),
        ))
    }
}

impl From<GltfLoaderSettings> for PyGltfLoaderSettings {
    fn from(settings: GltfLoaderSettings) -> Self {
        Self::from_owned(GltfLoaderSettingsValue(settings))
    }
}

impl TryFrom<&PyGltfLoaderSettings> for GltfLoaderSettings {
    type Error = PyErr;

    fn try_from(py: &PyGltfLoaderSettings) -> PyResult<Self> {
        let mut settings = GltfLoaderSettings::default();
        py.apply_to(&mut settings)?;
        Ok(settings)
    }
}
