//! Shared ShaderMaterial types for PyBevy.
//!
//! This crate contains the Bevy-side `ShaderMaterial` type definition
//! (`ExtendedMaterial<StandardMaterial, ShaderMaterialExtension>`) and
//! supporting types. No Python dependencies.

mod shader_def_registry;

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::{OnceLock, RwLock},
};

use bevy::{
    asset::{Asset, AssetId, AssetLoadFailedEvent, AssetServer, Assets, Handle},
    ecs::{
        message::MessageReader,
        system::{Res, ResMut},
    },
    image::Image,
    math::Vec4,
    mesh::MeshVertexBufferLayoutRef,
    pbr::{
        ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
        StandardMaterial,
    },
    reflect::Reflect,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
    },
    shader::{Shader, ShaderDefVal, ShaderImport, ShaderRef},
};
use shader_def_registry::{
    clear_shader_def_names, get_shader_def_names, register_shader_def_names,
};
use tracing::error;

static SHADER_REGISTRY: OnceLock<RwLock<HashMap<u64, Handle<Shader>>>> = OnceLock::new();
/// Retains generated shader modules so their assets remain available for imports.
static BINDINGS_REGISTRY: OnceLock<RwLock<BindingsRegistry>> = OnceLock::new();

#[derive(Clone)]
struct GeneratedBindingsShader {
    logical_type_id: u64,
    source: String,
    handle: Handle<Shader>,
}

type BindingsRegistry = HashMap<ShaderImport, GeneratedBindingsShader>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ShaderStage {
    Fragment,
    Vertex,
}

impl ShaderStage {
    fn label(self) -> &'static str {
        match self {
            Self::Fragment => "fragment",
            Self::Vertex => "vertex",
        }
    }
}

fn registry() -> &'static RwLock<HashMap<u64, Handle<Shader>>> {
    SHADER_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn hash_shader_path(path: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}

fn get_shader_handle(id: u64) -> Option<Handle<Shader>> {
    registry().read().ok()?.get(&id).cloned()
}

fn register_shader_handle(id: u64, handle: Handle<Shader>) {
    if let Ok(mut map) = registry().write() {
        map.insert(id, handle);
    }
}

fn bindings_registry() -> &'static RwLock<BindingsRegistry> {
    BINDINGS_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

fn sync_generated_bindings_shader(
    registry: &mut BindingsRegistry,
    shaders: &mut Assets<Shader>,
    logical_type_id: u64,
    wgsl: &str,
) {
    let source_hash = hash_shader_path(wgsl);
    let shader = Shader::from_wgsl(wgsl.to_owned(), format!("pybevy://gen/{source_hash}"));
    let import_path = shader.import_path.clone();

    if let Some(entry) = registry.get_mut(&import_path) {
        if entry.source == wgsl {
            entry.logical_type_id = entry.logical_type_id.max(logical_type_id);
            return;
        }
        // Assets created by an older material class can survive a reload. They
        // must not restore their layout after the newer class replaces it.
        if logical_type_id <= entry.logical_type_id {
            return;
        }

        if shaders.insert(entry.handle.id(), shader.clone()).is_ok() {
            entry.logical_type_id = logical_type_id;
            entry.source = wgsl.to_owned();
            return;
        }
    }

    let handle = shaders.add(shader);
    registry.insert(
        import_path,
        GeneratedBindingsShader {
            logical_type_id,
            source: wgsl.to_owned(),
            handle,
        },
    );
}

/// Clear all cached shader handles and def names.
/// Called on plugin build to avoid stale handles from previous app runs.
pub fn clear_shader_registries() {
    if let Some(reg) = SHADER_REGISTRY.get()
        && let Ok(mut map) = reg.write()
    {
        map.clear();
    }
    clear_shader_def_names();
    if let Some(reg) = BINDINGS_REGISTRY.get()
        && let Ok(mut map) = reg.write()
    {
        map.clear();
    }
}

