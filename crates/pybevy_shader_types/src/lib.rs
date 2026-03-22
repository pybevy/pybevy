//! Shared ShaderMaterial types for PyBevy.
//!
//! This crate contains the Bevy-side `ShaderMaterial` type definition
//! (`ExtendedMaterial<StandardMaterial, ShaderMaterialExtension>`) and
//! supporting types. No Python dependencies.
//!
//! Used by:
//! - `pybevy_pbr` — Python wrappers + plugin registration
//! - `pybevy_replicon` — server-side extraction for browser sync
//! - `pybevy_browser` — client-side material reconstruction

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::{OnceLock, RwLock},
};

use bevy::{
    asset::{Asset, Assets, Handle},
    ecs::system::{Res, ResMut},
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
    shader::{Shader, ShaderRef},
};

// ── Global shader handle registry ────────────────────────────────────

static SHADER_REGISTRY: OnceLock<RwLock<HashMap<u64, Handle<Shader>>>> = OnceLock::new();
static SHADER_DEF_REGISTRY: OnceLock<RwLock<HashMap<u64, Vec<String>>>> = OnceLock::new();
/// Tracks which bindings WGSL sources have been injected (by hash of content).
static BINDINGS_INJECTED: OnceLock<RwLock<std::collections::HashSet<u64>>> = OnceLock::new();

fn registry() -> &'static RwLock<HashMap<u64, Handle<Shader>>> {
    SHADER_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

fn def_registry() -> &'static RwLock<HashMap<u64, Vec<String>>> {
    SHADER_DEF_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
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

fn get_shader_def_names(shader_id: u64) -> Option<Vec<String>> {
    def_registry().read().ok()?.get(&shader_id).cloned()
}

fn register_shader_def_names(shader_id: u64, names: Vec<String>) {
    if let Ok(mut map) = def_registry().write() {
        map.insert(shader_id, names);
    }
}

fn bindings_set() -> &'static RwLock<std::collections::HashSet<u64>> {
    BINDINGS_INJECTED.get_or_init(|| RwLock::new(std::collections::HashSet::new()))
}

/// Clear all cached shader handles and def names.
/// Called on plugin build to avoid stale handles from previous app runs.
pub fn clear_shader_registries() {
    if let Some(reg) = SHADER_REGISTRY.get() {
        if let Ok(mut map) = reg.write() {
            map.clear();
        }
    }
    if let Some(reg) = SHADER_DEF_REGISTRY.get() {
        if let Ok(mut map) = reg.write() {
            map.clear();
        }
    }
    if let Some(reg) = BINDINGS_INJECTED.get() {
        if let Ok(mut set) = reg.write() {
            set.clear();
        }
    }
}

// ── Shader params uniform buffer ─────────────────────────────────────
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

// ── Pipeline specialization key ──────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ShaderMaterialKey {
    pub frag_shader_id: u64,
    pub vert_shader_id: u64,
    pub shader_defs: u32,
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
        }
    }
}

// ── Material extension ───────────────────────────────────────────────

/// Maximum number of texture slots available to @material classes.
pub const MAX_TEXTURE_SLOTS: usize = 4;

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
        if key.bind_group_data.frag_shader_id != 0 {
            if let Some(handle) = get_shader_handle(key.bind_group_data.frag_shader_id) {
                if let Some(fragment) = &mut descriptor.fragment {
                    fragment.shader = handle;
                }
            }
        }
        if key.bind_group_data.vert_shader_id != 0 {
            if let Some(handle) = get_shader_handle(key.bind_group_data.vert_shader_id) {
                descriptor.vertex.shader = handle;
            }
        }

        // Push shader defs from bool fields
        if key.bind_group_data.shader_defs != 0 {
            if let Some(names) = get_shader_def_names(key.bind_group_data.frag_shader_id) {
                for (i, name) in names.iter().enumerate() {
                    if key.bind_group_data.shader_defs & (1 << i) != 0 {
                        if let Some(fragment) = &mut descriptor.fragment {
                            fragment
                                .shader_defs
                                .push(bevy::shader::ShaderDefVal::Bool(name.clone(), true));
                        }
                        descriptor
                            .vertex
                            .shader_defs
                            .push(bevy::shader::ShaderDefVal::Bool(name.clone(), true));
                    }
                }
            }
        }

        Ok(())
    }
}

/// The complete material type: StandardMaterial + ShaderMaterialExtension.
pub type ShaderMaterial = ExtendedMaterial<StandardMaterial, ShaderMaterialExtension>;

// ── Shader sync system ───────────────────────────────────────────────
//
// Runs in `Last` schedule — after all user Update systems, before Render
// extraction. Scans all ShaderMaterial assets for shader paths and loads
// them into the global registry so specialize() can find them.

pub fn sync_shader_handles(
    materials: Res<Assets<ShaderMaterial>>,
    asset_server: Res<bevy::asset::AssetServer>,
    mut shaders: ResMut<Assets<Shader>>,
) {
    for (_, material) in materials.iter() {
        if let Some(path) = &material.extension.fragment_shader_path {
            let id = hash_shader_path(path);
            if get_shader_handle(id).is_none() {
                let handle: Handle<Shader> = asset_server.load(path.clone());
                register_shader_handle(id, handle);
            }
            // Register shader def names for this shader (idempotent)
            if !material.extension.shader_def_names.is_empty() && get_shader_def_names(id).is_none()
            {
                register_shader_def_names(id, material.extension.shader_def_names.clone());
            }
        }
        if let Some(path) = &material.extension.vertex_shader_path {
            let id = hash_shader_path(path);
            if get_shader_handle(id).is_none() {
                let handle: Handle<Shader> = asset_server.load(path.clone());
                register_shader_handle(id, handle);
            }
        }

        // Inject in-memory binding declarations into Assets<Shader>.
        // The WGSL contains #define_import_path so user shaders can #import it.
        if let Some(wgsl) = &material.extension.bindings_wgsl {
            let id = hash_shader_path(wgsl);
            let already = bindings_set()
                .read()
                .map(|s| s.contains(&id))
                .unwrap_or(false);
            if !already {
                let shader = Shader::from_wgsl(wgsl.clone(), format!("pybevy://gen/{}", id));
                shaders.add(shader);
                if let Ok(mut set) = bindings_set().write() {
                    set.insert(id);
                }
            }
        }
    }
}
