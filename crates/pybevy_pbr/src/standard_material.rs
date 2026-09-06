use bevy::{
    asset::Handle,
    color::{Color, LinearRgba},
    image::Image,
    pbr::StandardMaterial,
};
use pybevy_color::{color::PyColor, linear_rgba::PyLinearRgba};
#[cfg(not(feature = "pbr_anisotropy_texture"))]
use pybevy_core::public_error::ANISOTROPY_TEXTURE_UNAVAILABLE;
#[cfg(not(feature = "pbr_multi_layer_material_textures"))]
use pybevy_core::public_error::MULTI_LAYER_MATERIAL_TEXTURES_UNAVAILABLE;
#[cfg(not(feature = "pbr_specular_textures"))]
use pybevy_core::public_error::SPECULAR_TEXTURES_UNAVAILABLE;
#[cfg(not(feature = "pbr_transmission_textures"))]
use pybevy_core::public_error::TRANSMISSION_TEXTURES_UNAVAILABLE;
use pybevy_core::{
    AssetInputConverter, AssetStorage, PyHandle, PyMaterial, PyMaterializable, ValueStorage,
    extract_handle_from_any,
};
use pybevy_macros::pyasset;
use pybevy_material::{alpha_mode::PyAlphaMode, opaque_renderer_method::PyOpaqueRendererMethod};
use pybevy_math::affine2::PyAffine2;
use pybevy_mesh::uv_channel::PyUvChannel;
use pybevy_render::face::PyFace;
#[cfg(not(all(
    feature = "pbr_anisotropy_texture",
    feature = "pbr_transmission_textures",
    feature = "pbr_specular_textures",
    feature = "pbr_multi_layer_material_textures",
)))]
use pyo3::exceptions::PyRuntimeError;
use pyo3::{exceptions::PyTypeError, prelude::*};

use crate::parallax_mapping_method::PyParallaxMappingMethod;

fn extract_linear_rgba(value: &Bound<'_, PyAny>) -> PyResult<LinearRgba> {
    if let Ok(color) = value.extract::<PyColor>() {
        Ok(LinearRgba::from(color.resolved_copy()?))
    } else if let Ok(linear) = value.extract::<PyLinearRgba>() {
        Ok(LinearRgba::try_from(&linear)?)
    } else {
        Err(PyTypeError::new_err("expected Color or LinearRgba"))
    }
}

fn convert_optional_handle(handle: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Handle<Image>>> {
    match handle {
        Some(h) => {
            let extracted = extract_handle_from_any(h)?;
            Ok(Some((&extracted).try_into()?))
        }
        None => Ok(None),
    }
}

#[pyasset(StandardMaterial, bridge, input_converter, material)]
#[pyclass(name = "StandardMaterial", module = "pybevy.pbr", extends = PyMaterial, skip_from_py_object)]
#[derive(Debug)]
pub struct PyStandardMaterial {
    pub(crate) storage: AssetStorage<StandardMaterial>,
}

impl AssetInputConverter for PyStandardMaterial {
    fn try_convert_input<'py>(
        asset: &Bound<'py, PyAny>,
        _py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        if asset.is_instance_of::<PyMaterializable>() {
            return Ok(Some(asset.call_method0("materialize")?));
        }
        Ok(None)
    }
}

