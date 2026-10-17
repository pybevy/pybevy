//! Shared ShaderMaterial types for PyBevy.
//!
//! This crate contains the Bevy-side `ShaderMaterial` type definition
//! (`ExtendedMaterial<StandardMaterial, ShaderMaterialExtension>`) and
//! supporting types. No Python dependencies.

mod generated_bindings;
pub mod material_shader_variants;
pub mod packed_uniform;
pub mod shader_def_registry;
pub mod storage_element;
pub mod storage_material;
pub mod texture_binding;
pub use material_shader_variants::material_shader_path_id;
use material_shader_variants::{
    MaterialShaderLifetimes, MaterialShaderSources, MaterialShaderVariants, material_shader_handle,
};
pub use storage_material::{
    ShaderMaterialPlugin, StorageShaderMaterial, StorageShaderMaterialExtension,
    StorageShaderMaterialPlugin,
};

use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
    sync::{OnceLock, RwLock},
};

use bevy::{
    asset::{
        Asset, AssetEvent, AssetId, AssetLoadFailedEvent, AssetPath, AssetServer, Assets, Handle,
    },
    ecs::{
        message::{MessageCursor, MessageReader, Messages},
        system::{Local, Res, ResMut},
    },
    image::Image,
    mesh::MeshVertexBufferLayoutRef,
    pbr::{
        ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
        StandardMaterial,
    },
    reflect::Reflect,
    render::render_resource::{
        AsBindGroup, FragmentState, RenderPipelineDescriptor, ShaderType,
        SpecializedMeshPipelineError,
    },
    shader::{Shader, ShaderDefVal, ShaderImport, ShaderRef},
};
use generated_bindings::{BindingsRegistry, binding_definition, sync_generated_bindings};
use tracing::error;

static SHADER_REGISTRY: OnceLock<RwLock<HashMap<u64, Handle<Shader>>>> = OnceLock::new();
/// Retains generated shader modules so their assets remain available for imports.
static BINDINGS_REGISTRY: OnceLock<RwLock<BindingsRegistry>> = OnceLock::new();
const GENERATED_MATERIAL_IMPORT_PREFIX: &str = "pybevy::gen::";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ShaderStage {
    Fragment,
    Vertex,
    PrepassVertex,
    PrepassFragment,
    DeferredVertex,
    DeferredFragment,
}

impl ShaderStage {
    const ALL: [Self; 6] = [
        Self::Fragment,
        Self::Vertex,
        Self::PrepassVertex,
        Self::PrepassFragment,
        Self::DeferredVertex,
        Self::DeferredFragment,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Fragment => "fragment",
            Self::Vertex => "vertex",
            Self::PrepassVertex => "prepass vertex",
            Self::PrepassFragment => "prepass fragment",
            Self::DeferredVertex => "deferred vertex",
            Self::DeferredFragment => "deferred fragment",
        }
    }

    fn path(self, extension: &ShaderMaterialExtension) -> Option<&str> {
        match self {
            Self::Fragment => &extension.fragment_shader_path,
            Self::Vertex => &extension.vertex_shader_path,
            Self::PrepassVertex => &extension.prepass_vertex_shader_path,
            Self::PrepassFragment => &extension.prepass_fragment_shader_path,
            Self::DeferredVertex => &extension.deferred_vertex_shader_path,
            Self::DeferredFragment => &extension.deferred_fragment_shader_path,
        }
        .as_deref()
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

fn registered_path_id(id: AssetId<Shader>) -> Option<u64> {
    registry()
        .read()
        .ok()?
        .iter()
        .find_map(|(path_id, handle)| (handle.id() == id).then_some(*path_id))
}

fn register_shader_handle(id: u64, handle: Handle<Shader>) {
    if let Ok(mut map) = registry().write() {
        map.insert(id, handle);
    }
}

fn bindings_registry() -> &'static RwLock<BindingsRegistry> {
    BINDINGS_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Clear all cached shader handles and def names.
/// Called on plugin build to avoid stale handles from previous app runs.
pub fn clear_shader_registries() {
    shader_def_registry::clear_names();
    if let Some(reg) = SHADER_REGISTRY.get()
        && let Ok(mut map) = reg.write()
    {
        map.clear();
    }
    if let Some(reg) = BINDINGS_REGISTRY.get()
        && let Ok(mut map) = reg.write()
    {
        map.clear();
    }
}

/// Custom material imports and generated bindings currently use the WGSL composer.
pub fn validate_material_shader_path(path: &str) -> Result<(), String> {
    let asset_path = AssetPath::try_parse(path).map_err(|error| error.to_string())?;
    if asset_path
        .path()
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("wgsl")
    {
        return Err(format!(
            "Custom material shader '{path}' must use a .wgsl asset; GLSL, WESL and SPIR-V material shaders are not supported by this API"
        ));
    }
    Ok(())
}

fn sync_shader_path(path: &str, asset_server: &AssetServer) -> Result<(), String> {
    validate_material_shader_path(path)?;
    let asset_path = AssetPath::try_parse(path).map_err(|error| error.to_string())?;
    let id = hash_shader_path(path);
    if get_shader_handle(id).is_none() {
        let handle = asset_server.load(asset_path.into_owned());
        register_shader_handle(id, handle);
    }
    Ok(())
}

fn stage_uses_shader(
    materials: &[&MaterialShaderSources],
    stage: ShaderStage,
    shader_id: AssetId<Shader>,
) -> bool {
    materials.iter().any(|material| {
        material
            .path(stage)
            .and_then(|path| get_shader_handle(hash_shader_path(path)))
            .is_some_and(|handle| handle.id() == shader_id)
    })
}

fn expected_generated_material_import(bindings_wgsl: &str) -> Option<&str> {
    bindings_wgsl.lines().find_map(|line| {
        line.trim()
            .strip_prefix("#define_import_path ")
            .map(str::trim)
            .filter(|path| path.starts_with(GENERATED_MATERIAL_IMPORT_PREFIX))
    })
}

fn mismatched_generated_material_imports<'a>(
    shader: &'a Shader,
    expected: &'a str,
) -> impl Iterator<Item = &'a str> + 'a {
    shader
        .imports
        .iter()
        .filter_map(move |import| match import {
            ShaderImport::Custom(actual)
                if actual.starts_with(GENERATED_MATERIAL_IMPORT_PREFIX) && actual != expected =>
            {
                Some(actual.as_str())
            }
            _ => None,
        })
}

