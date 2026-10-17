use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    iter, mem,
    sync::{Arc, Mutex, MutexGuard},
};

use bevy::{
    asset::UntypedAssetId,
    prelude::*,
    render::render_resource::{CachedPipelineState, PipelineCache, PipelineDescriptor},
    shader::{Shader, ShaderCacheError, ShaderImport},
};
use pybevy_core::public_error;
use pybevy_shader_types::{
    ShaderMaterial, ShaderMaterialExtension, StorageShaderMaterial, hash_shader_path,
    material_shader_path_id,
};

/// One failing render pipeline: its compile error and the `hash_shader_path`
/// ids of the `@material` shader files it compiles.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PipelineShaderError {
    pub message: String,
    pub material_shaders: Vec<u64>,
}

impl PipelineShaderError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            material_shaders: Vec::new(),
        }
    }
}

#[derive(Default)]
struct ShaderErrorState {
    failing: Vec<PipelineShaderError>,
    recovered: Vec<u64>,
}

/// Compile errors of the render pipelines that currently fail, and shader
/// files whose failing pipelines compiled again, shared between the render
/// world, which publishes them, and the main world.
#[derive(Resource, Clone, Default)]
pub struct ShaderErrors(Arc<Mutex<ShaderErrorState>>);

impl ShaderErrors {
    fn state(&self) -> MutexGuard<'_, ShaderErrorState> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn current(&self) -> Vec<PipelineShaderError> {
        self.state().failing.clone()
    }

    #[cfg(test)]
    pub(crate) fn replace(&self, errors: Vec<PipelineShaderError>) {
        self.state().failing = errors;
    }

    fn take_recovered(&self) -> Vec<u64> {
        mem::take(&mut self.state().recovered)
    }
}

fn material_shaders(descriptor: &PipelineDescriptor) -> Vec<u64> {
    let mut ids: Vec<u64> = match descriptor {
        PipelineDescriptor::RenderPipelineDescriptor(render) => iter::once(&render.vertex.shader)
            .chain(render.fragment.as_ref().map(|fragment| &fragment.shader))
            .filter_map(material_shader_path_id)
            .collect(),
        PipelineDescriptor::ComputePipelineDescriptor(compute) => {
            material_shader_path_id(&compute.shader)
                .into_iter()
                .collect()
        }
        PipelineDescriptor::MeshPipelineDescriptor(mesh) => mesh
            .task
            .as_ref()
            .map(|task| &task.shader)
            .into_iter()
            .chain(iter::once(&mesh.mesh.shader))
            .chain(mesh.fragment.as_ref().map(|fragment| &fragment.shader))
            .filter_map(material_shader_path_id)
            .collect(),
    };
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Publish the errors of pipelines Bevy gave up on. A pipeline is retried
/// when its shader changes, so a fixed shader clears its entry; an `@material`
/// file whose pipelines failed before is reported recovered once one of them
/// compiles.
pub fn publish_shader_errors(
    cache: Res<PipelineCache>,
    errors: Res<ShaderErrors>,
    mut failed_before: Local<BTreeSet<u64>>,
) {
    let mut failing = Vec::new();
    let mut compiled = BTreeSet::new();
    for pipeline in cache.pipelines() {
        let message = match &pipeline.state {
            CachedPipelineState::Err(ShaderCacheError::ProcessShaderError(error)) => {
                error.to_string()
            }
            CachedPipelineState::Err(ShaderCacheError::CreateShaderModule(description)) => {
                description.clone()
            }
            CachedPipelineState::Ok(_) if !failed_before.is_empty() => {
                compiled.extend(material_shaders(&pipeline.descriptor));
                continue;
            }
            _ => continue,
        };
        failing.push(PipelineShaderError {
            message,
            material_shaders: material_shaders(&pipeline.descriptor),
        });
    }
    failing.sort_unstable();
    failing.dedup();
    let still_failing: BTreeSet<u64> = failing
        .iter()
        .flat_map(|error| error.material_shaders.iter().copied())
        .collect();
    let recovered: Vec<u64> = failed_before
        .iter()
        .copied()
        .filter(|id| !still_failing.contains(id) && compiled.contains(id))
        .collect();
    for id in &recovered {
        failed_before.remove(id);
    }
    failed_before.extend(still_failing);
    let mut state = errors.state();
    state.failing = failing;
    state.recovered.extend(recovered);
}

/// An `@material` class whose custom shader stage uses a failing file.
struct MaterialShaderOwner<'a> {
    class_name: &'a str,
    stage: &'static str,
    path: &'a str,
    asset: UntypedAssetId,
}

