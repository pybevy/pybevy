//! Native sampled-texture layout metadata and pre-bind validation.
use bevy::{
    image::{ImageFilterMode, ImageSamplerDescriptor},
    reflect::Reflect,
    render::{
        render_resource::{
            BindingType, SamplerBindingType, TextureDimension, TextureSampleType, TextureUsages,
            TextureViewDimension,
        },
        renderer::RenderDevice,
        texture::GpuImage,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub struct TextureBindingLayout {
    dimension: String,
    sample_type: String,
    filterable: bool,
    sampler: String,
}

impl TextureBindingLayout {
    pub fn parse(
        dimension: &str,
        sample_type: &str,
        filterable: bool,
        sampler: &str,
    ) -> Result<Self, String> {
        if !["2d", "2d_array", "cube", "cube_array", "3d"].contains(&dimension) {
            return Err("Texture dimension must be 2d, 2d_array, cube, cube_array, or 3d".into());
        }
        if !["float", "sint", "uint", "depth"].contains(&sample_type) {
            return Err("Texture sample_type must be float, sint, uint, or depth".into());
        }
        if !["filtering", "non_filtering", "comparison", "none"].contains(&sampler) {
            return Err(
                "Texture sampler must be filtering, non_filtering, comparison, or none".into(),
            );
        }
        if (sample_type != "float" && filterable)
            || (sampler == "comparison" && sample_type != "depth")
            || (sampler == "filtering" && (sample_type != "float" || !filterable))
            || (sample_type == "depth" && dimension == "3d")
        {
            return Err(
                "Incompatible texture sample type, filterability, sampler, or dimension".into(),
            );
        }
        Ok(Self {
            dimension: dimension.into(),
            sample_type: sample_type.into(),
            filterable,
            sampler: sampler.into(),
        })
    }

    pub fn view_dimension(&self) -> TextureViewDimension {
        match self.dimension.as_str() {
            "2d_array" => TextureViewDimension::D2Array,
            "cube" => TextureViewDimension::Cube,
            "cube_array" => TextureViewDimension::CubeArray,
            "3d" => TextureViewDimension::D3,
            _ => TextureViewDimension::D2,
        }
    }

    pub fn has_sampler(&self) -> bool {
        self.sampler != "none"
    }

    pub fn sampler_type(&self) -> SamplerBindingType {
        match self.sampler.as_str() {
            "non_filtering" => SamplerBindingType::NonFiltering,
            "comparison" => SamplerBindingType::Comparison,
            _ => SamplerBindingType::Filtering,
        }
    }

    fn sample(&self) -> TextureSampleType {
        match self.sample_type.as_str() {
            "sint" => TextureSampleType::Sint,
            "uint" => TextureSampleType::Uint,
            "depth" => TextureSampleType::Depth,
            _ => TextureSampleType::Float {
                filterable: self.filterable,
            },
        }
    }

    pub fn binding_type(&self) -> BindingType {
        BindingType::Texture {
            sample_type: self.sample(),
            view_dimension: self.view_dimension(),
            multisampled: false,
        }
    }

    pub fn wgsl_types(&self) -> (String, String) {
        let texture = if self.sample_type == "depth" {
            format!("texture_depth_{}", self.dimension)
        } else {
            let scalar = match self.sample_type.as_str() {
                "sint" => "i32",
                "uint" => "u32",
                _ => "f32",
            };
            format!("texture_{}<{scalar}>", self.dimension)
        };
        let sampler = if self.sampler == "comparison" {
            "sampler_comparison"
        } else {
            "sampler"
        };
        (texture, sampler.into())
    }

    pub fn validate(
        &self,
        image: &GpuImage,
        sampler: &ImageSamplerDescriptor,
        device: &RenderDevice,
    ) -> Result<(), String> {
        let texture = &image.texture_descriptor;
        let view = image.texture_view_descriptor.as_ref();
        let dimension = view
            .and_then(|view| view.dimension)
            .unwrap_or(match texture.dimension {
                TextureDimension::D1 => TextureViewDimension::D1,
                TextureDimension::D3 => TextureViewDimension::D3,
                TextureDimension::D2 if texture.size.depth_or_array_layers > 1 => {
                    TextureViewDimension::D2Array
                }
                TextureDimension::D2 => TextureViewDimension::D2,
            });
        if dimension != self.view_dimension()
            || texture.sample_count != 1
            || !texture.usage.contains(TextureUsages::TEXTURE_BINDING)
        {
            return Err(format!(
                "Texture requires a single-sampled {:?} view with TEXTURE_BINDING usage; got {dimension:?}",
                self.view_dimension()
            ));
        }
        let format = view.and_then(|view| view.format).unwrap_or(texture.format);
        let actual = format.sample_type(view.map(|view| view.aspect), Some(device.features()));
        let compatible = match (self.sample(), actual) {
            (
                TextureSampleType::Float { filterable: false },
                Some(TextureSampleType::Float { .. }),
            ) => true,
            (expected, Some(actual)) => expected == actual,
            _ => false,
        };
        if !compatible {
            return Err(format!(
                "Texture format {format:?} is incompatible with {:?}",
                self.sample()
            ));
        }
        if !self.has_sampler() {
            return Ok(());
        }
        let linear = sampler.mag_filter == ImageFilterMode::Linear
            || sampler.min_filter == ImageFilterMode::Linear
            || sampler.mipmap_filter == ImageFilterMode::Linear;
        if (self.sampler_type() == SamplerBindingType::Comparison) != sampler.compare.is_some()
            || (self.sampler_type() == SamplerBindingType::NonFiltering && linear)
        {
            return Err(format!(
                "Image sampler is incompatible with {:?}; set Image.sampler to a matching descriptor",
                self.sampler_type()
            ));
        }
        Ok(())
    }
}
