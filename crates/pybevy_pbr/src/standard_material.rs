use bevy::{
    color::{Color, LinearRgba},
    pbr::StandardMaterial,
};
use pybevy_color::{color::PyColor, linear_rgba::PyLinearRgba};
use pybevy_core::{
    AssetInputConverter, AssetStorage, PyAsset, PyHandle, PyMaterializable, extract_handle_from_any,
};
use pybevy_macros::pyasset;
use pybevy_math::affine2::PyAffine2;
use pybevy_render::{
    alpha_mode::PyAlphaMode, face::PyFace, opaque_render_method::PyOpaqueRenderMethod,
};
use pyo3::{exceptions::PyTypeError, prelude::*};

use crate::{parallax_mapping_method::PyParallaxMappingMethod, uv_channel::PyUvChannel};

fn extract_linear_rgba(value: &Bound<'_, PyAny>) -> PyResult<LinearRgba> {
    if let Ok(color) = value.extract::<PyColor>() {
        Ok(LinearRgba::from(color.0))
    } else if let Ok(linear) = value.extract::<PyLinearRgba>() {
        Ok(LinearRgba::from(&linear))
    } else {
        Err(PyTypeError::new_err("expected Color or LinearRgba"))
    }
}

fn convert_optional_handle(
    handle: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<bevy::asset::Handle<bevy::image::Image>>> {
    match handle {
        Some(h) => {
            let extracted = extract_handle_from_any(h)?;
            Ok(Some((&extracted).try_into()?))
        }
        None => Ok(None),
    }
}

#[pyasset(StandardMaterial, bridge, input_converter)]
#[pyclass(name = "StandardMaterial", extends = PyAsset)]
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
        specular_transmission = 0.0,
        thickness = 0.0,
        ior = 1.5,
        attenuation_distance = f32::INFINITY,
        attenuation_color = Color::WHITE.into(),
        normal_map_channel = PyUvChannel::Uv0,
        normal_map_texture = None,
        flip_normal_map_y = false,
        occlusion_channel = PyUvChannel::Uv0,
        occlusion_texture = None,
        anisotropy_strength = 0.0,
        anisotropy_rotation = 0.0,
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
        opaque_render_method = PyOpaqueRenderMethod::Auto,
        deferred_lighting_pass_id = 1,
        uv_transform = PyAffine2::IDENTITY,
        clearcoat = 0.0,
        clearcoat_perceptual_roughness = 0.5
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
        specular_transmission: f32,
        thickness: f32,
        ior: f32,
        attenuation_distance: f32,
        attenuation_color: PyColor,
        normal_map_channel: PyUvChannel,
        normal_map_texture: Option<&Bound<'_, PyAny>>,
        flip_normal_map_y: bool,
        occlusion_channel: PyUvChannel,
        occlusion_texture: Option<&Bound<'_, PyAny>>,
        anisotropy_strength: f32,
        anisotropy_rotation: f32,
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
        opaque_render_method: PyOpaqueRenderMethod,
        deferred_lighting_pass_id: u8,
        uv_transform: PyAffine2,
        clearcoat: f32,
        clearcoat_perceptual_roughness: f32,
    ) -> PyResult<(Self, PyAsset)> {
        let emissive_linear = match emissive {
            Some(ref emissive_val) => extract_linear_rgba(emissive_val)?,
            None => LinearRgba::from(Color::BLACK),
        };

        let material = StandardMaterial {
            base_color: base_color.0,
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
            specular_tint: specular_tint.0,
            diffuse_transmission,
            specular_transmission,
            thickness,
            ior,
            attenuation_distance,
            attenuation_color: attenuation_color.0,
            normal_map_channel: normal_map_channel.into(),
            normal_map_texture: convert_optional_handle(normal_map_texture)?,
            flip_normal_map_y,
            occlusion_channel: occlusion_channel.into(),
            occlusion_texture: convert_optional_handle(occlusion_texture)?,
            anisotropy_strength,
            anisotropy_rotation,
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
            uv_transform: uv_transform.into(),
            clearcoat,
            clearcoat_perceptual_roughness,
            ..Default::default()
        };

        Ok(Self::from_owned(material))
    }

    #[staticmethod]
    pub fn from_color(py: Python, color: PyColor) -> PyResult<Py<Self>> {
        let material = StandardMaterial::from_color(color.0);
        Py::new(py, (PyStandardMaterial::from(material), PyAsset))
    }

    #[getter]
    pub fn base_color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.base_color, py)
    }

    #[setter]
    pub fn set_base_color(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.base_color = color.0;
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
        PyColor::from_color(Color::from(self.as_ref()?.emissive), py)
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
        PyColor::from_color(self.as_ref()?.specular_tint, py)
    }

    #[setter]
    pub fn set_specular_tint(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.specular_tint = color.0;
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
        PyColor::from_color(self.as_ref()?.attenuation_color, py)
    }

    #[setter]
    pub fn set_attenuation_color(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.attenuation_color = color.0;
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
    pub fn opaque_render_method(&self) -> PyResult<PyOpaqueRenderMethod> {
        Ok(self.as_ref()?.opaque_render_method.into())
    }

    #[setter]
    pub fn set_opaque_render_method(&mut self, method: PyOpaqueRenderMethod) -> PyResult<()> {
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
        Ok(self.as_ref()?.uv_transform.into())
    }

    #[setter]
    pub fn set_uv_transform(&mut self, transform: PyAffine2) -> PyResult<()> {
        self.as_mut()?.uv_transform = transform.into();
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
    pub fn flipped(
        slf: Py<Self>,
        py: Python<'_>,
        horizontal: bool,
        vertical: bool,
    ) -> PyResult<Py<Self>> {
        slf.borrow_mut(py)
            .storage
            .as_mut()?
            .flip(horizontal, vertical);
        Ok(slf)
    }
}