fn material_shader_owners<'a>(
    extensions: &[(UntypedAssetId, &'a ShaderMaterialExtension)],
    ids: &[u64],
) -> Vec<MaterialShaderOwner<'a>> {
    let mut owners = Vec::new();
    for &(asset, extension) in extensions {
        let class_name = extension.material_class_name().unwrap_or("Material");
        for (stage, path) in extension.shader_stage_paths() {
            if ids.contains(&hash_shader_path(path)) {
                owners.push(MaterialShaderOwner {
                    class_name,
                    stage,
                    path,
                    asset,
                });
            }
        }
    }
    owners
}

fn material_extensions(world: &World) -> Vec<(UntypedAssetId, &ShaderMaterialExtension)> {
    let mut extensions: Vec<(UntypedAssetId, &ShaderMaterialExtension)> = Vec::new();
    if let Some(materials) = world.get_resource::<Assets<ShaderMaterial>>() {
        extensions.extend(
            materials
                .iter()
                .map(|(id, material)| (id.untyped(), &material.extension)),
        );
    }
    if let Some(materials) = world.get_resource::<Assets<StorageShaderMaterial>>() {
        extensions.extend(
            materials
                .iter()
                .map(|(id, material)| (id.untyped(), &material.extension.inner)),
        );
    }
    extensions
}

fn material_entities(world: &World) -> Vec<(UntypedAssetId, Entity)> {
    let mut entities = Vec::new();
    if let Some(mut query) = world.try_query::<(Entity, &MeshMaterial3d<ShaderMaterial>)>() {
        entities.extend(
            query
                .iter(world)
                .map(|(entity, material)| (material.id().untyped(), entity)),
        );
    }
    if let Some(mut query) = world.try_query::<(Entity, &MeshMaterial3d<StorageShaderMaterial>)>() {
        entities.extend(
            query
                .iter(world)
                .map(|(entity, material)| (material.id().untyped(), entity)),
        );
    }
    entities
}

/// Stages and entities per `@material` class and shader file.
type OwnerGroups<'a> = BTreeMap<(&'a str, &'a str), (Vec<&'static str>, Vec<Entity>)>;

/// Prefix `error` with the `@material` classes, stages, files and entities it
/// belongs to; errors no `@material` owns keep their plain message.
fn attribute(
    error: &PipelineShaderError,
    extensions: &[(UntypedAssetId, &ShaderMaterialExtension)],
    entities: &[(UntypedAssetId, Entity)],
) -> String {
    let owners = material_shader_owners(extensions, &error.material_shaders);
    let mut groups: OwnerGroups = BTreeMap::new();
    for owner in &owners {
        let (stages, owned) = groups.entry((owner.class_name, owner.path)).or_default();
        if !stages.contains(&owner.stage) {
            stages.push(owner.stage);
        }
        owned.extend(
            entities
                .iter()
                .filter(|(asset, _)| *asset == owner.asset)
                .map(|(_, entity)| *entity),
        );
    }
    if groups.is_empty() {
        return error.message.clone();
    }
    let owners: Vec<String> = groups
        .into_iter()
        .map(|((class_name, path), (stages, mut entities))| {
            entities.sort_by_key(|entity| entity.index());
            entities.dedup();
            public_error::material_shader_owner(class_name, &stages.join(", "), path, &entities)
        })
        .collect();
    format!("{}: {}", owners.join("; "), error.message)
}

/// The current compile errors, attributed to their `@material` owners.
fn attributed_shader_errors(world: &World) -> Vec<String> {
    let Some(errors) = world.get_resource::<ShaderErrors>() else {
        return Vec::new();
    };
    let failing = errors.current();
    if failing.is_empty() {
        return Vec::new();
    }
    let extensions = material_extensions(world);
    let entities = material_entities(world);
    failing
        .iter()
        .map(|error| attribute(error, &extensions, &entities))
        .collect()
}

/// Log each attributed compile error once, and a line when a failing shader
/// file compiles again.
pub fn report_material_shader_errors(world: &World, mut reported: Local<Vec<PipelineShaderError>>) {
    let Some(errors) = world.get_resource::<ShaderErrors>() else {
        return;
    };
    let recovered = errors.take_recovered();
    let failing = errors.current();
    if failing != *reported {
        let extensions = material_extensions(world);
        let entities = material_entities(world);
        for error in failing.iter().filter(|error| !reported.contains(error)) {
            error!("{}", attribute(error, &extensions, &entities));
        }
        *reported = failing;
    }
    if recovered.is_empty() {
        return;
    }
    let extensions = material_extensions(world);
    for id in recovered {
        let owners = material_shader_owners(&extensions, &[id]);
        let Some(path) = owners.first().map(|owner| owner.path) else {
            continue;
        };
        let mut classes: Vec<&str> = owners.iter().map(|owner| owner.class_name).collect();
        classes.sort_unstable();
        classes.dedup();
        info!(
            "{}",
            public_error::material_shader_recovered(&classes, path)
        );
    }
}