fn validate_material_shader_import(
    stage: ShaderStage,
    shader_path: &str,
    expected: &str,
    shaders: &Assets<Shader>,
    reported_diagnostics: &HashSet<String>,
    current_diagnostics: &mut HashSet<String>,
) {
    let Some(handle) = get_shader_handle(hash_shader_path(shader_path)) else {
        return;
    };
    let Some(shader) = shaders.get(&handle) else {
        return;
    };

    for actual in mismatched_generated_material_imports(shader, expected) {
        let message = format!(
            "Custom {} shader '{}' imports generated material '{}', but this material expects \
             '{}'",
            stage.label(),
            shader_path,
            actual,
            expected
        );
        if current_diagnostics.insert(message.clone()) && !reported_diagnostics.contains(&message) {
            error!("{message}");
        }
    }
}

// @material packs fields here and generates the matching WGSL struct at @group(3) @binding(100).

#[derive(ShaderType, Reflect, Clone, Debug)]
pub struct ShaderParams {
    pub data: packed_uniform::UniformData,
}

impl Default for ShaderParams {
    fn default() -> Self {
        Self {
            data: packed_uniform::UniformData::default(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct ShaderMaterialKey {
    pub frag_shader_id: u64,
    pub vert_shader_id: u64,
    pub prepass_vert_shader_id: u64,
    pub prepass_frag_shader_id: u64,
    pub deferred_vert_shader_id: u64,
    pub deferred_frag_shader_id: u64,
    pub shader_defs: u32,
    pub def_names_id: u64,
    pub bindings_id: Option<u64>,
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
            prepass_vert_shader_id: ext
                .prepass_vertex_shader_path
                .as_deref()
                .map(hash_shader_path)
                .unwrap_or(0),
            prepass_frag_shader_id: ext
                .prepass_fragment_shader_path
                .as_deref()
                .map(hash_shader_path)
                .unwrap_or(0),
            deferred_vert_shader_id: ext
                .deferred_vertex_shader_path
                .as_deref()
                .map(hash_shader_path)
                .unwrap_or(0),
            deferred_frag_shader_id: ext
                .deferred_fragment_shader_path
                .as_deref()
                .map(hash_shader_path)
                .unwrap_or(0),
            shader_defs: ext.shader_defs,
            def_names_id: shader_def_registry::intern_names(&ext.shader_def_names),
            bindings_id: ext
                .bindings_wgsl
                .as_deref()
                .filter(|source| expected_generated_material_import(source).is_some())
                .map(hash_shader_path),
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

    // Non-binding fields - used for pipeline specialization only.
    // Fields without #[uniform]/#[texture]/#[sampler] are ignored by AsBindGroup.
    #[reflect(ignore)]
    pub fragment_shader_path: Option<String>,
    #[reflect(ignore)]
    pub vertex_shader_path: Option<String>,
    #[reflect(ignore)]
    pub prepass_vertex_shader_path: Option<String>,
    #[reflect(ignore)]
    pub prepass_fragment_shader_path: Option<String>,
    #[reflect(ignore)]
    pub deferred_vertex_shader_path: Option<String>,
    #[reflect(ignore)]
    pub deferred_fragment_shader_path: Option<String>,

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

impl ShaderMaterialExtension {
    /// The `@material` class name carried by the generated bindings module.
    pub fn material_class_name(&self) -> Option<&str> {
        self.bindings_wgsl
            .as_deref()
            .and_then(expected_generated_material_import)
            .and_then(|path| path.strip_prefix(GENERATED_MATERIAL_IMPORT_PREFIX))
    }

    /// Each custom shader stage label ("fragment", "prepass vertex", ...) and
    /// its asset path.
    pub fn shader_stage_paths(&self) -> impl Iterator<Item = (&'static str, &str)> {
        ShaderStage::ALL
            .into_iter()
            .filter_map(|stage| stage.path(self).map(|path| (stage.label(), path)))
    }

    /// The texture slot at `slot`, or `None` past [`MAX_TEXTURE_SLOTS`].
    pub fn texture_slot(&self, slot: usize) -> Option<&Option<Handle<Image>>> {
        match slot {
            0 => Some(&self.texture_0),
            1 => Some(&self.texture_1),
            2 => Some(&self.texture_2),
            3 => Some(&self.texture_3),
            _ => None,
        }
    }

    /// The mutable texture slot at `slot`, or `None` past [`MAX_TEXTURE_SLOTS`].
    pub fn texture_slot_mut(&mut self, slot: usize) -> Option<&mut Option<Handle<Image>>> {
        match slot {
            0 => Some(&mut self.texture_0),
            1 => Some(&mut self.texture_1),
            2 => Some(&mut self.texture_2),
            3 => Some(&mut self.texture_3),
            _ => None,
        }
    }
}

impl MaterialExtension for ShaderMaterialExtension {
    fn fragment_shader() -> ShaderRef {
        // Default PBR - overridden per-instance in specialize()
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
        if let Some(id) = key.bind_group_data.bindings_id {
            let definition = ShaderDefVal::Bool(binding_definition(id), true);
            descriptor.vertex.shader_defs.push(definition.clone());
            if let Some(fragment) = &mut descriptor.fragment {
                fragment.shader_defs.push(definition);
            }
        }
        let is_prepass = descriptor.vertex.shader_defs.iter().any(
            |def| matches!(def, ShaderDefVal::Bool(name, true) if name == PREPASS_PIPELINE_DEF),
        );
        let is_deferred = is_prepass
            && descriptor.vertex.shader_defs.iter().any(
                |def| matches!(def, ShaderDefVal::Bool(name, true) if name == "DEFERRED_PREPASS"),
            );
        let data = &key.bind_group_data;
        let (vertex_id, fragment_id) = if is_deferred {
            (data.deferred_vert_shader_id, data.deferred_frag_shader_id)
        } else if is_prepass {
            (data.prepass_vert_shader_id, data.prepass_frag_shader_id)
        } else {
            (data.vert_shader_id, data.frag_shader_id)
        };
        if fragment_id != 0
            && let Some(handle) = material_shader_handle(fragment_id, data.bindings_id)
        {
            if let Some(fragment) = &mut descriptor.fragment {
                fragment.shader = handle;
            } else if is_prepass {
                // A custom depth/shadow fragment may discard even with no color targets.
                descriptor.fragment = Some(FragmentState {
                    shader: handle,
                    shader_defs: descriptor.vertex.shader_defs.clone(),
                    targets: Vec::new(),
                    ..Default::default()
                });
            }
        }
        if vertex_id != 0
            && let Some(handle) = material_shader_handle(vertex_id, data.bindings_id)
        {
            descriptor.vertex.shader = handle;
        }

        if is_prepass && vertex_id == 0 && fragment_id == 0 {
            return Ok(());
        }

        let names = shader_def_registry::get_names(data.def_names_id);
        if let Some(name) = shader_def_registry::pipeline_collision(&names, descriptor) {
            error!(
                "Material pipeline {:?}: shader define '{name}' conflicts with an existing pipeline definition (prepass={is_prepass}, deferred={is_deferred})",
                descriptor.label
            );
            // Bevy 0.19 only exposes a missing-vertex-attribute specialization error.
            descriptor.vertex.shader = material_shader_variants::rejected_material_shader();
            return Ok(());
        }

        // Push shader defs from bool fields into each explicitly customized pass.
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

/// Shared material data projection for adapters; rendering owns no interpreter values.
pub trait ShaderMaterialData: Asset {
    fn shader_extension(&self) -> &ShaderMaterialExtension;
    fn shader_extension_mut(&mut self) -> &mut ShaderMaterialExtension;
}

impl ShaderMaterialData for ShaderMaterial {
    fn shader_extension(&self) -> &ShaderMaterialExtension {
        &self.extension
    }
    fn shader_extension_mut(&mut self) -> &mut ShaderMaterialExtension {
        &mut self.extension
    }
}

impl ShaderMaterialData for StorageShaderMaterial {
    fn shader_extension(&self) -> &ShaderMaterialExtension {
        &self.extension.inner
    }
    fn shader_extension_mut(&mut self) -> &mut ShaderMaterialExtension {
        &mut self.extension.inner
    }
}

// Runs in `Last` (after Update, before extraction) so specialize() finds the loaded shaders.

pub fn sync_shader_handles(
    materials: Res<Assets<ShaderMaterial>>,
    storage_materials: Option<Res<Assets<StorageShaderMaterial>>>,
    asset_server: Res<AssetServer>,
    mut shaders: ResMut<Assets<Shader>>,
    mut load_failures: MessageReader<AssetLoadFailedEvent<Shader>>,
    mut reported_diagnostics: Local<HashSet<String>>,
    mut variants: Local<MaterialShaderVariants>,
    mut lifetimes: Local<MaterialShaderLifetimes>,
    mut material_events: MessageReader<AssetEvent<ShaderMaterial>>,
    storage_events: Option<Res<Messages<AssetEvent<StorageShaderMaterial>>>>,
    mut storage_cursor: Local<MessageCursor<AssetEvent<StorageShaderMaterial>>>,
) {
    for event in material_events.read() {
        if let AssetEvent::Unused { id } = event {
            lifetimes.forget(id.untyped());
        }
    }
    if let Some(events) = storage_events {
        for event in storage_cursor.read(&events) {
            if let AssetEvent::Unused { id } = event {
                lifetimes.forget(id.untyped());
            }
        }
    }
    for (id, material) in materials.iter() {
        lifetimes.observe(id.untyped(), &material.extension);
    }
    if let Some(materials) = storage_materials {
        for (id, material) in materials.iter() {
            lifetimes.observe(id.untyped(), &material.extension.inner);
        }
    }
    let extensions = lifetimes.sources();
    for failure in load_failures.read() {
        for stage in ShaderStage::ALL {
            if stage_uses_shader(&extensions, stage, failure.id) {
                error!(
                    "Custom {} shader '{}' failed to load: {}",
                    stage.label(),
                    failure.path,
                    failure.error
                );
            }
        }
    }

    let mut current_diagnostics = HashSet::new();
    for extension in &extensions {
        for stage in ShaderStage::ALL {
            if let Some(path) = extension.path(stage)
                && let Err(parse_error) = sync_shader_path(path, &asset_server)
            {
                let message = format!(
                    "Custom {} shader path '{}' is invalid: {}",
                    stage.label(),
                    path,
                    parse_error
                );
                if current_diagnostics.insert(message.clone())
                    && !reported_diagnostics.contains(&message)
                {
                    error!("{message}");
                }
            }
        }

        // Bevy retries a missing import without logging because an asset may
        // still arrive later. A typo that names another generated material is
        // already available, though, and can silently compile against the
        // wrong binding layout. Check it against this material's own module.
        if let Some(expected) = extension
            .bindings_wgsl
            .as_deref()
            .and_then(expected_generated_material_import)
        {
            for stage in ShaderStage::ALL {
                if let Some(path) = extension.path(stage) {
                    validate_material_shader_import(
                        stage,
                        path,
                        expected,
                        &shaders,
                        &reported_diagnostics,
                        &mut current_diagnostics,
                    );
                }
            }
        }
    }
    if let Ok(mut registry) = bindings_registry().write() {
        sync_generated_bindings(
            &mut registry,
            &mut shaders,
            extensions
                .iter()
                .filter_map(|extension| extension.bindings_wgsl.as_deref()),
        );
    }
    variants.sync(&mut shaders, &extensions);
    *reported_diagnostics = current_diagnostics;
}

pub mod sampler_binding;
