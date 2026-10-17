use std::collections::HashMap;

use bevy::{
    asset::{Asset, Handle},
    color::{Color, LinearRgba},
    gltf::{Gltf, GltfExtras, GltfMaterial, GltfMesh, GltfNode, GltfPrimitive, GltfSkin},
    image::Image,
    platform::collections::HashMap as BevyHashMap,
};
use pybevy_color::{color::PyColor, linear_rgba::PyLinearRgba};
use pybevy_core::{AssetStorage, PyAsset, PyHandle, ValueStorage, computed_owned};
use pybevy_macros::pyasset;
use pybevy_material::alpha_mode::{PyAlphaMode, checked_alpha_mode};
use pybevy_math::affine2::PyAffine2;
use pybevy_mesh::uv_channel::PyUvChannel;
use pybevy_render::face::PyFace;
use pybevy_transform::transform::PyTransform;
use pyo3::prelude::*;

use crate::{
    gltf_primitives::PyGltfPrimitives,
    handle_lists::{
        PyAnimationClipHandles, PyGltfMaterialHandles, PyGltfMeshHandles, PyGltfNodeHandles,
        PyGltfSceneHandles, PyGltfSkinHandles,
    },
    label::PyGltfAssetLabel,
};

fn extract_handle<A: Asset>(handle: PyHandle) -> PyResult<Handle<A>> {
    handle.try_into()
}

fn extract_handles<A: Asset>(handles: Vec<PyHandle>) -> PyResult<Vec<Handle<A>>> {
    handles.into_iter().map(extract_handle).collect()
}

fn named_handles<A: Asset>(
    handles: &BevyHashMap<Box<str>, Handle<A>>,
) -> HashMap<String, PyHandle> {
    handles
        .iter()
        .map(|(name, handle)| (name.to_string(), PyHandle::from(handle)))
        .collect()
}

fn extract_named_handles<A: Asset>(
    handles: HashMap<String, PyHandle>,
) -> PyResult<BevyHashMap<Box<str>, Handle<A>>> {
    handles
        .into_iter()
        .map(|(name, handle)| Ok((name.into_boxed_str(), extract_handle(handle)?)))
        .collect()
}

#[pyasset(GltfMaterial, bridge, domains = [perceptual_roughness => [finite], metallic => [finite], reflectance => [finite], specular_transmission => [finite], thickness => [finite], ior => [finite], attenuation_distance => [finite_or_pos_inf], clearcoat => [finite], clearcoat_perceptual_roughness => [finite], anisotropy_strength => [finite], anisotropy_rotation => [finite]])]
#[pyclass(name = "GltfMaterial", module = "pybevy.gltf", extends = PyAsset, skip_from_py_object)]
#[derive(Debug)]
pub struct PyGltfMaterial {
    pub(crate) storage: AssetStorage<GltfMaterial>,
}

