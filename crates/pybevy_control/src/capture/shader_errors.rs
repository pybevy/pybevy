use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    iter, mem,
    sync::{Arc, Mutex, MutexGuard},
};

use bevy::{
    asset::UntypedAssetId,
    prelude::*,
    render::render_resource::{CachedPipelineState, PipelineCache, PipelineDescriptor},
    shader::{Shader, ShaderCacheError, ShaderImport, Source},
};
use pybevy_core::public_error;
use pybevy_shader_types::{
    ShaderMaterial, ShaderMaterialExtension, StorageShaderMaterial, hash_shader_path,
    material_shader_path_id, rejected_define_conflict,
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
        // The rejection placeholder's own compile error does not name the define.
        let message = match &pipeline.descriptor {
            PipelineDescriptor::RenderPipelineDescriptor(render) => {
                rejected_define_conflict(render).unwrap_or(message)
            }
            _ => message,
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

/// WESL imports no loaded shader defines. Bevy keeps such pipelines queued
/// without an error, so the shader silently draws nothing.
#[derive(Resource, Default)]
pub struct UnresolvedShaderImports(Vec<String>);

/// A WESL module path: a package path such as `bevy_pbr::render`, or an asset
/// path under `package::` when `package` is `None`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ModulePath {
    package: Option<String>,
    components: Vec<String>,
}

impl ModulePath {
    fn of(import: &ShaderImport) -> Option<Self> {
        match import {
            ShaderImport::Custom(path) => {
                let mut segments = path.split("::");
                let package = segments.next().filter(|segment| !segment.is_empty())?;
                let components = segments
                    .map(|segment| (!segment.is_empty()).then(|| segment.to_owned()))
                    .collect::<Option<Vec<_>>>()?;
                Some(Self {
                    package: Some(package.to_owned()),
                    components,
                })
            }
            ShaderImport::AssetPath(path) => Some(Self {
                package: None,
                components: path
                    .split('/')
                    .filter(|component| !component.is_empty())
                    .map(str::to_owned)
                    .collect(),
            }),
        }
    }

    fn parent(&self) -> Option<Self> {
        let (_, components) = self.components.split_last()?;
        Some(Self {
            package: self.package.clone(),
            components: components.to_vec(),
        })
    }

    fn is_constants(&self) -> bool {
        self.package.as_deref() == Some("constants") && self.components.is_empty()
    }

    fn describe(&self) -> String {
        iter::once(self.package.as_deref().unwrap_or("package"))
            .chain(self.components.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join("::")
    }

    fn message(&self, shader: &str) -> String {
        match self.package {
            Some(_) => public_error::shader_import_unresolved(shader, &self.describe()),
            None => public_error::shader_asset_import_unresolved(
                shader,
                &self.describe(),
                &format!("{}.wesl", self.components.join("/")),
            ),
        }
    }
}

/// Bevy lists each WESL import as its module beside the module joined with the
/// imported name, which may itself be a module. An import resolves when either
/// is a loaded WESL module; otherwise the module is reported, or the import
/// itself when its module is a bare package.
fn unresolved_imports(shaders: &[&Shader]) -> Vec<String> {
    let provided: HashSet<ModulePath> = shaders
        .iter()
        .filter(|shader| matches!(shader.source, Source::Wesl(_)))
        .filter_map(|shader| ModulePath::of(&shader.import_path))
        .collect();
    let resolves = |path: &ModulePath| path.is_constants() || provided.contains(path);
    // Engine shaders import modules of pipelines that may never be built.
    let mut missing: Vec<String> = shaders
        .iter()
        .filter(|shader| !shader.path.starts_with("embedded://"))
        .flat_map(|shader| {
            let imported: Vec<ModulePath> =
                shader.imports.iter().filter_map(ModulePath::of).collect();
            imported
                .iter()
                .filter_map(|path| {
                    let module = path.parent().filter(|parent| imported.contains(parent))?;
                    if resolves(path) || resolves(&module) {
                        return None;
                    }
                    let reported = if module.components.is_empty() {
                        path
                    } else {
                        &module
                    };
                    Some(reported.message(&shader.path))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    missing.sort_unstable();
    missing.dedup();
    missing
}

pub fn track_unresolved_shader_imports(
    mut events: MessageReader<AssetEvent<Shader>>,
    shaders: Res<Assets<Shader>>,
    mut unresolved: ResMut<UnresolvedShaderImports>,
) {
    if events.read().count() == 0 {
        return;
    }
    let shaders: Vec<&Shader> = shaders.iter().map(|(_, shader)| shader).collect();
    unresolved.0 = unresolved_imports(&shaders);
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
    use bevy::{
        asset::AssetId,
        prelude::*,
        shader::{Shader, ShaderImport},
    };
    use pybevy_shader_types::{ShaderMaterial, ShaderMaterialExtension, hash_shader_path};
    use uuid::Uuid;

    use super::{PipelineShaderError, attribute, unresolved_imports};

    fn module(path: &str, source: &str) -> Shader {
        let mut shader = Shader::from_wesl(source.to_owned(), path.to_owned());
        shader.import_path = ShaderImport::Custom(path.to_owned());
        shader
    }

    fn asset(path: &str, source: &str) -> Shader {
        Shader::from_wesl(source.to_owned(), path.to_owned())
    }

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

    #[test]
    fn module_item_material_and_relative_imports_of_loaded_modules_resolve() {
        let shaders = [
            module("pybevy::gen::Glow", "struct Glow { color: vec4<f32>, }"),
            module("bevy_render::maths", "const PI: f32 = 3.14159;"),
            asset("shaders/common/tint.wesl", "fn tint() {}"),
            asset(
                "shaders/glow.wesl",
                "import pybevy::gen::Glow as params;\n\
                 import bevy_render::maths;\n\
                 import bevy_render::{maths::PI};\n\
                 import constants::MATERIAL_BIND_GROUP;\n\
                 import super::common::tint::tint;\n\
                 fn f() {}",
            ),
        ];

        assert_eq!(
            unresolved_imports(&shaders.each_ref()),
            Vec::<String>::new()
        );
    }

    #[test]
    fn missing_modules_are_named_once_per_shader() {
        let shaders = [asset(
            "shaders/broken.wesl",
            "import lab::common::{tint, shade};\n\
             import lab_root::missing;\n\
             import super::common::nope::tint;\n\
             fn f() {}",
        )];

        assert_eq!(
            unresolved_imports(&shaders.each_ref()),
            vec![
                "shaders/broken.wesl imports `lab::common`, which no loaded shader provides; \
                 load the shader that defines module `lab::common` as a Shader asset and keep its handle",
                "shaders/broken.wesl imports `lab_root::missing`, which no loaded shader provides; \
                 load the shader that defines module `lab_root::missing` as a Shader asset and keep its handle",
                "shaders/broken.wesl imports `package::shaders::common::nope`, \
                 but no shader asset `shaders/common/nope.wesl` is loaded",
            ]
        );
    }

    #[test]
    fn a_wgsl_shader_does_not_provide_a_wesl_module() {
        let mut helper = Shader::from_wgsl("fn tint() {}", "shaders/common/tint.wgsl");
        helper.import_path = ShaderImport::AssetPath("/shaders/common/tint".to_owned());
        let shaders = [
            helper,
            asset(
                "shaders/uses_wgsl.wesl",
                "import super::common::tint::tint;\nfn f() {}",
            ),
        ];

        assert_eq!(
            unresolved_imports(&shaders.each_ref()),
            vec![
                "shaders/uses_wgsl.wesl imports `package::shaders::common::tint`, \
                 but no shader asset `shaders/common/tint.wesl` is loaded"
            ]
        );
    }
}