fn sync_shader_path(path: &str, asset_server: &AssetServer) -> u64 {
    let id = hash_shader_path(path);
    if get_shader_handle(id).is_none() {
        let handle = asset_server.load(path.to_owned());
        register_shader_handle(id, handle);
    }
    id
}

fn stage_uses_shader(
    materials: &Assets<ShaderMaterial>,
    stage: ShaderStage,
    shader_id: AssetId<Shader>,
) -> bool {
    materials.iter().any(|(_, material)| {
        let path = match stage {
            ShaderStage::Fragment => &material.extension.fragment_shader_path,
            ShaderStage::Vertex => &material.extension.vertex_shader_path,
        };
        path.as_deref()
            .and_then(|path| get_shader_handle(hash_shader_path(path)))
            .is_some_and(|handle| handle.id() == shader_id)
    })
}

// Shader params uniform buffer
//
// 64 × vec4 = 256 floats = 1024 bytes.
// Python's @material decorator packs typed fields into this buffer and
// generates a matching WGSL struct at the same binding.
//
// WGSL: @group(3) @binding(100) var<uniform> material: YourStruct;

#[derive(ShaderType, Reflect, Clone, Debug)]
pub struct ShaderParams {
    pub data: [Vec4; 64],
}

impl Default for ShaderParams {
    fn default() -> Self {
        Self {
            data: [Vec4::ZERO; 64],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct ShaderMaterialKey {
    pub frag_shader_id: u64,
    pub vert_shader_id: u64,
    pub shader_defs: u32,
    pub def_names_id: u64,
}

impl From<&ShaderMaterialExtension> for ShaderMaterialKey {
    fn from(ext: &ShaderMaterialExtension) -> Self {
        Self {
            frag_shader_id: ext
                .fragment_shader_path
                .as_ref()
                .map(|p| hash_shader_path(p))
                .unwrap_or(0),
            vert_shader_id: ext
                .vertex_shader_path
                .as_ref()
                .map(|p| hash_shader_path(p))
                .unwrap_or(0),
            shader_defs: ext.shader_defs,
            def_names_id: ext.logical_type_id.unwrap_or(0),
        }
    }
}

/// Maximum number of texture slots available to @material classes.
pub const MAX_TEXTURE_SLOTS: usize = 4;

/// Bevy pushes this for every prepass and shadow pipeline it specializes.
const PREPASS_PIPELINE_DEF: &str = "PREPASS_PIPELINE";

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
#[bind_group_data(ShaderMaterialKey)]
pub struct ShaderMaterialExtension {
    #[uniform(100)]
    pub params: ShaderParams,

    // Texture slots: 4 × (texture + sampler) at bindings 101-108
    #[texture(101, dimension = "2d")]
    #[sampler(102)]
    pub texture_0: Option<Handle<Image>>,

    #[texture(103, dimension = "2d")]
    #[sampler(104)]
    pub texture_1: Option<Handle<Image>>,

    #[texture(105, dimension = "2d")]
    #[sampler(106)]
    pub texture_2: Option<Handle<Image>>,

    #[texture(107, dimension = "2d")]
    #[sampler(108)]
    pub texture_3: Option<Handle<Image>>,

    // Non-binding fields — used for pipeline specialization only.
    // Fields without #[uniform]/#[texture]/#[sampler] are ignored by AsBindGroup.
    #[reflect(ignore)]
    pub fragment_shader_path: Option<String>,
    #[reflect(ignore)]
    pub vertex_shader_path: Option<String>,

    /// Bitmask of active shader defs (up to 32 bool fields).
    #[reflect(ignore)]
    pub shader_defs: u32,
    /// Names of bool fields that map to shader def bits (bit 0 = names[0], etc.).
    #[reflect(ignore)]
    pub shader_def_names: Vec<String>,

    /// Process-local identity of the Python `@material` class that created this asset.
    /// This is metadata only and is deliberately excluded from GPU bindings/reflection.
    #[reflect(ignore)]
    pub logical_type_id: Option<u64>,

    /// In-memory WGSL source for generated binding declarations.
    /// Contains `#define_import_path` so user shaders can `#import` it.
    /// Injected into `Assets<Shader>` by `sync_shader_handles`.
    #[reflect(ignore)]
    pub bindings_wgsl: Option<String>,
}

impl MaterialExtension for ShaderMaterialExtension {
    fn fragment_shader() -> ShaderRef {
        // Default PBR — overridden per-instance in specialize()
        ShaderRef::Default
    }

    fn vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Bevy calls specialize for prepass and shadow pipelines too. A forward
        // shader swapped in there mismatches the prepass vertex stage, so leave
        // those pipelines on bevy's defaults as an unextended material would.
        if descriptor
            .vertex
            .shader_defs
            .iter()
            .any(|def| matches!(def, ShaderDefVal::Bool(name, _) if name == PREPASS_PIPELINE_DEF))
        {
            return Ok(());
        }
        if key.bind_group_data.frag_shader_id != 0
            && let Some(handle) = get_shader_handle(key.bind_group_data.frag_shader_id)
            && let Some(fragment) = &mut descriptor.fragment
        {
            fragment.shader = handle;
        }
        if key.bind_group_data.vert_shader_id != 0
            && let Some(handle) = get_shader_handle(key.bind_group_data.vert_shader_id)
        {
            descriptor.vertex.shader = handle;
        }

        // Push shader defs from bool fields
        if key.bind_group_data.shader_defs != 0
            && let Some(names) = get_shader_def_names(key.bind_group_data.def_names_id)
        {
            for (i, name) in names.iter().take(u32::BITS as usize).enumerate() {
                if key.bind_group_data.shader_defs & (1 << i) != 0 {
                    if let Some(fragment) = &mut descriptor.fragment {
                        fragment
                            .shader_defs
                            .push(ShaderDefVal::Bool(name.clone(), true));
                    }
                    descriptor
                        .vertex
                        .shader_defs
                        .push(ShaderDefVal::Bool(name.clone(), true));
                }
            }
        }

        Ok(())
    }
}

/// The complete material type: StandardMaterial + ShaderMaterialExtension.
pub type ShaderMaterial = ExtendedMaterial<StandardMaterial, ShaderMaterialExtension>;

// Shader sync system
//
// Runs in `Last` schedule — after all user Update systems, before Render
// extraction. Scans all ShaderMaterial assets for shader paths and loads
// them into the global registry so specialize() can find them.

pub fn sync_shader_handles(
    materials: Res<Assets<ShaderMaterial>>,
    asset_server: Res<AssetServer>,
    mut shaders: ResMut<Assets<Shader>>,
    mut load_failures: MessageReader<AssetLoadFailedEvent<Shader>>,
) {
    for failure in load_failures.read() {
        for stage in [ShaderStage::Fragment, ShaderStage::Vertex] {
            if stage_uses_shader(&materials, stage, failure.id) {
                error!(
                    "Custom {} shader '{}' failed to load: {}",
                    stage.label(),
                    failure.path,
                    failure.error
                );
            }
        }
    }

    for (_, material) in materials.iter() {
        if let Some(path) = &material.extension.fragment_shader_path {
            sync_shader_path(path, &asset_server);
        }
        if let Some(path) = &material.extension.vertex_shader_path {
            sync_shader_path(path, &asset_server);
        }
        if let Some(type_id) = material.extension.logical_type_id
            && !material.extension.shader_def_names.is_empty()
        {
            register_shader_def_names(type_id, material.extension.shader_def_names.clone());
        }

        // Inject in-memory binding declarations into Assets<Shader>.
        // The WGSL contains #define_import_path so user shaders can #import it.
        if let Some(wgsl) = &material.extension.bindings_wgsl
            && let Ok(mut registry) = bindings_registry().write()
        {
            sync_generated_bindings_shader(
                &mut registry,
                &mut shaders,
                material.extension.logical_type_id.unwrap_or(0),
                wgsl,
            );
        }
    }
}
