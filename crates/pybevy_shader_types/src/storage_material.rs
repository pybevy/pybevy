//! Opt-in read-only storage binding. Ordinary material layouts are unchanged.

use std::{
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
    mesh::MeshVertexBufferLayoutRef,
    pbr::{
        ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
        MaterialPlugin, MeshMaterial3d, StandardMaterial,
    },
    reflect::Reflect,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        erased_render_asset::{AssetExtractionSystems, ExtractedAssets, prepare_erased_assets},
        render_asset::{RenderAssets, prepare_assets},
        render_resource::{
            AsBindGroup, AsBindGroupError, BindGroupLayout, BindGroupLayoutEntry, BindingType,
            BufferBindingType, BufferId, BufferSize, BufferUsages, DownlevelFlags,
            OwnedBindingResource, RenderPipelineDescriptor, ShaderStages,
            SpecializedMeshPipelineError, UnpreparedBindGroup,
        },
        renderer::{RenderAdapter, RenderDevice},
        storage::{GpuShaderBuffer, ShaderBuffer},
    },
};

use crate::{
    ShaderMaterial, ShaderMaterialExtension, ShaderMaterialKey, clear_shader_registries,
    sync_shader_handles,
};

pub const STORAGE_BINDING: u32 = 109;

#[derive(Asset, Reflect, Debug, Clone, Default)]
pub struct StorageShaderMaterialExtension {
    pub inner: ShaderMaterialExtension,
    pub storage_buffer: Option<Handle<ShaderBuffer>>,
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

pub type StorageShaderMaterial = ExtendedMaterial<StandardMaterial, StorageShaderMaterialExtension>;

/// Checked before binding; an absent or invalid buffer must not produce a draw.
pub fn validate_storage_buffer(
    size: u64,
    usage: BufferUsages,
    max_size: u64,
) -> Result<(), String> {
    if size == 0 || !size.is_multiple_of(16) {
        return Err(format!(
            "Material storage buffer must contain a nonempty array<vec4<f32>> (byte size divisible by 16); got {size} bytes"
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

fn storage_layout_error(
    device: &RenderDevice,
    extension: &[BindGroupLayoutEntry],
) -> Option<String> {
    let mut entries = StandardMaterial::bind_group_layout_entries(device, true);
    entries.extend_from_slice(extension);
    let limits = device.limits();
    if limits.max_bind_groups < 4
        || entries
            .iter()
            .any(|entry| entry.binding >= limits.max_bindings_per_bind_group)
    {
        return Some(
            "StorageShaderMaterialPlugin requires material group 3 with binding 109".into(),
        );
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

fn storage_entries(device: &RenderDevice) -> Vec<BindGroupLayoutEntry> {
    let mut entries = ShaderMaterialExtension::bind_group_layout_entries(device, true);
    entries.push(BindGroupLayoutEntry {
        binding: STORAGE_BINDING,
        visibility: ShaderStages::VERTEX_FRAGMENT,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: BufferSize::new(16),
        },
        count: None,
    });
    entries
}

#[derive(Resource, Default)]
pub struct StorageBufferAvailability {
    live: HashSet<AssetId<ShaderBuffer>>,
    unsupported: bool,
}

impl AsBindGroup for StorageShaderMaterialExtension {
    type Data = ShaderMaterialKey;
    type Param = (
        <ShaderMaterialExtension as AsBindGroup>::Param,
        SRes<RenderAssets<GpuShaderBuffer>>,
        SRes<StorageBufferAvailability>,
    );

    fn label() -> &'static str {
        "pybevy_storage_material"
    }
    fn bind_group_data(&self) -> Self::Data {
        ShaderMaterialKey::from(&self.inner)
    }

    fn unprepared_bind_group(
        &self,
        layout: &BindGroupLayout,
        device: &RenderDevice,
        param: &mut SystemParamItem<Self::Param>,
        _: bool,
    ) -> Result<UnpreparedBindGroup, AsBindGroupError> {
        let handle = self
            .storage_buffer
            .as_ref()
            .ok_or(AsBindGroupError::RetryNextUpdate)?;
        if param.2.unsupported || !param.2.live.contains(&handle.id()) {
            return Err(AsBindGroupError::RetryNextUpdate);
        }
        let buffer = param
            .1
            .get(handle)
            .ok_or(AsBindGroupError::RetryNextUpdate)?;
        validate_storage_buffer(
            buffer.buffer.size(),
            buffer.buffer.usage(),
            device.limits().max_storage_buffer_binding_size,
        )
        .map_err(|_| AsBindGroupError::RetryNextUpdate)?;
        let mut group = self
            .inner
            .unprepared_bind_group(layout, device, &mut param.0, true)?;
        group.bindings.push((
            STORAGE_BINDING,
            OwnedBindingResource::Buffer(buffer.buffer.clone()),
        ));
        Ok(group)
    }

    fn bind_group_layout_entries(device: &RenderDevice, _: bool) -> Vec<BindGroupLayoutEntry> {
        let entries = storage_entries(device);
        if storage_layout_error(device, &entries).is_some() {
            // The plugin reports unsupported capabilities and prevents preparation.
            return Vec::new();
        }
        entries
    }
}

impl MaterialExtension for StorageShaderMaterialExtension {
    fn specialize(
        pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        ShaderMaterialExtension::specialize(
            pipeline,
            descriptor,
            layout,
            MaterialExtensionKey {
                mesh_key: key.mesh_key,
                bind_group_data: key.bind_group_data,
            },
        )
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
                        .before(prepare_erased_assets::<MeshMaterial3d<StorageShaderMaterial>>),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            let device = render_app.world().resource::<RenderDevice>();
            let flags = render_app
                .world()
                .resource::<RenderAdapter>()
                .get_downlevel_capabilities()
                .flags;
            let required = DownlevelFlags::VERTEX_STORAGE | DownlevelFlags::FRAGMENT_STORAGE;
            let error = if !flags.contains(required) {
                Some("StorageShaderMaterialPlugin requires vertex and fragment storage-buffer support".to_owned())
            } else {
                storage_layout_error(device, &storage_entries(device))
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

struct MaterialDependency {
    material: StorageShaderMaterial,
    allocation: Option<BufferId>,
    diagnostic: Option<String>,
}

#[derive(Resource, Default)]
struct StorageMaterialDependencies(HashMap<AssetId<StorageShaderMaterial>, MaterialDependency>);

fn extract_storage_dependencies(
    buffers: Extract<Res<Assets<ShaderBuffer>>>,
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
}

fn refresh_storage_bindings(
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    device: Res<RenderDevice>,
    availability: Res<StorageBufferAvailability>,
    mut dependencies: ResMut<StorageMaterialDependencies>,
    mut extracted: ResMut<ExtractedAssets<MeshMaterial3d<StorageShaderMaterial>>>,
) {
    if availability.unsupported {
        // Cancel inputs and retries before Bevy resolves an unsupported GPU layout.
        extracted.removed.extend(dependencies.0.keys().copied());
        extracted.extracted.clear();
        extracted.added.clear();
        extracted.modified.clear();
        return;
    }
    let fresh: HashSet<_> = extracted.extracted.iter().map(|(id, _)| *id).collect();
    for (id, dependency) in &mut dependencies.0 {
        let buffer = dependency
            .material
            .extension
            .storage_buffer
            .as_ref()
            .filter(|handle| availability.live.contains(&handle.id()))
            .and_then(|handle| buffers.get(handle));
        let diagnostic = buffer.and_then(|buffer| {
            validate_storage_buffer(
                buffer.buffer.size(),
                buffer.buffer.usage(),
                device.limits().max_storage_buffer_binding_size,
            )
            .err()
        });
        if diagnostic != dependency.diagnostic {
            if let Some(error) = &diagnostic {
                tracing::error!("Storage material {id:?}: {error}");
            }
            dependency.diagnostic = diagnostic;
        }
        let allocation = buffer
            .filter(|_| dependency.diagnostic.is_none() && !availability.unsupported)
            .map(|buffer| buffer.buffer.id());
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
}