/// Custom `#import`s no loaded shader defines. Bevy keeps such pipelines queued
/// without an error, so the shader silently draws nothing.
#[derive(Resource, Default)]
pub struct UnresolvedShaderImports(Vec<String>);

pub fn track_unresolved_shader_imports(
    mut events: MessageReader<AssetEvent<Shader>>,
    shaders: Res<Assets<Shader>>,
    mut unresolved: ResMut<UnresolvedShaderImports>,
) {
    if events.read().count() == 0 {
        return;
    }
    let provided: HashSet<&str> = shaders
        .iter()
        .filter_map(|(_, shader)| match &shader.import_path {
            ShaderImport::Custom(path) => Some(path.as_str()),
            ShaderImport::AssetPath(_) => None,
        })
        .collect();
    // Engine shaders import modules of pipelines that may never be built.
    let mut missing: Vec<String> = shaders
        .iter()
        .filter(|(_, shader)| !shader.path.starts_with("embedded://"))
        .flat_map(|(_, shader)| {
            let imported: HashSet<&str> = shader
                .imports
                .iter()
                .filter_map(|import| match import {
                    ShaderImport::Custom(path) => Some(path.as_str()),
                    ShaderImport::AssetPath(_) => None,
                })
                .collect();
            // WESL lists each imported item beside its module; report the module once.
            imported
                .iter()
                .filter(|path| {
                    !path
                        .rsplit_once("::")
                        .is_some_and(|(module, _)| imported.contains(module))
                })
                .filter(|path| !provided.contains(**path))
                .map(|path| public_error::shader_import_unresolved(&shader.path, path))
                .collect::<Vec<_>>()
        })
        .collect();
    missing.sort_unstable();
    missing.dedup();
    unresolved.0 = missing;
}

/// Compile errors and unresolved imports, as `get_last_error.shader_errors` reports them.
pub fn current_shader_errors(world: &World) -> Vec<String> {
    let mut errors = attributed_shader_errors(world);
    if let Some(unresolved) = world.get_resource::<UnresolvedShaderImports>() {
        errors.extend(unresolved.0.iter().cloned());
    }
    errors
}

#[cfg(test)]
mod tests {
    use bevy::{asset::AssetId, prelude::*};
    use pybevy_shader_types::{ShaderMaterial, ShaderMaterialExtension, hash_shader_path};
    use uuid::Uuid;

    use super::{PipelineShaderError, attribute};

    fn extension(class_name: &str, fragment: &str) -> ShaderMaterialExtension {
        ShaderMaterialExtension {
            fragment_shader_path: Some(fragment.to_owned()),
            deferred_fragment_shader_path: Some(fragment.to_owned()),
            bindings_wgsl: Some(format!("// module pybevy::gen::{class_name}\n")),
            ..Default::default()
        }
    }

    #[test]
    fn errors_name_the_material_class_stages_file_and_entities() {
        let mut world = World::new();
        let first = world.spawn_empty().id();
        let second = world.spawn_empty().id();
        let glow = extension("GlowMaterial", "shaders/glow.wgsl");
        let other = extension("OtherMaterial", "shaders/other.wgsl");
        let glow_id = AssetId::<ShaderMaterial>::Uuid {
            uuid: Uuid::from_u128(1),
        }
        .untyped();
        let other_id = AssetId::<ShaderMaterial>::Uuid {
            uuid: Uuid::from_u128(2),
        }
        .untyped();
        let error = PipelineShaderError {
            message: "expected `;`".to_owned(),
            material_shaders: vec![hash_shader_path("shaders/glow.wgsl")],
        };

        let attributed = attribute(
            &error,
            &[(glow_id, &glow), (other_id, &other)],
            &[(glow_id, second), (other_id, first), (glow_id, first)],
        );

        assert_eq!(
            attributed,
            format!(
                "@material GlowMaterial fragment, deferred fragment shader shaders/glow.wgsl \
                 (entities {first}, {second}): expected `;`"
            )
        );
    }

    #[test]
    fn errors_no_material_owns_keep_their_message() {
        let error = PipelineShaderError {
            message: "expected `;`".to_owned(),
            material_shaders: vec![hash_shader_path("shaders/standalone.wgsl")],
        };

        assert_eq!(attribute(&error, &[], &[]), "expected `;`");
    }
}
