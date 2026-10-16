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
    image::{Image, ImageSampler},
    material::{AlphaMode, OpaqueRendererMethod},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{
        Material, MaterialBindGroupAllocators, MaterialExtension, MaterialExtensionKey,
        MaterialExtensionPipeline, MaterialPipeline, MaterialPipelineKey, MaterialPlugin,
        MeshMaterial3d, RenderMaterialBindings, StandardMaterial,
    },
    reflect::Reflect,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        erased_render_asset::{AssetExtractionSystems, ExtractedAssets, prepare_erased_assets},
        render_asset::{RenderAssets, prepare_assets},
        render_resource::{
            AsBindGroup, AsBindGroupError, BindGroupEntry, BindGroupLayout,
            BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferBindingType,
            BufferId, BufferInitDescriptor, BufferSize, BufferUsages,
            DefaultImageSamplerDescriptor, DownlevelFlags, OwnedBindingResource, PipelineCache,
            PreparedBindGroup, RenderPipelineDescriptor, SamplerId, ShaderStages,
            SpecializedMeshPipelineError, TextureViewId, UnpreparedBindGroup,
        },
        renderer::{RenderAdapter, RenderDevice},
        settings::WgpuLimits,
        storage::{GpuShaderBuffer, ShaderBuffer},
        texture::GpuImage,
    },
    shader::ShaderRef,
};

use crate::{
    ShaderMaterial, ShaderMaterialExtension, ShaderMaterialKey, clear_shader_registries,
    storage_element::StorageElement, sync_shader_handles, texture_binding::TextureBindingLayout,
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
    pub texture_layouts: Vec<Option<TextureBindingLayout>>,
}

impl Default for StorageShaderMaterialExtension {
    fn default() -> Self {
        Self {
            inner: ShaderMaterialExtension::default(),
            buffers: vec![StorageBinding::default()],
            texture_layouts: Vec::new(),
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
            "StorageShaderMaterialPlugin requires material group 3 and binding indices below {}; highest requested binding is {}",
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
                "StorageShaderMaterialPlugin combined material layout exceeds device limits for {stage:?}: required {counts:?}, available {maxima:?} (storage, uniform, texture, sampler)"
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
        SRes<RenderAssets<GpuShaderBuffer>>,
        SRes<StorageBufferAvailability>,
        SRes<RenderAssets<GpuImage>>,
        SRes<DefaultImageSamplerDescriptor>,
    );

    fn label() -> &'static str {
        "pybevy_storage_material"
    }

