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

#[cfg(test)]
mod tests {
    use bevy::image::ImageCompareFunction;

    use super::*;

    #[test]
    fn rejects_invalid_descriptors_before_gpu_creation() {
        let mut binding = MaterialSamplerBinding {
            kind: MaterialSamplerKind::NonFiltering,
            descriptor: ImageSamplerDescriptor::default(),
        };
        assert!(binding.validate(WgpuFeatures::empty()).is_ok());
        binding.descriptor.mag_filter = ImageFilterMode::Linear;
        assert!(binding.validate(WgpuFeatures::empty()).is_err());
        binding.kind = MaterialSamplerKind::Filtering;
        assert!(binding.validate(WgpuFeatures::empty()).is_ok());
        binding.descriptor.compare = Some(ImageCompareFunction::Less);
        assert!(binding.validate(WgpuFeatures::empty()).is_err());
        binding.kind = MaterialSamplerKind::Comparison;
        assert!(binding.validate(WgpuFeatures::empty()).is_ok());
        for value in [f32::NAN, f32::INFINITY, -1.0, 33.0] {
            binding.descriptor.lod_min_clamp = value;
            assert!(binding.validate(WgpuFeatures::empty()).is_err());
        }
        binding.descriptor = ImageSamplerDescriptor::default();
        binding.kind = MaterialSamplerKind::Filtering;
        binding.descriptor.anisotropy_clamp = 0;
        assert!(binding.validate(WgpuFeatures::empty()).is_err());
        binding.descriptor.anisotropy_clamp = 2;
        assert!(binding.validate(WgpuFeatures::empty()).is_err());
        binding.descriptor = ImageSamplerDescriptor::linear();
        binding.descriptor.anisotropy_clamp = 32;
        assert!(binding.validate(WgpuFeatures::empty()).is_ok());
        binding.descriptor.address_mode_u = ImageAddressMode::ClampToBorder;
        assert!(binding.validate(WgpuFeatures::empty()).is_err());
        assert!(
            binding
                .validate(WgpuFeatures::ADDRESS_MODE_CLAMP_TO_BORDER)
                .is_ok()
        );
        binding.descriptor.border_color = Some(ImageSamplerBorderColor::Zero);
        assert!(
            binding
                .validate(WgpuFeatures::ADDRESS_MODE_CLAMP_TO_BORDER)
                .is_err()
        );
    }

    #[test]
    fn cache_shares_reuses_and_retires_sampler_allocations() {
        let (device, _) = wgpu::Device::noop(&Default::default());
        let device = RenderDevice::from(device);
        let mut cache = MaterialSamplerCache::default();
        let mut binding = MaterialSamplerBinding {
            kind: MaterialSamplerKind::Filtering,
            descriptor: ImageSamplerDescriptor::default(),
        };
        cache.begin_frame();
        let first = cache.prepare(&binding, &device).unwrap().id();
        assert_eq!(cache.prepare(&binding, &device).unwrap().id(), first);
        cache.finish_frame();
        cache.begin_frame();
        assert_eq!(cache.prepare(&binding, &device).unwrap().id(), first);
        binding.descriptor = ImageSamplerDescriptor::linear();
        assert_ne!(cache.prepare(&binding, &device).unwrap().id(), first);
        cache.finish_frame();
        assert_eq!(cache.entries.len(), 2);
        cache.begin_frame();
        cache.prepare(&binding, &device).unwrap();
        cache.finish_frame();
        assert_eq!(cache.entries.len(), 1);
        binding.kind = MaterialSamplerKind::NonFiltering;
        cache.begin_frame();
        assert!(cache.prepare(&binding, &device).is_err());
        cache.finish_frame();
        assert!(cache.entries.is_empty());
    }
}
