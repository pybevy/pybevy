//! Opt-in read-only storage binding. Ordinary material layouts are unchanged.

use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
};

use bevy::{
    app::{App, Last, Plugin},
    asset::{Asset, AssetId, Assets, Handle},
    ecs::{
        prelude::*,
        system::{SystemParamItem, lifetimeless::SRes},
    },
    image::{Image, ImageSampler, ImageSamplerDescriptor},
    material::{AlphaMode, OpaqueRendererMethod},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{
        MATERIAL_BIND_GROUP_INDEX, Material, MaterialExtension, MaterialExtensionKey,
        MaterialExtensionPipeline, MaterialPipeline, MaterialPipelineKey, MaterialPlugin,
        MeshMaterial3d, StandardMaterial,
    },
    reflect::Reflect,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        erased_render_asset::{AssetExtractionSystems, ExtractedAssets, prepare_erased_assets},
        material_bind_groups::{
            FallbackBuffer, MaterialBindGroupAllocators, RenderMaterialBindings,
        },
        render_asset::{RenderAssets, prepare_assets},
        render_resource::{
            AsBindGroup, AsBindGroupError, BindGroupBuilder, BindGroupEntry, BindGroupLayout,
            BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResources, BindingType,
            BufferBindingType, BufferId, BufferInitDescriptor, BufferSize, BufferUsages,
            DefaultImageSamplerDescriptor, DownlevelFlags, OwnedBindingResource, OwnedData,
            PipelineCache, PreparedBindGroup, RenderPipelineDescriptor, SamplerBindingType,
            SamplerId, ShaderStages, SpecializedMeshPipelineError, TextureViewId,
            UnpreparedBindingResource,
        },
        renderer::{RenderAdapter, RenderDevice},
        settings::WgpuLimits,
        storage::{GpuShaderBuffer, ShaderBuffer},
        texture::{FallbackImage, GpuImage},
    },
    shader::ShaderRef,
};

use crate::{
    ShaderMaterial, ShaderMaterialExtension, ShaderMaterialKey, clear_shader_registries,
    expected_generated_material_import,
    sampler_binding::{MaterialSamplerBinding, MaterialSamplerCache, MaterialSamplerKind},
    storage_element::{StorageElement, generated_bindings_wgsl},
    sync_shader_handles,
    texture_binding::TextureBindingLayout,
};

pub const STORAGE_BINDING: u32 = 109;

#[derive(Reflect, Debug, Clone, Default)]
pub struct StorageBinding {
    pub handle: Option<Handle<ShaderBuffer>>,
    pub element: StorageElement,
}

#[derive(Reflect, Debug, Clone)]
pub struct StorageShaderMaterialExtension {
    pub inner: ShaderMaterialExtension,
    pub buffers: Vec<StorageBinding>,
    pub samplers: Vec<MaterialSamplerBinding>,
    pub texture_layouts: Vec<Option<TextureBindingLayout>>,
}

impl Default for StorageShaderMaterialExtension {
    fn default() -> Self {
        Self {
            inner: ShaderMaterialExtension::default(),
            buffers: vec![StorageBinding::default()],
            texture_layouts: Vec::new(),
            samplers: Vec::new(),
        }
    }
}