    fn bind_group_data(&self) -> Self::Data {
        StorageMaterialKey {
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

    fn unprepared_bind_group(
        &self,
        _: &BindGroupLayout,
        _: &RenderDevice,
        _: &mut SystemParamItem<Self::Param>,
        _: bool,
    ) -> Result<UnpreparedBindGroup, AsBindGroupError> {
        Err(AsBindGroupError::CreateBindGroupDirectly)
    }

    fn as_bind_group(
        &self,
        descriptor: &BindGroupLayoutDescriptor,
        device: &RenderDevice,
        cache: &PipelineCache,
        param: &mut SystemParamItem<Self::Param>,
    ) -> Result<PreparedBindGroup, AsBindGroupError> {
        if self.extension.texture_layouts.len() > 4
            || (param.3.unsupported && !self.extension.buffers.is_empty())
        {
            return Err(AsBindGroupError::RetryNextUpdate);
        }
        let mut descriptor = descriptor.clone();
        apply_texture_layouts(&mut descriptor.entries, &self.extension.texture_layouts);
        descriptor
            .entries
            .extend(storage_entries(&self.bind_group_data().strides));
        if storage_layout_error(device, &descriptor.entries).is_some() {
            return Err(AsBindGroupError::RetryNextUpdate);
        }
        let mut storage = Vec::new();
        for (slot, binding) in self.extension.buffers.iter().enumerate() {
            let handle = binding
                .handle
                .as_ref()
                .filter(|handle| param.3.live.contains(&handle.id()))
                .ok_or(AsBindGroupError::RetryNextUpdate)?;
            let buffer = param
                .2
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
                OwnedBindingResource::Buffer(buffer.buffer.clone()),
            ));
        }
        let mut inner = self.extension.inner.clone();
        let mut textures = Vec::new();
        for (slot, metadata) in self.extension.texture_layouts.iter().enumerate() {
            let Some(metadata) = metadata else {
                continue;
            };
            let handle = inner
                .texture_slot(slot)
                .and_then(Option::as_ref)
                .ok_or(AsBindGroupError::RetryNextUpdate)?;
            let sampler = param
                .3
                .images
                .get(&handle.id())
                .ok_or(AsBindGroupError::RetryNextUpdate)?;
            let image = param
                .4
                .get(handle)
                .ok_or(AsBindGroupError::RetryNextUpdate)?;
            let sampler = match sampler {
                ImageSampler::Default => &param.5.0,
                ImageSampler::Descriptor(value) => value,
            };
            metadata
                .validate(image, sampler, device)
                .map_err(|_| AsBindGroupError::RetryNextUpdate)?;
            textures.push((
                101 + slot as u32 * 2,
                OwnedBindingResource::TextureView(
                    metadata.view_dimension(),
                    image.texture_view.clone(),
                ),
            ));
            textures.push((
                102 + slot as u32 * 2,
                OwnedBindingResource::Sampler(metadata.sampler_type(), image.sampler.clone()),
            ));
            // The derive-generated ordinary path validates all textures as float 2D.
            // Let it supply uniform/fallback resources, then replace custom slots.
            *inner.texture_slot_mut(slot).unwrap() = None;
        }
        let layout = cache.get_bind_group_layout(&descriptor);
        let mut bindings = self
            .base
            .unprepared_bind_group(&layout, device, &mut param.0, true)?
            .bindings;
        bindings.extend(
            inner
                .unprepared_bind_group(&layout, device, &mut param.1, true)?
                .bindings
                .0,
        );
        for (index, resource) in textures {
            *bindings
                .iter_mut()
                .find(|(binding, _)| *binding == index)
                .unwrap() = (index, resource);
        }
        bindings.extend(storage);
        // Direct preparation must perform the uniform packing normally done by the allocator.
        for (_, resource) in bindings.iter_mut() {
            if let OwnedBindingResource::Data(data) = resource {
                *resource = OwnedBindingResource::Buffer(device.create_buffer_with_data(
                    &BufferInitDescriptor {
                        label: Some(Self::label()),
                        contents: &data.0,
                        usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
                    },
                ));
            }
        }
        let entries: Vec<_> = bindings
            .iter()
            .map(|(binding, resource)| BindGroupEntry {
                binding: *binding,
                resource: resource.get_binding(),
            })
            .collect();
        let bind_group = device.create_bind_group(Self::label(), &layout, &entries);
        Ok(PreparedBindGroup {
            bindings,
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
        Ok(())
    }
}

#[derive(Default)]
pub struct StorageShaderMaterialPlugin;

impl Plugin for StorageShaderMaterialPlugin {
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
            let error = if !flags.contains(required) {
                Some("StorageShaderMaterialPlugin requires vertex and fragment storage-buffer support".to_owned())
            } else {
                None
            };
            if let Some(error) = error {
                tracing::error!("{error}; storage materials will not render");
                render_app
                    .world_mut()
                    .resource_mut::<StorageBufferAvailability>()
                    .unsupported = true;
            }
        }
    }
}

fn apply_texture_layouts(
    entries: &mut [BindGroupLayoutEntry],
    layouts: &[Option<TextureBindingLayout>],
) {
    for (slot, layout) in layouts.iter().enumerate() {
        let Some(layout) = layout else {
            continue;
        };
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
    textures: Vec<(TextureViewId, SamplerId)>,
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
            dependencies.0.insert(
                *id,
                MaterialDependency {
                    material: material.clone(),
                    allocation: None,
                    diagnostic: None,
                },
            );
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
    device: Res<RenderDevice>,
    availability: Res<StorageBufferAvailability>,
    mut dependencies: ResMut<StorageMaterialDependencies>,
    mut extracted: ResMut<ExtractedAssets<MeshMaterial3d<StorageShaderMaterial>>>,
    mut bindings: ResMut<RenderMaterialBindings>,
    mut allocators: ResMut<MaterialBindGroupAllocators>,
) {
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
            .map(|binding| {
                let handle = binding
                    .handle
                    .as_ref()
                    .filter(|handle| availability.live.contains(&handle.id()))?;
                let buffer = buffers.get(handle)?;
                if let Err(error) = validate_storage_element_buffer(
                    buffer.buffer.size(),
                    buffer.buffer.usage(),
                    device.limits().max_storage_buffer_binding_size,
                    binding.element.stride,
                ) {
                    diagnostic = Some(error);
                    return None;
                }
                Some(buffer.buffer.id())
            })
            .collect();
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
                    Some((image.texture_view.id(), image.sampler.id()))
                })())
            })
            .collect();
        let allocation = allocation
            .zip(textures)
            .map(|(buffers, textures)| MaterialAllocation { buffers, textures });
        let mut entries = StorageShaderMaterial::bind_group_layout_entries(&device, true);
        entries.extend(storage_entries(
            &dependency.material.bind_group_data().strides,
        ));
        apply_texture_layouts(&mut entries, &dependency.material.extension.texture_layouts);
        diagnostic = diagnostic.or_else(|| storage_layout_error(&device, &entries));
        if diagnostic != dependency.diagnostic {
            if let Some(error) = &diagnostic {
                tracing::error!("Storage material {id:?}: {error}");
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
    // Bevy 0.19's direct-preparation path allocates a fresh slot without retiring
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