#[pymethods]
impl PyGltfMaterial {
    #[new]
    #[pyo3(signature = (
        *,
        base_color = Color::WHITE.into(),
        base_color_channel = PyUvChannel::Uv0,
        base_color_texture = None,
        emissive = LinearRgba::BLACK.into(),
        emissive_channel = PyUvChannel::Uv0,
        emissive_texture = None,
        perceptual_roughness = 0.5,
        metallic = 0.0,
        metallic_roughness_channel = PyUvChannel::Uv0,
        metallic_roughness_texture = None,
        reflectance = 0.5,
        specular_tint = Color::WHITE.into(),
        specular_transmission = 0.0,
        thickness = 0.0,
        ior = 1.5,
        attenuation_distance = f32::INFINITY,
        attenuation_color = Color::WHITE.into(),
        normal_map_channel = PyUvChannel::Uv0,
        normal_map_texture = None,
        occlusion_channel = PyUvChannel::Uv0,
        occlusion_texture = None,
        clearcoat = 0.0,
        clearcoat_perceptual_roughness = 0.5,
        anisotropy_strength = 0.0,
        anisotropy_rotation = 0.0,
        double_sided = false,
        cull_mode = Some(PyFace::Back),
        unlit = false,
        alpha_mode = PyAlphaMode::Opaque(),
        uv_transform = PyAffine2::IDENTITY
    ))]
    #[allow(clippy::too_many_arguments, clippy::needless_update)]
    pub fn new(
        base_color: PyColor,
        base_color_channel: PyUvChannel,
        base_color_texture: Option<PyHandle>,
        emissive: PyLinearRgba,
        emissive_channel: PyUvChannel,
        emissive_texture: Option<PyHandle>,
        perceptual_roughness: f32,
        metallic: f32,
        metallic_roughness_channel: PyUvChannel,
        metallic_roughness_texture: Option<PyHandle>,
        reflectance: f32,
        specular_tint: PyColor,
        specular_transmission: f32,
        thickness: f32,
        ior: f32,
        attenuation_distance: f32,
        attenuation_color: PyColor,
        normal_map_channel: PyUvChannel,
        normal_map_texture: Option<PyHandle>,
        occlusion_channel: PyUvChannel,
        occlusion_texture: Option<PyHandle>,
        clearcoat: f32,
        clearcoat_perceptual_roughness: f32,
        anisotropy_strength: f32,
        anisotropy_rotation: f32,
        double_sided: bool,
        cull_mode: Option<PyFace>,
        unlit: bool,
        alpha_mode: PyAlphaMode,
        uv_transform: PyAffine2,
    ) -> PyResult<PyClassInitializer<Self>> {
        let material = GltfMaterial {
            base_color: base_color.try_into()?,
            base_color_channel: base_color_channel.into(),
            base_color_texture: base_color_texture.map(extract_handle).transpose()?,
            emissive: LinearRgba::try_from(&emissive)?,
            emissive_channel: emissive_channel.into(),
            emissive_texture: emissive_texture.map(extract_handle).transpose()?,
            perceptual_roughness: Self::check_perceptual_roughness(perceptual_roughness)?,
            metallic: Self::check_metallic(metallic)?,
            metallic_roughness_channel: metallic_roughness_channel.into(),
            metallic_roughness_texture: metallic_roughness_texture
                .map(extract_handle)
                .transpose()?,
            reflectance: Self::check_reflectance(reflectance)?,
            specular_tint: specular_tint.try_into()?,
            specular_transmission: Self::check_specular_transmission(specular_transmission)?,
            thickness: Self::check_thickness(thickness)?,
            ior: Self::check_ior(ior)?,
            attenuation_distance: Self::check_attenuation_distance(attenuation_distance)?,
            attenuation_color: attenuation_color.try_into()?,
            normal_map_channel: normal_map_channel.into(),
            normal_map_texture: normal_map_texture.map(extract_handle).transpose()?,
            occlusion_channel: occlusion_channel.into(),
            occlusion_texture: occlusion_texture.map(extract_handle).transpose()?,
            clearcoat: Self::check_clearcoat(clearcoat)?,
            clearcoat_perceptual_roughness: Self::check_clearcoat_perceptual_roughness(
                clearcoat_perceptual_roughness,
            )?,
            anisotropy_strength: Self::check_anisotropy_strength(anisotropy_strength)?,
            anisotropy_rotation: Self::check_anisotropy_rotation(anisotropy_rotation)?,
            double_sided,
            cull_mode: cull_mode.map(Into::into),
            unlit,
            alpha_mode: checked_alpha_mode(alpha_mode)?,
            uv_transform: uv_transform.try_into()?,
            // Fills Bevy's feature-gated PBR texture fields when enabled.
            ..GltfMaterial::default()
        };
        Ok(Self::from_owned(material).into())
    }

    #[getter]
    pub fn base_color(&self, py: Python<'_>) -> PyResult<Py<PyColor>> {
        let storage: ValueStorage<Color> = self.storage.borrow_field(
            |material| &material.base_color,
            |material| &mut material.base_color,
        )?;
        PyColor::from_storage(storage, py)
    }

    #[setter]
    pub fn set_base_color(&mut self, value: PyColor) -> PyResult<()> {
        let value = value.try_into()?;
        self.as_mut()?.base_color = value;
        Ok(())
    }

    #[getter]
    pub fn base_color_channel(&self) -> PyResult<PyUvChannel> {
        Ok(computed_owned(
            self.as_ref()?.base_color_channel.clone().into(),
        ))
    }

    #[setter]
    pub fn set_base_color_channel(&mut self, value: PyUvChannel) -> PyResult<()> {
        self.as_mut()?.base_color_channel = value.into();
        Ok(())
    }

    #[getter]
    pub fn base_color_texture(&self) -> PyResult<Option<PyHandle>> {
        Ok(self
            .as_ref()?
            .base_color_texture
            .as_ref()
            .map(PyHandle::from))
    }

    #[setter]
    pub fn set_base_color_texture(&mut self, value: Option<PyHandle>) -> PyResult<()> {
        self.as_mut()?.base_color_texture = value.map(extract_handle::<Image>).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn emissive(&self) -> PyResult<PyLinearRgba> {
        Ok(self.storage.borrow_field_as(
            |material| &material.emissive,
            |material| &mut material.emissive,
        )?)
    }

    #[setter]
    pub fn set_emissive(&mut self, value: PyLinearRgba) -> PyResult<()> {
        let value = LinearRgba::try_from(&value)?;
        self.as_mut()?.emissive = value;
        Ok(())
    }

    #[getter]
    pub fn emissive_channel(&self) -> PyResult<PyUvChannel> {
        Ok(computed_owned(
            self.as_ref()?.emissive_channel.clone().into(),
        ))
    }

    #[setter]
    pub fn set_emissive_channel(&mut self, value: PyUvChannel) -> PyResult<()> {
        self.as_mut()?.emissive_channel = value.into();
        Ok(())
    }

    #[getter]
    pub fn emissive_texture(&self) -> PyResult<Option<PyHandle>> {
        Ok(self.as_ref()?.emissive_texture.as_ref().map(PyHandle::from))
    }

    #[setter]
    pub fn set_emissive_texture(&mut self, value: Option<PyHandle>) -> PyResult<()> {
        self.as_mut()?.emissive_texture = value.map(extract_handle::<Image>).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn perceptual_roughness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.perceptual_roughness)
    }

    #[setter]
    pub fn set_perceptual_roughness(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_perceptual_roughness(value)?;
        self.as_mut()?.perceptual_roughness = value;
        Ok(())
    }

    #[getter]
    pub fn metallic(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.metallic)
    }

    #[setter]
    pub fn set_metallic(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_metallic(value)?;
        self.as_mut()?.metallic = value;
        Ok(())
    }

    #[getter]
    pub fn metallic_roughness_channel(&self) -> PyResult<PyUvChannel> {
        Ok(computed_owned(
            self.as_ref()?.metallic_roughness_channel.clone().into(),
        ))
    }

    #[setter]
    pub fn set_metallic_roughness_channel(&mut self, value: PyUvChannel) -> PyResult<()> {
        self.as_mut()?.metallic_roughness_channel = value.into();
        Ok(())
    }

    #[getter]
    pub fn metallic_roughness_texture(&self) -> PyResult<Option<PyHandle>> {
        Ok(self
            .as_ref()?
            .metallic_roughness_texture
            .as_ref()
            .map(PyHandle::from))
    }

    #[setter]
    pub fn set_metallic_roughness_texture(&mut self, value: Option<PyHandle>) -> PyResult<()> {
        self.as_mut()?.metallic_roughness_texture =
            value.map(extract_handle::<Image>).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn reflectance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.reflectance)
    }

    #[setter]
    pub fn set_reflectance(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_reflectance(value)?;
        self.as_mut()?.reflectance = value;
        Ok(())
    }

    #[getter]
    pub fn specular_tint(&self, py: Python<'_>) -> PyResult<Py<PyColor>> {
        let storage: ValueStorage<Color> = self.storage.borrow_field(
            |material| &material.specular_tint,
            |material| &mut material.specular_tint,
        )?;
        PyColor::from_storage(storage, py)
    }

    #[setter]
    pub fn set_specular_tint(&mut self, value: PyColor) -> PyResult<()> {
        let value = value.try_into()?;
        self.as_mut()?.specular_tint = value;
        Ok(())
    }

    #[getter]
    pub fn specular_transmission(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.specular_transmission)
    }

    #[setter]
    pub fn set_specular_transmission(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_specular_transmission(value)?;
        self.as_mut()?.specular_transmission = value;
        Ok(())
    }

    #[getter]
    pub fn thickness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.thickness)
    }

    #[setter]
    pub fn set_thickness(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_thickness(value)?;
        self.as_mut()?.thickness = value;
        Ok(())
    }

    #[getter]
    pub fn ior(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.ior)
    }

    #[setter]
    pub fn set_ior(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_ior(value)?;
        self.as_mut()?.ior = value;
        Ok(())
    }

    #[getter]
    pub fn attenuation_distance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.attenuation_distance)
    }

    #[setter]
    pub fn set_attenuation_distance(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_attenuation_distance(value)?;
        self.as_mut()?.attenuation_distance = value;
        Ok(())
    }

    #[getter]
    pub fn attenuation_color(&self, py: Python<'_>) -> PyResult<Py<PyColor>> {
        let storage: ValueStorage<Color> = self.storage.borrow_field(
            |material| &material.attenuation_color,
            |material| &mut material.attenuation_color,
        )?;
        PyColor::from_storage(storage, py)
    }

    #[setter]
    pub fn set_attenuation_color(&mut self, value: PyColor) -> PyResult<()> {
        let value = value.try_into()?;
        self.as_mut()?.attenuation_color = value;
        Ok(())
    }

    #[getter]
    pub fn normal_map_channel(&self) -> PyResult<PyUvChannel> {
        Ok(computed_owned(
            self.as_ref()?.normal_map_channel.clone().into(),
        ))
    }

    #[setter]
    pub fn set_normal_map_channel(&mut self, value: PyUvChannel) -> PyResult<()> {
        self.as_mut()?.normal_map_channel = value.into();
        Ok(())
    }

    #[getter]
    pub fn normal_map_texture(&self) -> PyResult<Option<PyHandle>> {
        Ok(self
            .as_ref()?
            .normal_map_texture
            .as_ref()
            .map(PyHandle::from))
    }

    #[setter]
    pub fn set_normal_map_texture(&mut self, value: Option<PyHandle>) -> PyResult<()> {
        self.as_mut()?.normal_map_texture = value.map(extract_handle::<Image>).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn occlusion_channel(&self) -> PyResult<PyUvChannel> {
        Ok(computed_owned(
            self.as_ref()?.occlusion_channel.clone().into(),
        ))
    }

    #[setter]
    pub fn set_occlusion_channel(&mut self, value: PyUvChannel) -> PyResult<()> {
        self.as_mut()?.occlusion_channel = value.into();
        Ok(())
    }

    #[getter]
    pub fn occlusion_texture(&self) -> PyResult<Option<PyHandle>> {
        Ok(self
            .as_ref()?
            .occlusion_texture
            .as_ref()
            .map(PyHandle::from))
    }

    #[setter]
    pub fn set_occlusion_texture(&mut self, value: Option<PyHandle>) -> PyResult<()> {
        self.as_mut()?.occlusion_texture = value.map(extract_handle::<Image>).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn clearcoat(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.clearcoat)
    }

    #[setter]
    pub fn set_clearcoat(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_clearcoat(value)?;
        self.as_mut()?.clearcoat = value;
        Ok(())
    }

    #[getter]
    pub fn clearcoat_perceptual_roughness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.clearcoat_perceptual_roughness)
    }

    #[setter]
    pub fn set_clearcoat_perceptual_roughness(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_clearcoat_perceptual_roughness(value)?;
        self.as_mut()?.clearcoat_perceptual_roughness = value;
        Ok(())
    }

    #[getter]
    pub fn anisotropy_strength(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.anisotropy_strength)
    }

    #[setter]
    pub fn set_anisotropy_strength(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_anisotropy_strength(value)?;
        self.as_mut()?.anisotropy_strength = value;
        Ok(())
    }

    #[getter]
    pub fn anisotropy_rotation(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.anisotropy_rotation)
    }

    #[setter]
    pub fn set_anisotropy_rotation(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_anisotropy_rotation(value)?;
        self.as_mut()?.anisotropy_rotation = value;
        Ok(())
    }

    #[getter]
    pub fn double_sided(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.double_sided)
    }

    #[setter]
    pub fn set_double_sided(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.double_sided = value;
        Ok(())
    }

    #[getter]
    pub fn cull_mode(&self) -> PyResult<Option<PyFace>> {
        Ok(computed_owned(self.as_ref()?.cull_mode.map(Into::into)))
    }

    #[setter]
    pub fn set_cull_mode(&mut self, value: Option<PyFace>) -> PyResult<()> {
        self.as_mut()?.cull_mode = value.map(Into::into);
        Ok(())
    }

    #[getter]
    pub fn unlit(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.unlit)
    }

    #[setter]
    pub fn set_unlit(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.unlit = value;
        Ok(())
    }

    #[getter]
    pub fn alpha_mode(&self) -> PyResult<PyAlphaMode> {
        Ok(computed_owned(self.as_ref()?.alpha_mode.into()))
    }

    #[setter]
    pub fn set_alpha_mode(&mut self, value: PyAlphaMode) -> PyResult<()> {
        let value = checked_alpha_mode(value)?;
        self.as_mut()?.alpha_mode = value;
        Ok(())
    }

    #[getter]
    pub fn uv_transform(&self) -> PyResult<PyAffine2> {
        Ok(self.storage.borrow_field_as(
            |material| &material.uv_transform,
            |material| &mut material.uv_transform,
        )?)
    }

    #[setter]
    pub fn set_uv_transform(&mut self, value: PyAffine2) -> PyResult<()> {
        let value = value.try_into()?;
        self.as_mut()?.uv_transform = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let material = self.as_ref()?;
        Ok(format!(
            "GltfMaterial(base_color={}, metallic={:?}, perceptual_roughness={:?}, unlit={})",
            PyColor::from(material.base_color).__repr__()?,
            material.metallic,
            material.perceptual_roughness,
            if material.unlit { "True" } else { "False" },
        ))
    }
}