impl Deref for StorageShaderMaterialExtension {
    type Target = ShaderMaterialExtension;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for StorageShaderMaterialExtension {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[derive(Asset, Reflect, Debug, Clone, Default)]
pub struct StorageShaderMaterial {
    pub base: StandardMaterial,
    pub extension: StorageShaderMaterialExtension,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct StorageMaterialKey {
    base: <StandardMaterial as AsBindGroup>::Data,
    extension: ShaderMaterialKey,
    strides: Vec<u32>,
    textures: Vec<Option<TextureBindingLayout>>,
    samplers: Vec<MaterialSamplerKind>,
}

fn binding_field_name(extension: &ShaderMaterialExtension, binding: u32) -> String {
    let Some(source) = extension.bindings_wgsl.as_deref() else {
        return format!("binding {binding}");
    };
    let source = generated_bindings_wgsl(source);
    naga::front::wgsl::parse_str(&source)
        .ok()
        .and_then(|module| {
            module
                .global_variables
                .iter()
                .find(|(_, variable)| {
                    variable.binding.as_ref().is_some_and(|b| {
                        b.group == MATERIAL_BIND_GROUP_INDEX as u32 && b.binding == binding
                    })
                })
                .and_then(|(_, variable)| variable.name.clone())
        })
        .unwrap_or_else(|| format!("binding {binding}"))
}

/// Checked before binding; an absent or invalid buffer must not produce a draw.
pub fn validate_storage_buffer(
    size: u64,
    usage: BufferUsages,
    max_size: u64,
) -> Result<(), String> {
    validate_storage_element_buffer(size, usage, max_size, 16)
}

pub fn validate_storage_element_buffer(
    size: u64,
    usage: BufferUsages,
    max_size: u64,
    stride: u32,
) -> Result<(), String> {
    if stride == 0 || size == 0 || !size.is_multiple_of(u64::from(stride)) {
        return Err(format!(
            "Material storage buffer must contain a nonempty array with element stride {stride} (byte size divisible by {stride}); got {size} bytes"
        ));
    }
    if size > max_size {
        return Err(format!(
            "Material storage buffer has {size} bytes, exceeding the device binding limit of {max_size}"
        ));
    }
    if !usage.contains(BufferUsages::STORAGE) {
        return Err("Material storage buffer requires STORAGE usage".into());
    }
    Ok(())
}

fn storage_layout_error(device: &RenderDevice, entries: &[BindGroupLayoutEntry]) -> Option<String> {
    storage_layout_error_for_limits(&device.limits(), entries)
}

fn storage_layout_error_for_limits(
    limits: &WgpuLimits,
    entries: &[BindGroupLayoutEntry],
) -> Option<String> {
    if limits.max_bind_groups < 4
        || entries
            .iter()
            .any(|entry| entry.binding >= limits.max_bindings_per_bind_group)
    {
        return Some(format!(
            "ShaderMaterialPlugin requires material group 3 and binding indices below {}; highest requested binding is {}",
            limits.max_bindings_per_bind_group,
            entries.iter().map(|entry| entry.binding).max().unwrap_or(0)
        ));
    }
    for stage in [ShaderStages::VERTEX, ShaderStages::FRAGMENT] {
        let mut counts = [0u32; 4];
        for entry in entries
            .iter()
            .filter(|entry| entry.visibility.intersects(stage))
        {
            match entry.ty {
                BindingType::Buffer {
                    ty: BufferBindingType::Storage { .. },
                    ..
                } => counts[0] += 1,
                BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    ..
                } => counts[1] += 1,
                BindingType::Texture { .. } => counts[2] += 1,
                BindingType::Sampler(_) => counts[3] += 1,
                _ => {}
            }
        }
        let maxima = [
            limits.max_storage_buffers_per_shader_stage,
            limits.max_uniform_buffers_per_shader_stage,
            limits.max_sampled_textures_per_shader_stage,
            limits.max_samplers_per_shader_stage,
        ];
        if counts
            .into_iter()
            .zip(maxima)
            .any(|(count, limit)| count > limit)
        {
            return Some(format!(
                "ShaderMaterialPlugin combined material layout exceeds device limits for {stage:?}: required {counts:?}, available {maxima:?} (storage, uniform, texture, sampler)"
            ));
        }
    }
    None
}

fn storage_entries(strides: &[u32]) -> Vec<BindGroupLayoutEntry> {
    strides
        .iter()
        .enumerate()
        .map(|(index, stride)| BindGroupLayoutEntry {
            binding: STORAGE_BINDING + index as u32,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: BufferSize::new(u64::from(*stride)),
            },
            count: None,
        })
        .collect()
}

#[derive(Resource, Default)]
pub struct StorageBufferAvailability {
    live: HashSet<AssetId<ShaderBuffer>>,
    images: HashMap<AssetId<Image>, ImageSampler>,
    unsupported: bool,
}

impl AsBindGroup for StorageShaderMaterial {
    type Data = StorageMaterialKey;
    type Param = (
        <StandardMaterial as AsBindGroup>::Param,
        <ShaderMaterialExtension as AsBindGroup>::Param,
        SRes<StorageBufferAvailability>,
        SRes<RenderAssets<GpuImage>>,
        SRes<DefaultImageSamplerDescriptor>,
        SRes<MaterialSamplerCache>,
        SRes<FallbackImage>,
    );

    fn label() -> &'static str {
        "pybevy_storage_material"
    }

    fn bind_group_data(&self) -> Self::Data {
        StorageMaterialKey {
            samplers: self.extension.samplers.iter().map(|s| s.kind).collect(),
            base: self.base.bind_group_data(),
            textures: self.extension.texture_layouts.clone(),
            extension: ShaderMaterialKey::from(&self.extension.inner),
            strides: self
                .extension
                .buffers
                .iter()
                .map(|buffer| buffer.element.stride)
                .collect(),
        }
    }