#[pymethods]
impl PyStandardMaterial {
    #[new]
    #[pyo3(signature = (
        base_color = Color::WHITE.into(),
        base_color_texture = None,
        base_color_channel = PyUvChannel::Uv0,
        emissive = None,
        emissive_exposure_weight = 0.0,
        emissive_channel = PyUvChannel::Uv0,
        emissive_texture = None,
        perceptual_roughness = 0.5,
        metallic = 0.0,
        metallic_roughness_channel = PyUvChannel::Uv0,
        metallic_roughness_texture = None,
        reflectance = 0.5,
        specular_tint = Color::WHITE.into(),
        diffuse_transmission = 0.0,
        diffuse_transmission_channel = PyUvChannel::Uv0,
        diffuse_transmission_texture = None,
        specular_transmission = 0.0,
        specular_transmission_channel = PyUvChannel::Uv0,
        specular_transmission_texture = None,
        thickness = 0.0,
        thickness_channel = PyUvChannel::Uv0,
        thickness_texture = None,
        ior = 1.5,
        attenuation_distance = f32::INFINITY,
        attenuation_color = Color::WHITE.into(),
        normal_map_channel = PyUvChannel::Uv0,
        normal_map_texture = None,
        flip_normal_map_y = false,
        occlusion_channel = PyUvChannel::Uv0,
        occlusion_texture = None,
        specular_channel = PyUvChannel::Uv0,
        specular_texture = None,
        specular_tint_channel = PyUvChannel::Uv0,
        specular_tint_texture = None,
        anisotropy_strength = 0.0,
        anisotropy_rotation = 0.0,
        anisotropy_channel = PyUvChannel::Uv0,
        anisotropy_texture = None,
        double_sided = false,
        cull_mode = Some(PyFace::Back),
        unlit = false,
        fog_enabled = true,
        alpha_mode = PyAlphaMode::Opaque(),
        depth_bias = 0.0,
        depth_map = None,
        parallax_depth_scale = 0.1,
        parallax_mapping_method = PyParallaxMappingMethod::Occlusion(),
        max_parallax_layer_count = 16.0,
        lightmap_exposure = 1.0,
        opaque_render_method = PyOpaqueRendererMethod::Auto,
        deferred_lighting_pass_id = 1,
        uv_transform = PyAffine2::IDENTITY,
        clearcoat = 0.0,
        clearcoat_perceptual_roughness = 0.5,
        clearcoat_channel = PyUvChannel::Uv0,
        clearcoat_texture = None,
        clearcoat_roughness_channel = PyUvChannel::Uv0,
        clearcoat_roughness_texture = None,
        clearcoat_normal_channel = PyUvChannel::Uv0,
        clearcoat_normal_texture = None
    ))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        base_color: PyColor,
        base_color_texture: Option<&Bound<'_, PyAny>>,
        base_color_channel: PyUvChannel,
        emissive: Option<Bound<'_, PyAny>>,
        emissive_exposure_weight: f32,
        emissive_channel: PyUvChannel,
        emissive_texture: Option<&Bound<'_, PyAny>>,
        perceptual_roughness: f32,
        metallic: f32,
        metallic_roughness_channel: PyUvChannel,
        metallic_roughness_texture: Option<&Bound<'_, PyAny>>,
        reflectance: f32,
        specular_tint: PyColor,
        diffuse_transmission: f32,
        diffuse_transmission_channel: PyUvChannel,
        diffuse_transmission_texture: Option<&Bound<'_, PyAny>>,
        specular_transmission: f32,
        specular_transmission_channel: PyUvChannel,
        specular_transmission_texture: Option<&Bound<'_, PyAny>>,
        thickness: f32,
        thickness_channel: PyUvChannel,
        thickness_texture: Option<&Bound<'_, PyAny>>,
        ior: f32,
        attenuation_distance: f32,
        attenuation_color: PyColor,
        normal_map_channel: PyUvChannel,
        normal_map_texture: Option<&Bound<'_, PyAny>>,
        flip_normal_map_y: bool,
        occlusion_channel: PyUvChannel,
        occlusion_texture: Option<&Bound<'_, PyAny>>,
        specular_channel: PyUvChannel,
        specular_texture: Option<&Bound<'_, PyAny>>,
        specular_tint_channel: PyUvChannel,
        specular_tint_texture: Option<&Bound<'_, PyAny>>,
        anisotropy_strength: f32,
        anisotropy_rotation: f32,
        anisotropy_channel: PyUvChannel,
        anisotropy_texture: Option<&Bound<'_, PyAny>>,
        double_sided: bool,
        cull_mode: Option<PyFace>,
        unlit: bool,
        fog_enabled: bool,
        alpha_mode: PyAlphaMode,
        depth_bias: f32,
        depth_map: Option<&Bound<'_, PyAny>>,
        parallax_depth_scale: f32,
        parallax_mapping_method: PyParallaxMappingMethod,
        max_parallax_layer_count: f32,
        lightmap_exposure: f32,
        opaque_render_method: PyOpaqueRendererMethod,
        deferred_lighting_pass_id: u8,
        uv_transform: PyAffine2,
        clearcoat: f32,
        clearcoat_perceptual_roughness: f32,
        clearcoat_channel: PyUvChannel,
        clearcoat_texture: Option<&Bound<'_, PyAny>>,
        clearcoat_roughness_channel: PyUvChannel,
        clearcoat_roughness_texture: Option<&Bound<'_, PyAny>>,
        clearcoat_normal_channel: PyUvChannel,
        clearcoat_normal_texture: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let emissive_linear = match emissive {
            Some(ref emissive_val) => extract_linear_rgba(emissive_val)?,
            None => LinearRgba::from(Color::BLACK),
        };

        // The param stays in the signature on every platform so the stub and
        // surface contract are platform-independent; only the value is refused.
        #[cfg(not(feature = "pbr_anisotropy_texture"))]
        if anisotropy_texture.is_some() || anisotropy_channel != PyUvChannel::Uv0 {
            return Err(PyRuntimeError::new_err(ANISOTROPY_TEXTURE_UNAVAILABLE));
        }

        #[cfg(not(feature = "pbr_transmission_textures"))]
        if diffuse_transmission_texture.is_some()
            || diffuse_transmission_channel != PyUvChannel::Uv0
            || specular_transmission_texture.is_some()
            || specular_transmission_channel != PyUvChannel::Uv0
            || thickness_texture.is_some()
            || thickness_channel != PyUvChannel::Uv0
        {
            return Err(PyRuntimeError::new_err(TRANSMISSION_TEXTURES_UNAVAILABLE));
        }

        #[cfg(not(feature = "pbr_specular_textures"))]
        if specular_texture.is_some()
            || specular_channel != PyUvChannel::Uv0
            || specular_tint_texture.is_some()
            || specular_tint_channel != PyUvChannel::Uv0
        {
            return Err(PyRuntimeError::new_err(SPECULAR_TEXTURES_UNAVAILABLE));
        }

        #[cfg(not(feature = "pbr_multi_layer_material_textures"))]
        if clearcoat_texture.is_some()
            || clearcoat_channel != PyUvChannel::Uv0
            || clearcoat_roughness_texture.is_some()
            || clearcoat_roughness_channel != PyUvChannel::Uv0
            || clearcoat_normal_texture.is_some()
            || clearcoat_normal_channel != PyUvChannel::Uv0
        {
            return Err(PyRuntimeError::new_err(
                MULTI_LAYER_MATERIAL_TEXTURES_UNAVAILABLE,
            ));
        }

        let base_color = base_color.try_into()?;
        let specular_tint = specular_tint.try_into()?;
        let attenuation_color = attenuation_color.try_into()?;
        let material = StandardMaterial {
            base_color,
            base_color_texture: convert_optional_handle(base_color_texture)?,
            base_color_channel: base_color_channel.into(),
            emissive: emissive_linear,
            emissive_exposure_weight,
            emissive_channel: emissive_channel.into(),
            emissive_texture: convert_optional_handle(emissive_texture)?,
            perceptual_roughness,
            metallic,
            metallic_roughness_channel: metallic_roughness_channel.into(),
            metallic_roughness_texture: convert_optional_handle(metallic_roughness_texture)?,
            reflectance,
            specular_tint,
            diffuse_transmission,
            #[cfg(feature = "pbr_transmission_textures")]
            diffuse_transmission_channel: diffuse_transmission_channel.into(),
            #[cfg(feature = "pbr_transmission_textures")]
            diffuse_transmission_texture: convert_optional_handle(diffuse_transmission_texture)?,
            specular_transmission,
            #[cfg(feature = "pbr_transmission_textures")]
            specular_transmission_channel: specular_transmission_channel.into(),
            #[cfg(feature = "pbr_transmission_textures")]
            specular_transmission_texture: convert_optional_handle(specular_transmission_texture)?,
            thickness,
            #[cfg(feature = "pbr_transmission_textures")]
            thickness_channel: thickness_channel.into(),
            #[cfg(feature = "pbr_transmission_textures")]
            thickness_texture: convert_optional_handle(thickness_texture)?,
            ior,
            attenuation_distance,
            attenuation_color,
            normal_map_channel: normal_map_channel.into(),
            normal_map_texture: convert_optional_handle(normal_map_texture)?,
            flip_normal_map_y,
            occlusion_channel: occlusion_channel.into(),
            occlusion_texture: convert_optional_handle(occlusion_texture)?,
            #[cfg(feature = "pbr_specular_textures")]
            specular_channel: specular_channel.into(),
            #[cfg(feature = "pbr_specular_textures")]
            specular_texture: convert_optional_handle(specular_texture)?,
            #[cfg(feature = "pbr_specular_textures")]
            specular_tint_channel: specular_tint_channel.into(),
            #[cfg(feature = "pbr_specular_textures")]
            specular_tint_texture: convert_optional_handle(specular_tint_texture)?,
            anisotropy_strength,
            anisotropy_rotation,
            #[cfg(feature = "pbr_anisotropy_texture")]
            anisotropy_channel: anisotropy_channel.into(),
            #[cfg(feature = "pbr_anisotropy_texture")]
            anisotropy_texture: convert_optional_handle(anisotropy_texture)?,
            double_sided,
            cull_mode: cull_mode.map(Into::into),
            unlit,
            fog_enabled,
            alpha_mode: alpha_mode.into(),
            depth_bias,
            depth_map: convert_optional_handle(depth_map)?,
            parallax_depth_scale,
            parallax_mapping_method: parallax_mapping_method.into(),
            max_parallax_layer_count,
            lightmap_exposure,
            opaque_render_method: opaque_render_method.into(),
            deferred_lighting_pass_id,
            uv_transform: uv_transform.try_into()?,
            clearcoat,
            clearcoat_perceptual_roughness,
            #[cfg(feature = "pbr_multi_layer_material_textures")]
            clearcoat_channel: clearcoat_channel.into(),
            #[cfg(feature = "pbr_multi_layer_material_textures")]
            clearcoat_texture: convert_optional_handle(clearcoat_texture)?,
            #[cfg(feature = "pbr_multi_layer_material_textures")]
            clearcoat_roughness_channel: clearcoat_roughness_channel.into(),
            #[cfg(feature = "pbr_multi_layer_material_textures")]
            clearcoat_roughness_texture: convert_optional_handle(clearcoat_roughness_texture)?,
            #[cfg(feature = "pbr_multi_layer_material_textures")]
            clearcoat_normal_channel: clearcoat_normal_channel.into(),
            #[cfg(feature = "pbr_multi_layer_material_textures")]
            clearcoat_normal_texture: convert_optional_handle(clearcoat_normal_texture)?,
        };

        Ok(Self::from_owned(material))
    }

    #[staticmethod]
    pub fn from_color(py: Python, color: PyColor) -> PyResult<Py<Self>> {
        let color = Color::try_from(color)?;
        let material = StandardMaterial::from_color(color);
        Py::new(py, PyStandardMaterial::from_owned(material))
    }

    #[getter]
    pub fn base_color(&self, py: Python) -> PyResult<Py<PyColor>> {
        let storage: ValueStorage<Color> = self.storage.borrow_field(
            |material| &material.base_color,
            |material| &mut material.base_color,
        )?;
        PyColor::from_storage(storage, py)
    }

    #[setter]
    pub fn set_base_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = color.try_into()?;
        self.as_mut()?.base_color = color;
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
    pub fn set_base_color_texture(&mut self, texture: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.as_mut()?.base_color_texture = convert_optional_handle(texture)?;
        Ok(())
    }

    #[getter]
    pub fn base_color_channel(&self) -> PyResult<PyUvChannel> {
        Ok(self.as_ref()?.base_color_channel.clone().into())
    }

    #[setter]
    pub fn set_base_color_channel(&mut self, channel: PyUvChannel) -> PyResult<()> {
        self.as_mut()?.base_color_channel = channel.into();
        Ok(())
    }

    #[getter]
    pub fn emissive(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_snapshot(Color::from(self.as_ref()?.emissive), py)
    }

    #[setter]
    pub fn set_emissive(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.emissive = extract_linear_rgba(value)?;
        Ok(())
    }

    #[getter]
    pub fn emissive_exposure_weight(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.emissive_exposure_weight)
    }

    #[setter]
    pub fn set_emissive_exposure_weight(&mut self, weight: f32) -> PyResult<()> {
        self.as_mut()?.emissive_exposure_weight = weight;
        Ok(())
    }

    #[getter]
    pub fn emissive_channel(&self) -> PyResult<PyUvChannel> {
        Ok(self.as_ref()?.emissive_channel.clone().into())
    }

    #[setter]
    pub fn set_emissive_channel(&mut self, channel: PyUvChannel) -> PyResult<()> {
        self.as_mut()?.emissive_channel = channel.into();
        Ok(())
    }

    #[getter]
    pub fn emissive_texture(&self) -> PyResult<Option<PyHandle>> {
        Ok(self.as_ref()?.emissive_texture.as_ref().map(PyHandle::from))
    }

    #[setter]
    pub fn set_emissive_texture(&mut self, texture: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.as_mut()?.emissive_texture = convert_optional_handle(texture)?;
        Ok(())
    }

    #[getter]
    pub fn perceptual_roughness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.perceptual_roughness)
    }

    #[setter]
    pub fn set_perceptual_roughness(&mut self, roughness: f32) -> PyResult<()> {
        self.as_mut()?.perceptual_roughness = roughness;
        Ok(())
    }

    #[getter]
    pub fn metallic(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.metallic)
    }

    #[setter]
    pub fn set_metallic(&mut self, metallic: f32) -> PyResult<()> {
        self.as_mut()?.metallic = metallic;
        Ok(())
    }

    #[getter]
    pub fn metallic_roughness_channel(&self) -> PyResult<PyUvChannel> {
        Ok(self.as_ref()?.metallic_roughness_channel.clone().into())
    }

    #[setter]
    pub fn set_metallic_roughness_channel(&mut self, channel: PyUvChannel) -> PyResult<()> {
        self.as_mut()?.metallic_roughness_channel = channel.into();
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
    pub fn set_metallic_roughness_texture(
        &mut self,
        texture: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.as_mut()?.metallic_roughness_texture = convert_optional_handle(texture)?;
        Ok(())
    }

    #[getter]
    pub fn reflectance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.reflectance)
    }

    #[setter]
    pub fn set_reflectance(&mut self, reflectance: f32) -> PyResult<()> {
        self.as_mut()?.reflectance = reflectance;
        Ok(())
    }

    #[getter]
    pub fn specular_tint(&self, py: Python) -> PyResult<Py<PyColor>> {
        let storage: ValueStorage<Color> = self.storage.borrow_field(
            |material| &material.specular_tint,
            |material| &mut material.specular_tint,
        )?;
        PyColor::from_storage(storage, py)
    }

    #[setter]
    pub fn set_specular_tint(&mut self, color: PyColor) -> PyResult<()> {
        let color = color.try_into()?;
        self.as_mut()?.specular_tint = color;
        Ok(())
    }

    #[getter]
    pub fn diffuse_transmission(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.diffuse_transmission)
    }

    #[setter]
    pub fn set_diffuse_transmission(&mut self, transmission: f32) -> PyResult<()> {
        self.as_mut()?.diffuse_transmission = transmission;
        Ok(())
    }

    #[getter]
    pub fn specular_transmission(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.specular_transmission)
    }

    #[setter]
    pub fn set_specular_transmission(&mut self, transmission: f32) -> PyResult<()> {
        self.as_mut()?.specular_transmission = transmission;
        Ok(())
    }

    #[getter]
    pub fn thickness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.thickness)
    }

    #[setter]
    pub fn set_thickness(&mut self, thickness: f32) -> PyResult<()> {
        self.as_mut()?.thickness = thickness;
        Ok(())
    }

    #[getter]
    pub fn ior(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.ior)
    }

    #[setter]
    pub fn set_ior(&mut self, ior: f32) -> PyResult<()> {
        self.as_mut()?.ior = ior;
        Ok(())
    }

    #[getter]
    pub fn attenuation_distance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.attenuation_distance)
    }

    #[setter]
    pub fn set_attenuation_distance(&mut self, distance: f32) -> PyResult<()> {
        self.as_mut()?.attenuation_distance = distance;
        Ok(())
    }

    #[getter]
    pub fn attenuation_color(&self, py: Python) -> PyResult<Py<PyColor>> {
        let storage: ValueStorage<Color> = self.storage.borrow_field(
            |material| &material.attenuation_color,
            |material| &mut material.attenuation_color,
        )?;
        PyColor::from_storage(storage, py)
    }

    #[setter]
    pub fn set_attenuation_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = color.try_into()?;
        self.as_mut()?.attenuation_color = color;
        Ok(())
    }

    #[getter]
    pub fn normal_map_channel(&self) -> PyResult<PyUvChannel> {
        Ok(self.as_ref()?.normal_map_channel.clone().into())
    }

    #[setter]
    pub fn set_normal_map_channel(&mut self, channel: PyUvChannel) -> PyResult<()> {
        self.as_mut()?.normal_map_channel = channel.into();
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
    pub fn set_normal_map_texture(&mut self, texture: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.as_mut()?.normal_map_texture = convert_optional_handle(texture)?;
        Ok(())
    }

    #[getter]
    pub fn flip_normal_map_y(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.flip_normal_map_y)
    }

    #[setter]
    pub fn set_flip_normal_map_y(&mut self, flip: bool) -> PyResult<()> {
        self.as_mut()?.flip_normal_map_y = flip;
        Ok(())
    }

    #[getter]
    pub fn occlusion_channel(&self) -> PyResult<PyUvChannel> {
        Ok(self.as_ref()?.occlusion_channel.clone().into())
    }

    #[setter]
    pub fn set_occlusion_channel(&mut self, channel: PyUvChannel) -> PyResult<()> {
        self.as_mut()?.occlusion_channel = channel.into();
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
    pub fn set_occlusion_texture(&mut self, texture: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.as_mut()?.occlusion_texture = convert_optional_handle(texture)?;
        Ok(())
    }

    #[getter]
    pub fn anisotropy_strength(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.anisotropy_strength)
    }

    #[setter]
    pub fn set_anisotropy_strength(&mut self, strength: f32) -> PyResult<()> {
        self.as_mut()?.anisotropy_strength = strength;
        Ok(())
    }

    #[getter]
    pub fn anisotropy_rotation(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.anisotropy_rotation)
    }

    #[setter]
    pub fn set_anisotropy_rotation(&mut self, rotation: f32) -> PyResult<()> {
        self.as_mut()?.anisotropy_rotation = rotation;
        Ok(())
    }

    #[getter]
    pub fn anisotropy_channel(&self) -> PyResult<PyUvChannel> {
        #[cfg(feature = "pbr_anisotropy_texture")]
        {
            Ok(self.as_ref()?.anisotropy_channel.clone().into())
        }
        #[cfg(not(feature = "pbr_anisotropy_texture"))]
        {
            self.as_ref()?;
            Ok(PyUvChannel::Uv0)
        }
    }

    #[setter]
    pub fn set_anisotropy_channel(&mut self, channel: PyUvChannel) -> PyResult<()> {
        #[cfg(feature = "pbr_anisotropy_texture")]
        {
            self.as_mut()?.anisotropy_channel = channel.into();
            Ok(())
        }
        #[cfg(not(feature = "pbr_anisotropy_texture"))]
        {
            self.as_mut()?;
            if channel != PyUvChannel::Uv0 {
                return Err(PyRuntimeError::new_err(ANISOTROPY_TEXTURE_UNAVAILABLE));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn anisotropy_texture(&self) -> PyResult<Option<PyHandle>> {
        #[cfg(feature = "pbr_anisotropy_texture")]
        {
            Ok(self
                .as_ref()?
                .anisotropy_texture
                .as_ref()
                .map(PyHandle::from))
        }
        #[cfg(not(feature = "pbr_anisotropy_texture"))]
        {
            self.as_ref()?;
            Ok(None)
        }
    }

    #[setter]
    pub fn set_anisotropy_texture(&mut self, texture: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        #[cfg(feature = "pbr_anisotropy_texture")]
        {
            self.as_mut()?.anisotropy_texture = convert_optional_handle(texture)?;
            Ok(())
        }
        #[cfg(not(feature = "pbr_anisotropy_texture"))]
        {
            self.as_mut()?;
            if texture.is_some() {
                return Err(PyRuntimeError::new_err(ANISOTROPY_TEXTURE_UNAVAILABLE));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn diffuse_transmission_channel(&self) -> PyResult<PyUvChannel> {
        #[cfg(feature = "pbr_transmission_textures")]
        {
            Ok(self.as_ref()?.diffuse_transmission_channel.clone().into())
        }
        #[cfg(not(feature = "pbr_transmission_textures"))]
        {
            self.as_ref()?;
            Ok(PyUvChannel::Uv0)
        }
    }

    #[setter]
    pub fn set_diffuse_transmission_channel(&mut self, channel: PyUvChannel) -> PyResult<()> {
        #[cfg(feature = "pbr_transmission_textures")]
        {
            self.as_mut()?.diffuse_transmission_channel = channel.into();
            Ok(())
        }
        #[cfg(not(feature = "pbr_transmission_textures"))]
        {
            self.as_mut()?;
            if channel != PyUvChannel::Uv0 {
                return Err(PyRuntimeError::new_err(TRANSMISSION_TEXTURES_UNAVAILABLE));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn diffuse_transmission_texture(&self) -> PyResult<Option<PyHandle>> {
        #[cfg(feature = "pbr_transmission_textures")]
        {
            Ok(self
                .as_ref()?
                .diffuse_transmission_texture
                .as_ref()
                .map(PyHandle::from))
        }
        #[cfg(not(feature = "pbr_transmission_textures"))]
        {
            self.as_ref()?;
            Ok(None)
        }
    }

    #[setter]
    pub fn set_diffuse_transmission_texture(
        &mut self,
        texture: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        #[cfg(feature = "pbr_transmission_textures")]
        {
            self.as_mut()?.diffuse_transmission_texture = convert_optional_handle(texture)?;
            Ok(())
        }
        #[cfg(not(feature = "pbr_transmission_textures"))]
        {
            self.as_mut()?;
            if texture.is_some() {
                return Err(PyRuntimeError::new_err(TRANSMISSION_TEXTURES_UNAVAILABLE));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn specular_transmission_channel(&self) -> PyResult<PyUvChannel> {
        #[cfg(feature = "pbr_transmission_textures")]
        {
            Ok(self.as_ref()?.specular_transmission_channel.clone().into())
        }
        #[cfg(not(feature = "pbr_transmission_textures"))]
        {
            self.as_ref()?;
            Ok(PyUvChannel::Uv0)
        }
    }

    #[setter]
    pub fn set_specular_transmission_channel(&mut self, channel: PyUvChannel) -> PyResult<()> {
        #[cfg(feature = "pbr_transmission_textures")]
        {
            self.as_mut()?.specular_transmission_channel = channel.into();
            Ok(())
        }
        #[cfg(not(feature = "pbr_transmission_textures"))]
        {
            self.as_mut()?;
            if channel != PyUvChannel::Uv0 {
                return Err(PyRuntimeError::new_err(TRANSMISSION_TEXTURES_UNAVAILABLE));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn specular_transmission_texture(&self) -> PyResult<Option<PyHandle>> {
        #[cfg(feature = "pbr_transmission_textures")]
        {
            Ok(self
                .as_ref()?
                .specular_transmission_texture
                .as_ref()
                .map(PyHandle::from))
        }
        #[cfg(not(feature = "pbr_transmission_textures"))]
        {
            self.as_ref()?;
            Ok(None)
        }
    }

    #[setter]
    pub fn set_specular_transmission_texture(
        &mut self,
        texture: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        #[cfg(feature = "pbr_transmission_textures")]
        {
            self.as_mut()?.specular_transmission_texture = convert_optional_handle(texture)?;
            Ok(())
        }
        #[cfg(not(feature = "pbr_transmission_textures"))]
        {
            self.as_mut()?;
            if texture.is_some() {
                return Err(PyRuntimeError::new_err(TRANSMISSION_TEXTURES_UNAVAILABLE));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn thickness_channel(&self) -> PyResult<PyUvChannel> {
        #[cfg(feature = "pbr_transmission_textures")]
        {
            Ok(self.as_ref()?.thickness_channel.clone().into())
        }
        #[cfg(not(feature = "pbr_transmission_textures"))]
        {
            self.as_ref()?;
            Ok(PyUvChannel::Uv0)
        }
    }

    #[setter]
    pub fn set_thickness_channel(&mut self, channel: PyUvChannel) -> PyResult<()> {
        #[cfg(feature = "pbr_transmission_textures")]
        {
            self.as_mut()?.thickness_channel = channel.into();
            Ok(())
        }
        #[cfg(not(feature = "pbr_transmission_textures"))]
        {
            self.as_mut()?;
            if channel != PyUvChannel::Uv0 {
                return Err(PyRuntimeError::new_err(TRANSMISSION_TEXTURES_UNAVAILABLE));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn thickness_texture(&self) -> PyResult<Option<PyHandle>> {
        #[cfg(feature = "pbr_transmission_textures")]
        {
            Ok(self
                .as_ref()?
                .thickness_texture
                .as_ref()
                .map(PyHandle::from))
        }
        #[cfg(not(feature = "pbr_transmission_textures"))]
        {
            self.as_ref()?;
            Ok(None)
        }
    }

    #[setter]
    pub fn set_thickness_texture(&mut self, texture: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        #[cfg(feature = "pbr_transmission_textures")]
        {
            self.as_mut()?.thickness_texture = convert_optional_handle(texture)?;
            Ok(())
        }
        #[cfg(not(feature = "pbr_transmission_textures"))]
        {
            self.as_mut()?;
            if texture.is_some() {
                return Err(PyRuntimeError::new_err(TRANSMISSION_TEXTURES_UNAVAILABLE));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn specular_channel(&self) -> PyResult<PyUvChannel> {
        #[cfg(feature = "pbr_specular_textures")]
        {
            Ok(self.as_ref()?.specular_channel.clone().into())
        }
        #[cfg(not(feature = "pbr_specular_textures"))]
        {
            self.as_ref()?;
            Ok(PyUvChannel::Uv0)
        }
    }

    #[setter]
    pub fn set_specular_channel(&mut self, channel: PyUvChannel) -> PyResult<()> {
        #[cfg(feature = "pbr_specular_textures")]
        {
            self.as_mut()?.specular_channel = channel.into();
            Ok(())
        }
        #[cfg(not(feature = "pbr_specular_textures"))]
        {
            self.as_mut()?;
            if channel != PyUvChannel::Uv0 {
                return Err(PyRuntimeError::new_err(SPECULAR_TEXTURES_UNAVAILABLE));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn specular_texture(&self) -> PyResult<Option<PyHandle>> {
        #[cfg(feature = "pbr_specular_textures")]
        {
            Ok(self.as_ref()?.specular_texture.as_ref().map(PyHandle::from))
        }
        #[cfg(not(feature = "pbr_specular_textures"))]
        {
            self.as_ref()?;
            Ok(None)
        }
    }

    #[setter]
    pub fn set_specular_texture(&mut self, texture: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        #[cfg(feature = "pbr_specular_textures")]
        {
            self.as_mut()?.specular_texture = convert_optional_handle(texture)?;
            Ok(())
        }
        #[cfg(not(feature = "pbr_specular_textures"))]
        {
            self.as_mut()?;
            if texture.is_some() {
                return Err(PyRuntimeError::new_err(SPECULAR_TEXTURES_UNAVAILABLE));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn specular_tint_channel(&self) -> PyResult<PyUvChannel> {
        #[cfg(feature = "pbr_specular_textures")]
        {
            Ok(self.as_ref()?.specular_tint_channel.clone().into())
        }
        #[cfg(not(feature = "pbr_specular_textures"))]
        {
            self.as_ref()?;
            Ok(PyUvChannel::Uv0)
        }
    }

    #[setter]
    pub fn set_specular_tint_channel(&mut self, channel: PyUvChannel) -> PyResult<()> {
        #[cfg(feature = "pbr_specular_textures")]
        {
            self.as_mut()?.specular_tint_channel = channel.into();
            Ok(())
        }
        #[cfg(not(feature = "pbr_specular_textures"))]
        {
            self.as_mut()?;
            if channel != PyUvChannel::Uv0 {
                return Err(PyRuntimeError::new_err(SPECULAR_TEXTURES_UNAVAILABLE));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn specular_tint_texture(&self) -> PyResult<Option<PyHandle>> {
        #[cfg(feature = "pbr_specular_textures")]
        {
            Ok(self
                .as_ref()?
                .specular_tint_texture
                .as_ref()
                .map(PyHandle::from))
        }
        #[cfg(not(feature = "pbr_specular_textures"))]
        {
            self.as_ref()?;
            Ok(None)
        }
    }

    #[setter]
    pub fn set_specular_tint_texture(
        &mut self,
        texture: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        #[cfg(feature = "pbr_specular_textures")]
        {
            self.as_mut()?.specular_tint_texture = convert_optional_handle(texture)?;
            Ok(())
        }
        #[cfg(not(feature = "pbr_specular_textures"))]
        {
            self.as_mut()?;
            if texture.is_some() {
                return Err(PyRuntimeError::new_err(SPECULAR_TEXTURES_UNAVAILABLE));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn clearcoat_channel(&self) -> PyResult<PyUvChannel> {
        #[cfg(feature = "pbr_multi_layer_material_textures")]
        {
            Ok(self.as_ref()?.clearcoat_channel.clone().into())
        }
        #[cfg(not(feature = "pbr_multi_layer_material_textures"))]
        {
            self.as_ref()?;
            Ok(PyUvChannel::Uv0)
        }
    }

    #[setter]
    pub fn set_clearcoat_channel(&mut self, channel: PyUvChannel) -> PyResult<()> {
        #[cfg(feature = "pbr_multi_layer_material_textures")]
        {
            self.as_mut()?.clearcoat_channel = channel.into();
            Ok(())
        }
        #[cfg(not(feature = "pbr_multi_layer_material_textures"))]
        {
            self.as_mut()?;
            if channel != PyUvChannel::Uv0 {
                return Err(PyRuntimeError::new_err(
                    MULTI_LAYER_MATERIAL_TEXTURES_UNAVAILABLE,
                ));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn clearcoat_texture(&self) -> PyResult<Option<PyHandle>> {
        #[cfg(feature = "pbr_multi_layer_material_textures")]
        {
            Ok(self
                .as_ref()?
                .clearcoat_texture
                .as_ref()
                .map(PyHandle::from))
        }
        #[cfg(not(feature = "pbr_multi_layer_material_textures"))]
        {
            self.as_ref()?;
            Ok(None)
        }
    }

    #[setter]
    pub fn set_clearcoat_texture(&mut self, texture: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        #[cfg(feature = "pbr_multi_layer_material_textures")]
        {
            self.as_mut()?.clearcoat_texture = convert_optional_handle(texture)?;
            Ok(())
        }
        #[cfg(not(feature = "pbr_multi_layer_material_textures"))]
        {
            self.as_mut()?;
            if texture.is_some() {
                return Err(PyRuntimeError::new_err(
                    MULTI_LAYER_MATERIAL_TEXTURES_UNAVAILABLE,
                ));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn clearcoat_roughness_channel(&self) -> PyResult<PyUvChannel> {
        #[cfg(feature = "pbr_multi_layer_material_textures")]
        {
            Ok(self.as_ref()?.clearcoat_roughness_channel.clone().into())
        }
        #[cfg(not(feature = "pbr_multi_layer_material_textures"))]
        {
            self.as_ref()?;
            Ok(PyUvChannel::Uv0)
        }
    }

    #[setter]
    pub fn set_clearcoat_roughness_channel(&mut self, channel: PyUvChannel) -> PyResult<()> {
        #[cfg(feature = "pbr_multi_layer_material_textures")]
        {
            self.as_mut()?.clearcoat_roughness_channel = channel.into();
            Ok(())
        }
        #[cfg(not(feature = "pbr_multi_layer_material_textures"))]
        {
            self.as_mut()?;
            if channel != PyUvChannel::Uv0 {
                return Err(PyRuntimeError::new_err(
                    MULTI_LAYER_MATERIAL_TEXTURES_UNAVAILABLE,
                ));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn clearcoat_roughness_texture(&self) -> PyResult<Option<PyHandle>> {
        #[cfg(feature = "pbr_multi_layer_material_textures")]
        {
            Ok(self
                .as_ref()?
                .clearcoat_roughness_texture
                .as_ref()
                .map(PyHandle::from))
        }
        #[cfg(not(feature = "pbr_multi_layer_material_textures"))]
        {
            self.as_ref()?;
            Ok(None)
        }
    }

    #[setter]
    pub fn set_clearcoat_roughness_texture(
        &mut self,
        texture: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        #[cfg(feature = "pbr_multi_layer_material_textures")]
        {
            self.as_mut()?.clearcoat_roughness_texture = convert_optional_handle(texture)?;
            Ok(())
        }
        #[cfg(not(feature = "pbr_multi_layer_material_textures"))]
        {
            self.as_mut()?;
            if texture.is_some() {
                return Err(PyRuntimeError::new_err(
                    MULTI_LAYER_MATERIAL_TEXTURES_UNAVAILABLE,
                ));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn clearcoat_normal_channel(&self) -> PyResult<PyUvChannel> {
        #[cfg(feature = "pbr_multi_layer_material_textures")]
        {
            Ok(self.as_ref()?.clearcoat_normal_channel.clone().into())
        }
        #[cfg(not(feature = "pbr_multi_layer_material_textures"))]
        {
            self.as_ref()?;
            Ok(PyUvChannel::Uv0)
        }
    }

    #[setter]
    pub fn set_clearcoat_normal_channel(&mut self, channel: PyUvChannel) -> PyResult<()> {
        #[cfg(feature = "pbr_multi_layer_material_textures")]
        {
            self.as_mut()?.clearcoat_normal_channel = channel.into();
            Ok(())
        }
        #[cfg(not(feature = "pbr_multi_layer_material_textures"))]
        {
            self.as_mut()?;
            if channel != PyUvChannel::Uv0 {
                return Err(PyRuntimeError::new_err(
                    MULTI_LAYER_MATERIAL_TEXTURES_UNAVAILABLE,
                ));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn clearcoat_normal_texture(&self) -> PyResult<Option<PyHandle>> {
        #[cfg(feature = "pbr_multi_layer_material_textures")]
        {
            Ok(self
                .as_ref()?
                .clearcoat_normal_texture
                .as_ref()
                .map(PyHandle::from))
        }
        #[cfg(not(feature = "pbr_multi_layer_material_textures"))]
        {
            self.as_ref()?;
            Ok(None)
        }
    }

    #[setter]
    pub fn set_clearcoat_normal_texture(
        &mut self,
        texture: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        #[cfg(feature = "pbr_multi_layer_material_textures")]
        {
            self.as_mut()?.clearcoat_normal_texture = convert_optional_handle(texture)?;
            Ok(())
        }
        #[cfg(not(feature = "pbr_multi_layer_material_textures"))]
        {
            self.as_mut()?;
            if texture.is_some() {
                return Err(PyRuntimeError::new_err(
                    MULTI_LAYER_MATERIAL_TEXTURES_UNAVAILABLE,
                ));
            }
            Ok(())
        }
    }

    #[getter]
    pub fn double_sided(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.double_sided)
    }

    #[setter]
    pub fn set_double_sided(&mut self, double_sided: bool) -> PyResult<()> {
        self.as_mut()?.double_sided = double_sided;
        Ok(())
    }

    #[getter]
    pub fn cull_mode(&self) -> PyResult<Option<PyFace>> {
        Ok(self.as_ref()?.cull_mode.map(Into::into))
    }

    #[setter]
    pub fn set_cull_mode(&mut self, cull_mode: Option<PyFace>) -> PyResult<()> {
        self.as_mut()?.cull_mode = cull_mode.map(|v| v.into());
        Ok(())
    }

    #[getter]
    pub fn unlit(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.unlit)
    }

    #[setter]
    pub fn set_unlit(&mut self, unlit: bool) -> PyResult<()> {
        self.as_mut()?.unlit = unlit;
        Ok(())
    }

    #[getter]
    pub fn fog_enabled(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.fog_enabled)
    }

    #[setter]
    pub fn set_fog_enabled(&mut self, enabled: bool) -> PyResult<()> {
        self.as_mut()?.fog_enabled = enabled;
        Ok(())
    }

    #[getter]
    pub fn alpha_mode(&self) -> PyResult<PyAlphaMode> {
        Ok(self.as_ref()?.alpha_mode.into())
    }

    #[setter]
    pub fn set_alpha_mode(&mut self, mode: PyAlphaMode) -> PyResult<()> {
        self.as_mut()?.alpha_mode = mode.into();
        Ok(())
    }

    #[getter]
    pub fn depth_bias(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.depth_bias)
    }

    #[setter]
    pub fn set_depth_bias(&mut self, bias: f32) -> PyResult<()> {
        self.as_mut()?.depth_bias = bias;
        Ok(())
    }

    #[getter]
    pub fn depth_map(&self) -> PyResult<Option<PyHandle>> {
        Ok(self.as_ref()?.depth_map.as_ref().map(PyHandle::from))
    }

    #[setter]
    pub fn set_depth_map(&mut self, texture: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.as_mut()?.depth_map = convert_optional_handle(texture)?;
        Ok(())
    }

    #[getter]
    pub fn parallax_depth_scale(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.parallax_depth_scale)
    }

    #[setter]
    pub fn set_parallax_depth_scale(&mut self, scale: f32) -> PyResult<()> {
        self.as_mut()?.parallax_depth_scale = scale;
        Ok(())
    }

    #[getter]
    pub fn parallax_mapping_method(&self) -> PyResult<PyParallaxMappingMethod> {
        Ok(self.as_ref()?.parallax_mapping_method.into())
    }

    #[setter]
    pub fn set_parallax_mapping_method(&mut self, method: PyParallaxMappingMethod) -> PyResult<()> {
        self.as_mut()?.parallax_mapping_method = method.into();
        Ok(())
    }

    #[getter]
    pub fn max_parallax_layer_count(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.max_parallax_layer_count)
    }

    #[setter]
    pub fn set_max_parallax_layer_count(&mut self, count: f32) -> PyResult<()> {
        self.as_mut()?.max_parallax_layer_count = count;
        Ok(())
    }

    #[getter]
    pub fn lightmap_exposure(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.lightmap_exposure)
    }

    #[setter]
    pub fn set_lightmap_exposure(&mut self, exposure: f32) -> PyResult<()> {
        self.as_mut()?.lightmap_exposure = exposure;
        Ok(())
    }

    #[getter]
    pub fn opaque_render_method(&self) -> PyResult<PyOpaqueRendererMethod> {
        Ok(self.as_ref()?.opaque_render_method.into())
    }

    #[setter]
    pub fn set_opaque_render_method(&mut self, method: PyOpaqueRendererMethod) -> PyResult<()> {
        self.as_mut()?.opaque_render_method = method.into();
        Ok(())
    }

    #[getter]
    pub fn deferred_lighting_pass_id(&self) -> PyResult<u8> {
        Ok(self.as_ref()?.deferred_lighting_pass_id)
    }

    #[setter]
    pub fn set_deferred_lighting_pass_id(&mut self, id: u8) -> PyResult<()> {
        self.as_mut()?.deferred_lighting_pass_id = id;
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
    pub fn set_uv_transform(&mut self, transform: PyAffine2) -> PyResult<()> {
        let transform = transform.try_into()?;
        self.as_mut()?.uv_transform = transform;
        Ok(())
    }

    #[getter]
    pub fn clearcoat(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.clearcoat)
    }

    #[setter]
    pub fn set_clearcoat(&mut self, clearcoat: f32) -> PyResult<()> {
        self.as_mut()?.clearcoat = clearcoat;
        Ok(())
    }

    #[getter]
    pub fn clearcoat_perceptual_roughness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.clearcoat_perceptual_roughness)
    }

    #[setter]
    pub fn set_clearcoat_perceptual_roughness(&mut self, roughness: f32) -> PyResult<()> {
        self.as_mut()?.clearcoat_perceptual_roughness = roughness;
        Ok(())
    }

    pub fn flip(&mut self, horizontal: bool, vertical: bool) -> PyResult<()> {
        self.as_mut()?.flip(horizontal, vertical);
        Ok(())
    }

    #[pyo3(name = "flipped")]
    pub fn flipped(&self, py: Python<'_>, horizontal: bool, vertical: bool) -> PyResult<Py<Self>> {
        let mut material = self.as_ref()?.clone();
        material.flip(horizontal, vertical);
        Py::new(py, Self::from_owned(material))
    }
}