#[pyasset(Gltf, no_clone, bridge)]
#[pyclass(name = "Gltf", module = "pybevy.gltf", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltf {
    pub(crate) storage: AssetStorage<Gltf>,
}

#[pymethods]
impl PyGltf {
    #[getter]
    pub fn scenes(&self) -> PyResult<PyGltfSceneHandles> {
        Ok(self
            .storage
            .borrow_field_as(|gltf| &gltf.scenes, |gltf| &mut gltf.scenes)?)
    }

    #[setter]
    pub fn set_scenes(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.scenes = extract_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn named_scenes(&self) -> PyResult<HashMap<String, PyHandle>> {
        Ok(named_handles(&self.storage.as_ref()?.named_scenes))
    }

    #[setter]
    pub fn set_named_scenes(&mut self, value: HashMap<String, PyHandle>) -> PyResult<()> {
        self.as_mut()?.named_scenes = extract_named_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn meshes(&self) -> PyResult<PyGltfMeshHandles> {
        Ok(self
            .storage
            .borrow_field_as(|gltf| &gltf.meshes, |gltf| &mut gltf.meshes)?)
    }

    #[setter]
    pub fn set_meshes(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.meshes = extract_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn named_meshes(&self) -> PyResult<HashMap<String, PyHandle>> {
        Ok(named_handles(&self.storage.as_ref()?.named_meshes))
    }

    #[setter]
    pub fn set_named_meshes(&mut self, value: HashMap<String, PyHandle>) -> PyResult<()> {
        self.as_mut()?.named_meshes = extract_named_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn materials(&self) -> PyResult<PyGltfMaterialHandles> {
        Ok(self
            .storage
            .borrow_field_as(|gltf| &gltf.materials, |gltf| &mut gltf.materials)?)
    }

    #[setter]
    pub fn set_materials(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.materials = extract_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn named_materials(&self) -> PyResult<HashMap<String, PyHandle>> {
        Ok(named_handles(&self.storage.as_ref()?.named_materials))
    }

    #[setter]
    pub fn set_named_materials(&mut self, value: HashMap<String, PyHandle>) -> PyResult<()> {
        self.as_mut()?.named_materials = extract_named_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn nodes(&self) -> PyResult<PyGltfNodeHandles> {
        Ok(self
            .storage
            .borrow_field_as(|gltf| &gltf.nodes, |gltf| &mut gltf.nodes)?)
    }

    #[setter]
    pub fn set_nodes(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.nodes = extract_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn named_nodes(&self) -> PyResult<HashMap<String, PyHandle>> {
        Ok(named_handles(&self.storage.as_ref()?.named_nodes))
    }

    #[setter]
    pub fn set_named_nodes(&mut self, value: HashMap<String, PyHandle>) -> PyResult<()> {
        self.as_mut()?.named_nodes = extract_named_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn default_scene(&self) -> PyResult<Option<PyHandle>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf.default_scene.as_ref().map(PyHandle::from))
    }

    #[setter]
    pub fn set_default_scene(&mut self, value: Option<PyHandle>) -> PyResult<()> {
        self.as_mut()?.default_scene = value.map(extract_handle).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn skins(&self) -> PyResult<PyGltfSkinHandles> {
        Ok(self
            .storage
            .borrow_field_as(|gltf| &gltf.skins, |gltf| &mut gltf.skins)?)
    }

    #[setter]
    pub fn set_skins(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.skins = extract_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn named_skins(&self) -> PyResult<HashMap<String, PyHandle>> {
        Ok(named_handles(&self.storage.as_ref()?.named_skins))
    }

    #[setter]
    pub fn set_named_skins(&mut self, value: HashMap<String, PyHandle>) -> PyResult<()> {
        self.as_mut()?.named_skins = extract_named_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn animations(&self) -> PyResult<PyAnimationClipHandles> {
        Ok(self
            .storage
            .borrow_field_as(|gltf| &gltf.animations, |gltf| &mut gltf.animations)?)
    }

    #[setter]
    pub fn set_animations(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.animations = extract_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn named_animations(&self) -> PyResult<HashMap<String, PyHandle>> {
        Ok(named_handles(&self.storage.as_ref()?.named_animations))
    }

    #[setter]
    pub fn set_named_animations(&mut self, value: HashMap<String, PyHandle>) -> PyResult<()> {
        self.as_mut()?.named_animations = extract_named_handles(value)?;
        Ok(())
    }
}

#[pyasset(GltfMesh, no_clone, bridge)]
#[pyclass(name = "GltfMesh", module = "pybevy.gltf", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltfMesh {
    pub(crate) storage: AssetStorage<GltfMesh>,
}

#[pymethods]
impl PyGltfMesh {
    #[getter]
    pub fn index(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.index)
    }

    #[setter]
    pub fn set_index(&mut self, value: usize) -> PyResult<()> {
        self.as_mut()?.index = value;
        Ok(())
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.storage.as_ref()?.name.clone())
    }

    #[setter]
    pub fn set_name(&mut self, value: String) -> PyResult<()> {
        self.as_mut()?.name = value;
        Ok(())
    }

    #[getter]
    pub fn primitives(&self) -> PyResult<PyGltfPrimitives> {
        Ok(self
            .storage
            .borrow_field_as(|mesh| &mesh.primitives, |mesh| &mut mesh.primitives)?)
    }

    #[setter]
    pub fn set_primitives(&mut self, value: Vec<PyRef<'_, PyGltfPrimitive>>) -> PyResult<()> {
        let primitives = value
            .into_iter()
            .map(|primitive| Ok(PyGltfPrimitive::as_ref(&primitive)?.clone()))
            .collect::<PyResult<Vec<_>>>()?;
        self.as_mut()?.primitives = primitives;
        Ok(())
    }

    #[getter]
    pub fn extras(&self) -> PyResult<Option<String>> {
        let mesh = self.storage.as_ref()?;
        Ok(mesh.extras.as_ref().map(|e| e.value.clone()))
    }

    #[setter]
    pub fn set_extras(&mut self, value: Option<String>) -> PyResult<()> {
        self.as_mut()?.extras = value.map(|value| GltfExtras { value });
        Ok(())
    }

    pub fn asset_label(&self) -> PyResult<PyGltfAssetLabel> {
        Ok(PyGltfAssetLabel::Mesh {
            index: self.storage.as_ref()?.index,
        })
    }
}

#[pyasset(GltfNode, no_clone, bridge)]
#[pyclass(name = "GltfNode", module = "pybevy.gltf", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltfNode {
    pub(crate) storage: AssetStorage<GltfNode>,
}

#[pymethods]
impl PyGltfNode {
    #[getter]
    pub fn index(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.index)
    }

    #[setter]
    pub fn set_index(&mut self, value: usize) -> PyResult<()> {
        self.as_mut()?.index = value;
        Ok(())
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.storage.as_ref()?.name.clone())
    }

    #[setter]
    pub fn set_name(&mut self, value: String) -> PyResult<()> {
        self.as_mut()?.name = value;
        Ok(())
    }

    #[getter]
    pub fn children(&self) -> PyResult<PyGltfNodeHandles> {
        Ok(self
            .storage
            .borrow_field_as(|node| &node.children, |node| &mut node.children)?)
    }

    #[setter]
    pub fn set_children(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.children = extract_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn mesh(&self) -> PyResult<Option<PyHandle>> {
        let node = self.storage.as_ref()?;
        Ok(node.mesh.as_ref().map(PyHandle::from))
    }

    #[setter]
    pub fn set_mesh(&mut self, value: Option<PyHandle>) -> PyResult<()> {
        self.as_mut()?.mesh = value.map(extract_handle).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn skin(&self) -> PyResult<Option<PyHandle>> {
        let node = self.storage.as_ref()?;
        Ok(node.skin.as_ref().map(PyHandle::from))
    }

    #[setter]
    pub fn set_skin(&mut self, value: Option<PyHandle>) -> PyResult<()> {
        self.as_mut()?.skin = value.map(extract_handle).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn transform(&self, py: Python<'_>) -> PyResult<Py<PyTransform>> {
        let storage = self
            .storage
            .borrow_field(|node| &node.transform, |node| &mut node.transform)?;
        Py::new(py, PyTransform::from_borrowed(storage))
    }

    #[setter]
    pub fn set_transform(&mut self, value: PyRef<'_, PyTransform>) -> PyResult<()> {
        self.as_mut()?.transform = *PyTransform::as_ref(&value)?;
        Ok(())
    }

    #[getter]
    pub fn extras(&self) -> PyResult<Option<String>> {
        let node = self.storage.as_ref()?;
        Ok(node.extras.as_ref().map(|e| e.value.clone()))
    }

    #[setter]
    pub fn set_extras(&mut self, value: Option<String>) -> PyResult<()> {
        self.as_mut()?.extras = value.map(|value| GltfExtras { value });
        Ok(())
    }

    pub fn asset_label(&self) -> PyResult<PyGltfAssetLabel> {
        Ok(PyGltfAssetLabel::Node {
            index: self.storage.as_ref()?.index,
        })
    }
}

#[pyasset(GltfPrimitive, no_clone, bridge)]
#[pyclass(name = "GltfPrimitive", module = "pybevy.gltf", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltfPrimitive {
    pub(crate) storage: AssetStorage<GltfPrimitive>,
}

#[pymethods]
impl PyGltfPrimitive {
    #[getter]
    pub fn index(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.index)
    }

    #[setter]
    pub fn set_index(&mut self, value: usize) -> PyResult<()> {
        self.as_mut()?.index = value;
        Ok(())
    }

    #[getter]
    pub fn parent_mesh_index(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.parent_mesh_index)
    }

    #[setter]
    pub fn set_parent_mesh_index(&mut self, value: usize) -> PyResult<()> {
        self.as_mut()?.parent_mesh_index = value;
        Ok(())
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.storage.as_ref()?.name.clone())
    }

    #[setter]
    pub fn set_name(&mut self, value: String) -> PyResult<()> {
        self.as_mut()?.name = value;
        Ok(())
    }

    #[getter]
    pub fn mesh(&self) -> PyResult<PyHandle> {
        let primitive = self.storage.as_ref()?;
        Ok(PyHandle::from(&primitive.mesh))
    }

    #[setter]
    pub fn set_mesh(&mut self, value: PyHandle) -> PyResult<()> {
        self.as_mut()?.mesh = extract_handle(value)?;
        Ok(())
    }

    #[getter]
    pub fn material(&self) -> PyResult<Option<PyHandle>> {
        let primitive = self.storage.as_ref()?;
        Ok(primitive.material.as_ref().map(PyHandle::from))
    }

    #[setter]
    pub fn set_material(&mut self, value: Option<PyHandle>) -> PyResult<()> {
        self.as_mut()?.material = value.map(extract_handle).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn extras(&self) -> PyResult<Option<String>> {
        let primitive = self.storage.as_ref()?;
        Ok(primitive.extras.as_ref().map(|e| e.value.clone()))
    }

    #[setter]
    pub fn set_extras(&mut self, value: Option<String>) -> PyResult<()> {
        self.as_mut()?.extras = value.map(|value| GltfExtras { value });
        Ok(())
    }

    #[getter]
    pub fn material_extras(&self) -> PyResult<Option<String>> {
        let primitive = self.storage.as_ref()?;
        Ok(primitive.material_extras.as_ref().map(|e| e.value.clone()))
    }

    #[setter]
    pub fn set_material_extras(&mut self, value: Option<String>) -> PyResult<()> {
        self.as_mut()?.material_extras = value.map(|value| GltfExtras { value });
        Ok(())
    }

    pub fn asset_label(&self) -> PyResult<PyGltfAssetLabel> {
        let prim = self.storage.as_ref()?;
        Ok(PyGltfAssetLabel::Primitive {
            mesh: prim.parent_mesh_index,
            primitive: prim.index,
        })
    }
}

#[pyasset(GltfSkin, no_clone, bridge)]
#[pyclass(name = "GltfSkin", module = "pybevy.gltf", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltfSkin {
    pub(crate) storage: AssetStorage<GltfSkin>,
}

#[pymethods]
impl PyGltfSkin {
    #[getter]
    pub fn index(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.index)
    }

    #[setter]
    pub fn set_index(&mut self, value: usize) -> PyResult<()> {
        self.as_mut()?.index = value;
        Ok(())
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.storage.as_ref()?.name.clone())
    }

    #[setter]
    pub fn set_name(&mut self, value: String) -> PyResult<()> {
        self.as_mut()?.name = value;
        Ok(())
    }

    #[getter]
    pub fn joints(&self) -> PyResult<PyGltfNodeHandles> {
        Ok(self
            .storage
            .borrow_field_as(|skin| &skin.joints, |skin| &mut skin.joints)?)
    }

    #[setter]
    pub fn set_joints(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.joints = extract_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn inverse_bind_matrices(&self) -> PyResult<PyHandle> {
        let skin = self.storage.as_ref()?;
        Ok(PyHandle::from(&skin.inverse_bind_matrices))
    }

    #[setter]
    pub fn set_inverse_bind_matrices(&mut self, value: PyHandle) -> PyResult<()> {
        self.as_mut()?.inverse_bind_matrices = extract_handle(value)?;
        Ok(())
    }

    #[getter]
    pub fn extras(&self) -> PyResult<Option<String>> {
        let skin = self.storage.as_ref()?;
        Ok(skin.extras.as_ref().map(|e| e.value.clone()))
    }

    #[setter]
    pub fn set_extras(&mut self, value: Option<String>) -> PyResult<()> {
        self.as_mut()?.extras = value.map(|value| GltfExtras { value });
        Ok(())
    }

    pub fn asset_label(&self) -> PyResult<PyGltfAssetLabel> {
        Ok(PyGltfAssetLabel::Skin {
            index: self.storage.as_ref()?.index,
        })
    }
}