    fn build_bind_group(
        &self,
        _: &BindGroupLayout,
        _: &RenderDevice,
        _: &mut SystemParamItem<Self::Param>,
        _: bool,
        _: &mut BindGroupBuilder,
    ) -> Result<(), AsBindGroupError> {
        Err(AsBindGroupError::CreateBindGroupDirectly)
    }

    fn as_bind_group(
        &self,
        descriptor: &BindGroupLayoutDescriptor,
        device: &RenderDevice,
        cache: &PipelineCache,
        fallback_buffer: &FallbackBuffer,
        shader_buffers: &RenderAssets<GpuShaderBuffer>,
        (
            base_param,
            extension_param,
            availability,
            images,
            default_sampler,
            sampler_cache,
            fallback_image,
        ): &mut SystemParamItem<Self::Param>,
    ) -> Result<PreparedBindGroup, AsBindGroupError> {
        if self.extension.texture_layouts.len() > 4
            || (availability.unsupported && !self.extension.buffers.is_empty())
        {
            return Err(AsBindGroupError::RetryNextUpdate);
        }
        let mut descriptor = descriptor.clone();
        apply_texture_layouts(&mut descriptor.entries, &self.extension.texture_layouts);
        descriptor
            .entries
            .extend(storage_entries(&self.bind_group_data().strides));
        descriptor.entries.extend(sampler_entries(
            self.extension.buffers.len(),
            self.extension.samplers.iter().map(|s| s.kind),
        ));
        if storage_layout_error(device, &descriptor.entries).is_some() {
            return Err(AsBindGroupError::RetryNextUpdate);
        }
        let mut storage = Vec::new();
        for (slot, binding) in self.extension.buffers.iter().enumerate() {
            let handle = binding
                .handle
                .as_ref()
                .filter(|handle| availability.live.contains(&handle.id()))
                .ok_or(AsBindGroupError::RetryNextUpdate)?;
            let buffer = shader_buffers
                .get(handle)
                .ok_or(AsBindGroupError::RetryNextUpdate)?;
            validate_storage_element_buffer(
                buffer.buffer.size(),
                buffer.buffer.usage(),
                device.limits().max_storage_buffer_binding_size,
                binding.element.stride,
            )
            .map_err(|_| AsBindGroupError::RetryNextUpdate)?;
            storage.push((
                STORAGE_BINDING + slot as u32,
                UnpreparedBindingResource::Buffer(buffer.buffer.clone()),
            ));
        }
        let mut inner = self.extension.inner.clone();
        let mut textures = Vec::new();
        for (slot, metadata) in self.extension.texture_layouts.iter().enumerate() {
            let Some(metadata) = metadata else {
                continue;
            };
            if inner.texture_slot(slot).and_then(Option::as_ref).is_none()
                && metadata.uses_white_fallback()
            {
                let image = fallback_image.get(metadata.view_dimension());
                textures.push((
                    101 + slot as u32 * 2,
                    UnpreparedBindingResource::TextureView(
                        metadata.view_dimension(),
                        image.texture_view.clone(),
                    ),
                ));
                if metadata.has_sampler() {
                    let sampler = if metadata.sampler_type() == SamplerBindingType::NonFiltering {
                        sampler_cache
                            .get(&ImageSamplerDescriptor::default())
                            .ok_or(AsBindGroupError::RetryNextUpdate)?
                    } else {
                        &image.sampler
                    };
                    textures.push((
                        102 + slot as u32 * 2,
                        UnpreparedBindingResource::Sampler(
                            metadata.sampler_type(),
                            sampler.clone(),
                        ),
                    ));
                }
                continue;
            }
            let handle = inner
                .texture_slot(slot)
                .and_then(Option::as_ref)
                .ok_or(AsBindGroupError::RetryNextUpdate)?;
            let sampler = availability
                .images
                .get(&handle.id())
                .ok_or(AsBindGroupError::RetryNextUpdate)?;
            let image = images
                .get(handle)
                .ok_or(AsBindGroupError::RetryNextUpdate)?;
            let sampler = match sampler {
                ImageSampler::Default => &default_sampler.0,
                ImageSampler::Descriptor(value) => value,
            };
            metadata
                .validate(image, sampler, device)
                .map_err(|_| AsBindGroupError::RetryNextUpdate)?;
            textures.push((
                101 + slot as u32 * 2,
                UnpreparedBindingResource::TextureView(
                    metadata.view_dimension(),
                    image.texture_view.clone(),
                ),
            ));
            if metadata.has_sampler() {
                textures.push((
                    102 + slot as u32 * 2,
                    UnpreparedBindingResource::Sampler(
                        metadata.sampler_type(),
                        image.sampler.clone(),
                    ),
                ));
            }
            // The derive-generated ordinary path validates all textures as float 2D.
            // Let it supply uniform/fallback resources, then replace custom slots.
            *inner.texture_slot_mut(slot).unwrap() = None;
        }
        let layout = cache.get_bind_group_layout(&descriptor);
        let mut builder = BindGroupBuilder::default();
        self.base
            .build_bind_group(&layout, device, base_param, true, &mut builder)?;
        inner.build_bind_group(&layout, device, extension_param, true, &mut builder)?;
        let BindGroupBuilder {
            binding_resources,
            data_buffer,
        } = builder;
        let mut bindings = binding_resources.0;
        for (index, resource) in textures {
            *bindings
                .iter_mut()
                .find(|(binding, _)| *binding == index)
                .unwrap() = (index, resource);
        }
        bindings.retain(|(binding, _)| {
            descriptor
                .entries
                .iter()
                .any(|entry| entry.binding == *binding)
        });
        bindings.extend(storage);
        for (slot, binding) in self.extension.samplers.iter().enumerate() {
            binding
                .validate(device.features())
                .map_err(|_| AsBindGroupError::RetryNextUpdate)?;
            let sampler = sampler_cache
                .get(&binding.descriptor)
                .ok_or(AsBindGroupError::RetryNextUpdate)?;
            bindings.push((
                STORAGE_BINDING + self.extension.buffers.len() as u32 + slot as u32,
                UnpreparedBindingResource::Sampler(binding.kind.binding_type(), sampler.clone()),
            ));
        }
        // Direct preparation must perform the uniform packing normally done by the allocator.
        for (_, resource) in bindings.iter_mut() {
            if let UnpreparedBindingResource::Data(range) = resource {
                *resource = UnpreparedBindingResource::Buffer(device.create_buffer_with_data(
                    &BufferInitDescriptor {
                        label: Some(Self::label()),
                        contents: &data_buffer[range.start as usize..range.end as usize],
                        usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
                    },
                ));
            }
        }
        let entries: Vec<_> = bindings
            .iter()
            .map(|(binding, resource)| BindGroupEntry {
                binding: *binding,
                resource: resource.get_binding(fallback_buffer, shader_buffers),
            })
            .collect();
        let bind_group = device.create_bind_group(Self::label(), &layout, &entries);
        let bindings = bindings
            .into_iter()
            .map(|(binding, resource)| {
                let resource = match resource {
                    UnpreparedBindingResource::Buffer(buffer) => {
                        OwnedBindingResource::Buffer(buffer)
                    }
                    UnpreparedBindingResource::ShaderBuffer(handle) => {
                        OwnedBindingResource::ShaderBuffer(handle)
                    }
                    UnpreparedBindingResource::TextureView(dimension, view) => {
                        OwnedBindingResource::TextureView(dimension, view)
                    }
                    UnpreparedBindingResource::Sampler(kind, sampler) => {
                        OwnedBindingResource::Sampler(kind, sampler)
                    }
                    UnpreparedBindingResource::Data(range) => OwnedBindingResource::Data(
                        OwnedData(data_buffer[range.start as usize..range.end as usize].to_vec()),
                    ),
                };
                (binding, resource)
            })
            .collect();
        Ok(PreparedBindGroup {
            bindings: BindingResources(bindings),
            bind_group,
        })
    }

