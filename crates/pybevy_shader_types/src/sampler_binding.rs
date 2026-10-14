//! Independent material samplers, validated before device allocation.
use bevy::{
    ecs::resource::Resource,
    image::{ImageAddressMode, ImageFilterMode, ImageSamplerBorderColor, ImageSamplerDescriptor},
    reflect::Reflect,
    render::{
        render_resource::{Sampler, SamplerBindingType},
        renderer::RenderDevice,
        settings::WgpuFeatures,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect)]
pub enum MaterialSamplerKind {
    Filtering,
    NonFiltering,
    Comparison,
}

impl MaterialSamplerKind {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "filtering" => Ok(Self::Filtering),
            "non_filtering" => Ok(Self::NonFiltering),
            "comparison" => Ok(Self::Comparison),
            _ => Err("Sampler kind must be filtering, non_filtering, or comparison".into()),
        }
    }
    pub fn binding_type(self) -> SamplerBindingType {
        match self {
            Self::Filtering => SamplerBindingType::Filtering,
            Self::NonFiltering => SamplerBindingType::NonFiltering,
            Self::Comparison => SamplerBindingType::Comparison,
        }
    }
}

#[derive(Clone, Debug, Reflect)]
pub struct MaterialSamplerBinding {
    pub kind: MaterialSamplerKind,
    pub descriptor: ImageSamplerDescriptor,
}

impl MaterialSamplerBinding {
    pub fn validate(&self, features: WgpuFeatures) -> Result<(), String> {
        let d = &self.descriptor;
        let filters = [d.mag_filter, d.min_filter, d.mipmap_filter];
        if !d.lod_min_clamp.is_finite()
            || !d.lod_max_clamp.is_finite()
            || d.lod_min_clamp < 0.0
            || d.lod_max_clamp < d.lod_min_clamp
        {
            return Err("Material sampler LOD clamps must be finite and 0 <= min <= max".into());
        }
        if d.anisotropy_clamp == 0
            || (d.anisotropy_clamp > 1 && filters.iter().any(|f| *f != ImageFilterMode::Linear))
        {
            return Err("Material sampler anisotropy must be >= 1 and requires all-linear filtering above 1".into());
        }
        if (self.kind == MaterialSamplerKind::Comparison) != d.compare.is_some() {
            return Err(if self.kind == MaterialSamplerKind::Comparison {
                "compare must be set for a Comparison sampler".into()
            } else {
                "compare must be None for a non-comparison sampler".into()
            });
        }
        if self.kind == MaterialSamplerKind::NonFiltering {
            for (name, filter) in [
                ("mag_filter", d.mag_filter),
                ("min_filter", d.min_filter),
                ("mipmap_filter", d.mipmap_filter),
            ] {
                if filter != ImageFilterMode::Nearest {
                    return Err(format!("{name} must be Nearest for a NonFiltering sampler"));
                }
            }
        }
        if [d.address_mode_u, d.address_mode_v, d.address_mode_w]
            .contains(&ImageAddressMode::ClampToBorder)
            && !features.contains(WgpuFeatures::ADDRESS_MODE_CLAMP_TO_BORDER)
        {
            return Err("Material sampler requires ADDRESS_MODE_CLAMP_TO_BORDER".into());
        }
        if d.border_color == Some(ImageSamplerBorderColor::Zero)
            && !features.contains(WgpuFeatures::ADDRESS_MODE_CLAMP_TO_ZERO)
        {
            return Err("Material sampler requires ADDRESS_MODE_CLAMP_TO_ZERO".into());
        }
        Ok(())
    }
}

struct CachedSampler {
    descriptor: ImageSamplerDescriptor,
    sampler: Sampler,
    used: bool,
}

#[derive(Resource, Default)]
pub struct MaterialSamplerCache {
    entries: Vec<CachedSampler>,
}

impl MaterialSamplerCache {
    pub fn begin_frame(&mut self) {
        for entry in &mut self.entries {
            entry.used = false;
        }
    }
    pub fn finish_frame(&mut self) {
        self.entries.retain(|entry| entry.used);
    }
    pub fn get(&self, descriptor: &ImageSamplerDescriptor) -> Option<&Sampler> {
        self.entries
            .iter()
            .find(|entry| entry.descriptor == *descriptor)
            .map(|entry| &entry.sampler)
    }
    pub fn prepare(
        &mut self,
        binding: &MaterialSamplerBinding,
        device: &RenderDevice,
    ) -> Result<&Sampler, String> {
        binding.validate(device.features())?;
        let index = self
            .entries
            .iter()
            .position(|entry| entry.descriptor == binding.descriptor)
            .unwrap_or_else(|| {
                self.entries.push(CachedSampler {
                    descriptor: binding.descriptor.clone(),
                    sampler: device.create_sampler(&binding.descriptor.as_wgpu()),
                    used: false,
                });
                self.entries.len() - 1
            });
        let entry = &mut self.entries[index];
        entry.used = true;
        Ok(&entry.sampler)
    }
}