    fn bind_group_layout_entries(device: &RenderDevice, _: bool) -> Vec<BindGroupLayoutEntry> {
        let mut entries = StandardMaterial::bind_group_layout_entries(device, true);
        entries.extend(ShaderMaterialExtension::bind_group_layout_entries(
            device, true,
        ));
        entries
    }
}

impl Material for StorageShaderMaterial {
    fn vertex_shader() -> ShaderRef {
        StandardMaterial::vertex_shader()
    }
    fn fragment_shader() -> ShaderRef {
        StandardMaterial::fragment_shader()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        StandardMaterial::prepass_vertex_shader()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        StandardMaterial::prepass_fragment_shader()
    }
    fn deferred_vertex_shader() -> ShaderRef {
        StandardMaterial::deferred_vertex_shader()
    }
    fn deferred_fragment_shader() -> ShaderRef {
        StandardMaterial::deferred_fragment_shader()
    }
    fn alpha_mode(&self) -> AlphaMode {
        self.base.alpha_mode()
    }
    fn opaque_render_method(&self) -> OpaqueRendererMethod {
        self.base.opaque_render_method()
    }
    fn depth_bias(&self) -> f32 {
        self.base.depth_bias()
    }
    fn reads_view_transmission_texture(&self) -> bool {
        self.base.reads_view_transmission_texture()
    }
    fn specialize(
        pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        StandardMaterial::specialize(
            pipeline,
            descriptor,
            layout,
            MaterialPipelineKey {
                mesh_key: key.mesh_key,
                bind_group_data: key.bind_group_data.base,
            },
        )?;
        ShaderMaterialExtension::specialize(
            &MaterialExtensionPipeline {
                mesh_pipeline: pipeline.mesh_pipeline.clone(),
            },
            descriptor,
            layout,
            MaterialExtensionKey {
                mesh_key: key.mesh_key,
                bind_group_data: key.bind_group_data.extension,
            },
        )?;
        apply_texture_layouts(
            &mut descriptor.layout[3].entries,
            &key.bind_group_data.textures,
        );
        descriptor.layout[3]
            .entries
            .extend(storage_entries(&key.bind_group_data.strides));
        descriptor.layout[3].entries.extend(sampler_entries(
            key.bind_group_data.strides.len(),
            key.bind_group_data.samplers.iter().copied(),
        ));
        Ok(())
    }
}

fn sampler_entries(
    storage_count: usize,
    kinds: impl IntoIterator<Item = MaterialSamplerKind>,
) -> Vec<BindGroupLayoutEntry> {
    kinds
        .into_iter()
        .enumerate()
        .map(|(slot, kind)| BindGroupLayoutEntry {
            binding: STORAGE_BINDING + storage_count as u32 + slot as u32,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty: BindingType::Sampler(kind.binding_type()),
            count: None,
        })
        .collect()
}

/// Registers every native backing type used by Python material declarations.
#[derive(Default)]
pub struct ShaderMaterialPlugin;

impl Plugin for ShaderMaterialPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<MaterialBackendsPlugin>() {
            app.add_plugins(MaterialBackendsPlugin);
        }
    }
}

struct MaterialBackendsPlugin;

impl Plugin for MaterialBackendsPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<MaterialPlugin<ShaderMaterial>>() {
            clear_shader_registries();
            app.add_plugins(MaterialPlugin::<ShaderMaterial>::default());
            app.add_systems(Last, sync_shader_handles);
        }
        app.add_plugins(MaterialPlugin::<StorageShaderMaterial>::default());
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<StorageBufferAvailability>()
                .init_resource::<MaterialSamplerCache>()
                .init_resource::<StorageMaterialDependencies>()
                .add_systems(
                    ExtractSchedule,
                    extract_storage_dependencies.after(AssetExtractionSystems),
                )
                .add_systems(
                    Render,
                    refresh_storage_bindings
                        .in_set(RenderSystems::PrepareAssets)
                        .after(prepare_assets::<GpuShaderBuffer>)
                        .after(prepare_assets::<GpuImage>)
                        .before(prepare_erased_assets::<MeshMaterial3d<StorageShaderMaterial>>),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            let flags = render_app
                .world()
                .resource::<RenderAdapter>()
                .get_downlevel_capabilities()
                .flags;
            let required = DownlevelFlags::VERTEX_STORAGE | DownlevelFlags::FRAGMENT_STORAGE;
            render_app
                .world_mut()
                .resource_mut::<StorageBufferAvailability>()
                .unsupported = !flags.contains(required);
        }
    }
}

fn apply_texture_layouts(
    entries: &mut Vec<BindGroupLayoutEntry>,
    layouts: &[Option<TextureBindingLayout>],
) {
    for (slot, layout) in layouts.iter().enumerate() {
        let Some(layout) = layout else {
            continue;
        };
        if !layout.has_sampler() {
            entries.retain(|entry| entry.binding != 102 + slot as u32 * 2);
        }
        for entry in entries.iter_mut() {
            if entry.binding == 101 + slot as u32 * 2 {
                entry.ty = layout.binding_type();
            }
            if entry.binding == 102 + slot as u32 * 2 {
                entry.ty = BindingType::Sampler(layout.sampler_type());
            }
        }
    }
}

#[derive(PartialEq, Eq)]
struct MaterialAllocation {
    buffers: Vec<BufferId>,
    textures: Vec<(TextureViewId, Option<SamplerId>)>,
    samplers: Vec<SamplerId>,
}

struct MaterialDependency {
    material: StorageShaderMaterial,
    allocation: Option<MaterialAllocation>,
    diagnostic: Option<String>,
}

#[derive(Resource, Default)]
struct StorageMaterialDependencies(HashMap<AssetId<StorageShaderMaterial>, MaterialDependency>);

fn extract_storage_dependencies(
    buffers: Extract<Res<Assets<ShaderBuffer>>>,
    images: Extract<Res<Assets<Image>>>,
    extracted: Res<ExtractedAssets<MeshMaterial3d<StorageShaderMaterial>>>,
    mut availability: ResMut<StorageBufferAvailability>,
    mut dependencies: ResMut<StorageMaterialDependencies>,
) {
    availability.live = buffers.ids().collect();
    for id in &extracted.removed {
        dependencies.0.remove(id);
    }
    for (id, material) in &extracted.extracted {
        if !extracted.removed.contains(id) {
            dependencies
                .0
                .entry(*id)
                .and_modify(|dependency| {
                    // Preserve the last diagnostic while changed inputs are revalidated.
                    dependency.material = material.clone();
                    dependency.allocation = None;
                })
                .or_insert_with(|| MaterialDependency {
                    material: material.clone(),
                    allocation: None,
                    diagnostic: None,
                });
        }
    }
    availability.images.clear();
    for dependency in dependencies.0.values() {
        for slot in 0..4 {
            if let Some(handle) = dependency
                .material
                .extension
                .inner
                .texture_slot(slot)
                .and_then(Option::as_ref)
                && let Some(image) = images.get(handle)
            {
                availability
                    .images
                    .insert(handle.id(), image.sampler.clone());
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn refresh_storage_bindings(
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    images: Res<RenderAssets<GpuImage>>,
    default_sampler: Res<DefaultImageSamplerDescriptor>,
    fallback: Res<FallbackImage>,
    device: Res<RenderDevice>,
    availability: Res<StorageBufferAvailability>,
    mut dependencies: ResMut<StorageMaterialDependencies>,
    mut extracted: ResMut<ExtractedAssets<MeshMaterial3d<StorageShaderMaterial>>>,
    mut bindings: ResMut<RenderMaterialBindings>,
    mut allocators: ResMut<MaterialBindGroupAllocators>,
    mut sampler_cache: ResMut<MaterialSamplerCache>,
    mut reported_pending: Local<HashSet<String>>,
) {
    let mut pending = HashSet::new();
    sampler_cache.begin_frame();
    let fresh: HashSet<_> = extracted.extracted.iter().map(|(id, _)| *id).collect();
    for (id, dependency) in &mut dependencies.0 {
        let mut diagnostic = if dependency.material.extension.texture_layouts.len() > 4 {
            Some("At most four texture layouts are supported".into())
        } else if availability.unsupported && !dependency.material.extension.buffers.is_empty() {
            Some("Device lacks vertex/fragment storage support".into())
        } else {
            None
        };
        let allocation: Option<Vec<_>> = dependency
            .material
            .extension
            .buffers
            .iter()
            .enumerate()
            .map(|(slot, binding)| {
                let handle = binding
                    .handle
                    .as_ref()
                    .filter(|handle| availability.live.contains(&handle.id()))?;
                let buffer = buffers.get(handle)?;
                if buffer.buffer.size() == 0 {
                    return None;
                }
                if let Err(error) = validate_storage_element_buffer(
                    buffer.buffer.size(),
                    buffer.buffer.usage(),
                    device.limits().max_storage_buffer_binding_size,
                    binding.element.stride,
                ) {
                    diagnostic = Some(format!(
                        "storage field {} (binding {}): {error}",
                        binding_field_name(&dependency.material.extension.inner, 109 + slot as u32),
                        109 + slot
                    ));
                    return None;
                }
                Some(buffer.buffer.id())
            })
            .collect();
        for (slot, metadata) in dependency
            .material
            .extension
            .texture_layouts
            .iter()
            .enumerate()
        {
            if metadata
                .as_ref()
                .is_some_and(|layout| !layout.uses_white_fallback())
                && dependency
                    .material
                    .extension
                    .inner
                    .texture_slot(slot)
                    .and_then(Option::as_ref)
                    .is_none()
            {
                let extension = &dependency.material.extension.inner;
                let label = extension
                    .bindings_wgsl
                    .as_deref()
                    .and_then(expected_generated_material_import)
                    .or(extension.fragment_shader_path.as_deref())
                    .unwrap_or("<raw material>");
                let field = binding_field_name(extension, 101 + 2 * slot as u32);
                pending.insert(format!("Material {label}: texture field {field} is pending; assign an Image handle matching its texture contract"));
            }
        }
        let textures: Option<Vec<_>> = (0..4)
            .filter_map(|slot| {
                let metadata = dependency
                    .material
                    .extension
                    .texture_layouts
                    .get(slot)
                    .and_then(Option::as_ref);
                let handle = dependency
                    .material
                    .extension
                    .inner
                    .texture_slot(slot)
                    .and_then(Option::as_ref);
                if handle.is_none() && metadata.is_none() {
                    return None;
                }
                Some((|| {
                    if handle.is_none()
                        && metadata.is_some_and(TextureBindingLayout::uses_white_fallback)
                    {
                        let metadata = metadata.unwrap();
                        let image = fallback.get(metadata.view_dimension());
                        let sampler = if !metadata.has_sampler() {
                            None
                        } else if metadata.sampler_type() == SamplerBindingType::NonFiltering {
                            Some(
                                sampler_cache
                                    .prepare(
                                        &MaterialSamplerBinding {
                                            kind: MaterialSamplerKind::NonFiltering,
                                            descriptor: ImageSamplerDescriptor::default(),
                                        },
                                        &device,
                                    )
                                    .ok()?
                                    .id(),
                            )
                        } else {
                            Some(image.sampler.id())
                        };
                        return Some((image.texture_view.id(), sampler));
                    }
                    let handle = handle?;
                    let sampler = availability.images.get(&handle.id())?;
                    let image = images.get(handle)?;
                    if let Some(metadata) = metadata {
                        let sampler = match sampler {
                            ImageSampler::Default => &default_sampler.0,
                            ImageSampler::Descriptor(value) => value,
                        };
                        if let Err(error) = metadata.validate(image, sampler, &device) {
                            diagnostic = Some(error);
                            return None;
                        }
                    }
                    Some((
                        image.texture_view.id(),
                        metadata
                            .is_none_or(TextureBindingLayout::has_sampler)
                            .then(|| image.sampler.id()),
                    ))
                })())
            })
            .collect();
        let samplers: Option<Vec<_>> = dependency
            .material
            .extension
            .samplers
            .iter()
            .enumerate()
            .map(
                |(slot, binding)| match sampler_cache.prepare(binding, &device) {
                    Ok(sampler) => Some(sampler.id()),
                    Err(error) => {
                        let number = 109 + dependency.material.extension.buffers.len() + slot;
                        diagnostic = Some(format!(
                            "sampler binding {number} ({:?}): {error}",
                            binding.kind
                        ));
                        None
                    }
                },
            )
            .collect();
        let allocation =
            allocation
                .zip(textures)
                .zip(samplers)
                .map(|((buffers, textures), samplers)| MaterialAllocation {
                    buffers,
                    textures,
                    samplers,
                });
        let mut entries = StorageShaderMaterial::bind_group_layout_entries(&device, true);
        entries.extend(storage_entries(
            &dependency.material.bind_group_data().strides,
        ));
        apply_texture_layouts(&mut entries, &dependency.material.extension.texture_layouts);
        entries.extend(sampler_entries(
            dependency.material.extension.buffers.len(),
            dependency
                .material
                .extension
                .samplers
                .iter()
                .map(|s| s.kind),
        ));
        diagnostic = diagnostic.or_else(|| storage_layout_error(&device, &entries));
        if diagnostic != dependency.diagnostic {
            if let Some(error) = &diagnostic {
                tracing::error!(
                    "Storage material {} ({id:?}): {error}",
                    dependency
                        .material
                        .extension
                        .inner
                        .bindings_wgsl
                        .as_deref()
                        .and_then(expected_generated_material_import)
                        .unwrap_or_else(|| dependency
                            .material
                            .extension
                            .inner
                            .fragment_shader_path
                            .as_deref()
                            .unwrap_or("<raw material>"))
                );
            }
            dependency.diagnostic = diagnostic;
        }
        let allocation = allocation.filter(|_| dependency.diagnostic.is_none());
        if allocation != dependency.allocation {
            dependency.allocation = allocation;
            if !fresh.contains(id) && !extracted.removed.contains(id) {
                // Supersede queued retries and evict prepared bindings through Bevy.
                extracted.extracted.push((*id, dependency.material.clone()));
                extracted.added.insert(*id);
                extracted.modified.insert(*id);
            }
        }
    }
    for message in pending.difference(&reported_pending) {
        tracing::info!("{message}");
    }
    *reported_pending = pending;
    sampler_cache.finish_frame();
    // Bevy's direct-preparation path allocates a fresh slot without retiring
    // the previous one. Retire it once before fresh inputs (including retries)
    // supersede the old binding; ordinary material removal still belongs to Bevy.
    for (id, _) in &extracted.extracted {
        if let Some(binding) = bindings.remove(&id.untyped())
            && let Some(allocator) = allocators.get_mut(&TypeId::of::<StorageShaderMaterial>())
        {
            allocator.free(binding);
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::render::{
        render_resource::{
            BindGroupLayoutEntry, BindingType, BufferBindingType, BufferUsages, SamplerBindingType,
            ShaderStages, TextureSampleType, TextureViewDimension,
        },
        settings::WgpuLimits,
    };

    use super::{
        binding_field_name, storage_entries, storage_layout_error_for_limits,
        validate_storage_buffer, validate_storage_element_buffer,
    };
    use crate::ShaderMaterialExtension;

    #[test]
    fn diagnostics_name_fields_declared_by_generated_wesl() {
        let extension = ShaderMaterialExtension {
            bindings_wgsl: Some(
                "// Auto-generated by @material - do not edit\n// module pybevy::gen::Surface\n\
                 @group(constants::MATERIAL_BIND_GROUP) @binding(101) var albedo: texture_2d<f32>;\n\
                 @group(constants::MATERIAL_BIND_GROUP) @binding(109) var<storage, read> values: array<vec4<f32>>;\n"
                    .into(),
            ),
            ..Default::default()
        };
        assert_eq!(binding_field_name(&extension, 101), "albedo");
        assert_eq!(binding_field_name(&extension, 109), "values");
        assert_eq!(binding_field_name(&extension, 110), "binding 110");
    }

    #[test]
    fn reduced_limits_check_group_binding_and_storage_boundaries() {
        let entries = storage_entries(&[16, 32]);
        let mut limits = WgpuLimits {
            max_storage_buffers_per_shader_stage: 2,
            max_bindings_per_bind_group: 111,
            ..WgpuLimits::default()
        };
        assert!(storage_layout_error_for_limits(&limits, &entries).is_none());
        limits.max_storage_buffers_per_shader_stage = 1;
        assert!(
            storage_layout_error_for_limits(&limits, &entries)
                .unwrap()
                .contains("combined material layout")
        );
        limits.max_storage_buffers_per_shader_stage = 2;
        limits.max_bindings_per_bind_group = 110;
        assert!(
            storage_layout_error_for_limits(&limits, &entries)
                .unwrap()
                .contains("highest requested binding is 110")
        );
        limits.max_bindings_per_bind_group = 111;
        limits.max_bind_groups = 3;
        assert!(
            storage_layout_error_for_limits(&limits, &entries)
                .unwrap()
                .contains("material group 3")
        );
    }

    #[test]
    fn reduced_limits_count_each_resource_and_stage_without_requiring_storage() {
        let types = [
            BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable: true },
                view_dimension: TextureViewDimension::D2,
                multisampled: false,
            },
            BindingType::Sampler(SamplerBindingType::Filtering),
        ];
        let entries: Vec<_> = types
            .into_iter()
            .enumerate()
            .flat_map(|(binding, ty)| {
                [ShaderStages::VERTEX, ShaderStages::FRAGMENT]
                    .into_iter()
                    .enumerate()
                    .map(move |(offset, visibility)| BindGroupLayoutEntry {
                        binding: (binding * 2 + offset) as u32,
                        visibility,
                        ty,
                        count: None,
                    })
            })
            .collect();
        let limits = WgpuLimits {
            max_storage_buffers_per_shader_stage: 0,
            max_uniform_buffers_per_shader_stage: 1,
            max_sampled_textures_per_shader_stage: 1,
            max_samplers_per_shader_stage: 1,
            ..WgpuLimits::default()
        };
        assert!(storage_layout_error_for_limits(&limits, &entries).is_none());
        for index in [0, 2, 4] {
            let mut combined = entries.clone();
            let mut extra = entries[index];
            extra.binding = 100;
            combined.push(extra);
            assert!(
                storage_layout_error_for_limits(&limits, &combined)
                    .unwrap()
                    .contains("combined material layout")
            );
        }
    }

    #[test]
    fn typed_buffer_size_respects_reduced_binding_limit() {
        assert!(validate_storage_element_buffer(96, BufferUsages::STORAGE, 96, 48).is_ok());
        assert!(
            validate_storage_element_buffer(96, BufferUsages::STORAGE, 95, 48)
                .unwrap_err()
                .contains("device binding limit")
        );
        assert!(
            validate_storage_element_buffer(80, BufferUsages::STORAGE, 96, 48)
                .unwrap_err()
                .contains("element stride")
        );
    }

    #[test]
    fn rejects_invalid_array_lengths_usage_and_device_limit() {
        for size in [0, 4, 15, 20] {
            assert!(validate_storage_buffer(size, BufferUsages::STORAGE, 64).is_err());
        }
        assert!(validate_storage_buffer(16, BufferUsages::COPY_DST, 64).is_err());
        assert!(validate_storage_buffer(80, BufferUsages::STORAGE, 64).is_err());
        for size in [16, 32, 64] {
            assert!(validate_storage_buffer(size, BufferUsages::STORAGE, 64).is_ok());
        }
    }
}
