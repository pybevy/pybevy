//! Immutable per-layout imports keep retired pipelines out of new binding layouts.
use std::collections::{HashMap, HashSet};

use bevy::{
    asset::{AssetId, Assets, Handle, UntypedAssetId, uuid::Uuid},
    shader::{Shader, ShaderDefVal, ShaderImport, Source},
};

use crate::{
    GENERATED_MATERIAL_IMPORT_PREFIX, ShaderMaterialExtension, ShaderStage,
    expected_generated_material_import, generated_bindings::generated_bindings_shader,
    get_shader_handle, hash_shader_path, registered_path_id,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MaterialShaderSources {
    pub(crate) bindings_wgsl: Option<String>,
    paths: [Option<String>; 6],
}

impl From<&ShaderMaterialExtension> for MaterialShaderSources {
    fn from(extension: &ShaderMaterialExtension) -> Self {
        Self {
            bindings_wgsl: extension.bindings_wgsl.clone(),
            paths: ShaderStage::ALL.map(|stage| stage.path(extension).map(str::to_owned)),
        }
    }
}

impl MaterialShaderSources {
    pub(crate) fn path(&self, stage: ShaderStage) -> Option<&str> {
        self.paths[stage as usize].as_deref()
    }
}

#[derive(Default)]
pub struct MaterialShaderLifetimes {
    sources: HashMap<UntypedAssetId, MaterialShaderSources>,
}

impl MaterialShaderLifetimes {
    pub(crate) fn forget(&mut self, id: UntypedAssetId) {
        self.sources.remove(&id);
    }

    pub(crate) fn observe(&mut self, id: UntypedAssetId, extension: &ShaderMaterialExtension) {
        if self.sources.get(&id).is_some_and(|source| {
            source.bindings_wgsl == extension.bindings_wgsl
                && ShaderStage::ALL
                    .into_iter()
                    .all(|stage| source.path(stage) == stage.path(extension))
        }) {
            return;
        }
        // Store only shader metadata: owning a material handle would prevent Unused.
        self.sources.insert(id, extension.into());
    }

    pub(crate) fn sources(&self) -> Vec<&MaterialShaderSources> {
        self.sources.values().collect()
    }
}

#[derive(Default)]
pub struct MaterialShaderVariants {
    entries: HashMap<AssetId<Shader>, (String, Vec<ShaderDefVal>)>,
}

fn stable_handle(namespace: u64, value: u64) -> Handle<Shader> {
    Handle::Uuid(
        Uuid::from_u128((u128::from(namespace) << 64) | u128::from(value)),
        Default::default(),
    )
}

/// The `hash_shader_path` id of the `@material` shader file a pipeline handle
/// compiles: a per-layout variant carries it in its high half, and the plain
/// file handle is the one registered under it.
pub fn material_shader_path_id(handle: &Handle<Shader>) -> Option<u64> {
    if let Handle::Uuid(uuid, _) = handle {
        let path_id = (uuid.as_u128() >> 64) as u64;
        return get_shader_handle(path_id).map(|_| path_id);
    }
    registered_path_id(handle.id())
}

pub(crate) fn material_shader_handle(path: u64, bindings: Option<u64>) -> Option<Handle<Shader>> {
    match bindings {
        Some(bindings) => Some(stable_handle(path, bindings)),
        None => get_shader_handle(path),
    }
}

pub(crate) fn rejected_material_shader() -> Handle<Shader> {
    stable_handle(0x5059424556594552, 1)
}

fn replace_import(source: &str, original: &str, replacement: &str) -> String {
    let mut result = String::new();
    let mut start = 0;
    for (offset, _) in source.match_indices(original) {
        let end = offset + original.len();
        if source[end..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
            || source[..offset]
                .ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == ':')
        {
            continue;
        }
        result.push_str(&source[start..offset]);
        result.push_str(replacement);
        start = end;
    }
    result.push_str(&source[start..]);
    result
}

/// Keeps the material's module name so `import pybevy::gen::Class;` still binds `Class`.
fn versioned_material_import(import: &str, binding_id: u64) -> String {
    let class = import
        .strip_prefix(GENERATED_MATERIAL_IMPORT_PREFIX)
        .unwrap_or(import);
    format!("pybevy::gen_{binding_id:016x}::{class}")
}

/// A sibling module path, so the variant's `super::` imports resolve like the original's.
fn variant_module_path(original: &Shader, binding_id: u64) -> String {
    match &original.import_path {
        ShaderImport::AssetPath(path) => format!(
            "{}_pybevy_{binding_id:016x}.wesl",
            path.trim_start_matches('/')
        ),
        ShaderImport::Custom(path) => format!(
            "embedded://{}_pybevy_{binding_id:016x}.wesl",
            path.replace("::", "/")
        ),
    }
}

fn variant_source(original: &Shader, import: &str, binding_id: u64) -> String {
    match &original.source {
        Source::Wesl(source) => replace_import(
            source,
            import,
            &versioned_material_import(import, binding_id),
        ),
        source => source.as_str().to_owned(),
    }
}

fn material_shader_variant(original: &Shader, source: String, binding_id: u64) -> Shader {
    let mut variant = match &original.source {
        Source::Wesl(_) => Shader::from_wesl(source, variant_module_path(original, binding_id)),
        _ => {
            let mut variant = original.clone();
            variant.import_path = ShaderImport::AssetPath(format!(
                "pybevy://material/{binding_id:016x}/{}",
                original.path
            ));
            variant
        }
    };
    variant.path = original.path.clone();
    variant.shader_defs = original.shader_defs.clone();
    variant.file_dependencies = original.file_dependencies.clone();
    variant.validate_shader = original.validate_shader.clone();
    variant
}

impl MaterialShaderVariants {
    fn is_current(&self, handle: &Handle<Shader>, source: &str, defs: &[ShaderDefVal]) -> bool {
        self.entries
            .get(&handle.id())
            .is_some_and(|(current, current_defs)| current == source && current_defs == defs)
    }

    fn publish(&mut self, shaders: &mut Assets<Shader>, handle: &Handle<Shader>, shader: Shader) {
        let state = (
            shader.source.as_str().to_string(),
            shader.shader_defs.clone(),
        );
        if self.entries.get(&handle.id()) == Some(&state) {
            return;
        }
        shaders
            .insert(handle.id(), shader)
            .expect("UUID shader IDs are always insertable");
        self.entries.insert(handle.id(), state);
    }

    pub(crate) fn sync(
        &mut self,
        shaders: &mut Assets<Shader>,
        extensions: &[&MaterialShaderSources],
    ) {
        let rejected = rejected_material_shader();
        self.publish(
            shaders,
            &rejected,
            Shader::from_wgsl(
                "const_assert false;",
                "pybevy://material/rejected-define-collision",
            ),
        );
        let mut live = HashSet::from([rejected.id()]);
        for extension in extensions {
            let Some(bindings) = extension.bindings_wgsl.as_deref() else {
                continue;
            };
            let Some(import) = expected_generated_material_import(bindings) else {
                continue;
            };
            let binding_id = hash_shader_path(bindings);
            let binding_handle = stable_handle(0x5059424556595642, binding_id);
            if live.insert(binding_handle.id()) {
                self.publish(
                    shaders,
                    &binding_handle,
                    generated_bindings_shader(
                        bindings.to_owned(),
                        &versioned_material_import(import, binding_id),
                    ),
                );
            }
            for stage in ShaderStage::ALL {
                let Some(path) = extension.path(stage) else {
                    continue;
                };
                let path_id = hash_shader_path(path);
                let variant_handle = stable_handle(path_id, binding_id);
                if !live.insert(variant_handle.id()) {
                    continue;
                }
                let Some(original) =
                    get_shader_handle(path_id).and_then(|handle| shaders.get(&handle))
                else {
                    continue;
                };
                let source = variant_source(original, import, binding_id);
                if self.is_current(&variant_handle, &source, &original.shader_defs) {
                    continue;
                }
                let variant = material_shader_variant(original, source, binding_id);
                self.publish(shaders, &variant_handle, variant);
            }
        }
        self.entries.retain(|id, _| {
            if live.contains(id) {
                true
            } else {
                shaders.remove(*id);
                false
            }
        });
    }
}

#[cfg(test)]
mod composition_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ShaderMaterial, StorageShaderMaterial, register_shader_handle};

    #[test]
    fn stage_variants_import_their_layout_and_follow_source_edits() {
        let path = "shaders/variant_sync_probe.wesl";
        let mut shaders = Assets::<Shader>::default();
        let original = shaders.add(Shader::from_wesl(
            "import pybevy::gen::Probe as params;\n@fragment fn fragment() -> @location(0) vec4<f32> { return params::material.color; }\n",
            path,
        ));
        register_shader_handle(hash_shader_path(path), original.clone());
        let bindings = "// module pybevy::gen::Probe\nstruct Probe { color: vec4<f32>, }\n";
        let extension = ShaderMaterialExtension {
            fragment_shader_path: Some(path.into()),
            bindings_wgsl: Some(bindings.into()),
            ..Default::default()
        };
        let sources = MaterialShaderSources::from(&extension);
        let binding_id = hash_shader_path(bindings);
        let handle = material_shader_handle(hash_shader_path(path), Some(binding_id)).unwrap();
        let mut variants = MaterialShaderVariants::default();
        variants.sync(&mut shaders, &[&sources]);
        let variant = shaders.get(&handle).unwrap();
        let versioned = format!("pybevy::gen_{binding_id:016x}::Probe");
        assert!(matches!(variant.source, Source::Wesl(_)));
        assert_eq!(variant.path, path);
        assert_eq!(
            variant.import_path,
            ShaderImport::AssetPath(format!(
                "/shaders/variant_sync_probe_pybevy_{binding_id:016x}"
            ))
        );
        assert_eq!(
            variant.imports,
            vec![
                ShaderImport::Custom(format!("pybevy::gen_{binding_id:016x}")),
                ShaderImport::Custom(versioned.clone()),
            ]
        );
        let layout = stable_handle(0x5059424556595642, binding_id);
        assert_eq!(
            shaders.get(&layout).unwrap().import_path,
            ShaderImport::Custom(versioned)
        );
        shaders.insert(original.id(), Shader::from_wesl(
            "import pybevy::gen::Probe as params;\n@fragment fn fragment() -> @location(0) vec4<f32> { return params::material.color * 2.0; }\n",
            path,
        )).unwrap();
        variants.sync(&mut shaders, &[&sources]);
        assert!(
            shaders
                .get(&handle)
                .unwrap()
                .source
                .as_str()
                .contains("* 2.0")
        );
    }

    #[test]
    fn import_replacement_preserves_prefix_neighbors() {
        let source = "import pybevy::gen::Surface as p;\nimport pybevy::gen::SurfaceExtra as q;\nimport x::pybevy::gen::Surface;\np::x; pybevy::gen::Surface::value;";
        assert_eq!(
            replace_import(source, "pybevy::gen::Surface", "pybevy::gen_0a::Surface"),
            "import pybevy::gen_0a::Surface as p;\nimport pybevy::gen::SurfaceExtra as q;\nimport x::pybevy::gen::Surface;\np::x; pybevy::gen_0a::Surface::value;"
        );
    }
    #[test]
    fn layout_variants_are_distinct_and_retired_assets_are_removed() {
        let first = ShaderMaterialExtension {
            bindings_wgsl: Some(
                "// module pybevy::gen::Surface\nstruct Data { tint: f32, }".into(),
            ),
            ..Default::default()
        };
        let second = ShaderMaterialExtension {
            bindings_wgsl: Some(
                "// module pybevy::gen::Surface\nstruct Data { tint: vec4<f32>, }".into(),
            ),
            ..Default::default()
        };
        let mut variants = MaterialShaderVariants::default();
        let mut shaders = Assets::<Shader>::default();
        let first = MaterialShaderSources::from(&first);
        let second = MaterialShaderSources::from(&second);
        variants.sync(&mut shaders, &[&first, &second]);
        assert_eq!(shaders.len(), 3);
        let paths: HashSet<_> = shaders
            .iter()
            .map(|(_, shader)| shader.import_path.clone())
            .collect();
        assert_eq!(paths.len(), 3);
        variants.sync(&mut shaders, &[&second]);
        assert_eq!(shaders.len(), 2);
        variants.sync(&mut shaders, &[]);
        assert_eq!(shaders.len(), 1);
        assert!(shaders.contains(rejected_material_shader().id()));
        assert_eq!(variants.entries.len(), 1);
    }
    #[test]
    fn removed_material_sources_survive_until_unused_without_aliasing_asset_types() {
        let mut ordinary = Assets::<ShaderMaterial>::default();
        let mut extended = Assets::<StorageShaderMaterial>::default();
        let a = ordinary.add(ShaderMaterial::default());
        let b = extended.add(StorageShaderMaterial::default());
        let first = ShaderMaterialExtension {
            bindings_wgsl: Some(
                "// module pybevy::gen::Retained\nstruct Data { value: f32, }".into(),
            ),
            ..Default::default()
        };
        let mut second = first.clone();
        second.bindings_wgsl =
            Some("// module pybevy::gen::Retained\nstruct Data { value: i32, }".into());
        let mut lifetimes = MaterialShaderLifetimes::default();
        lifetimes.observe(a.id().untyped(), &first);
        lifetimes.observe(b.id().untyped(), &first);
        assert_eq!(lifetimes.sources.len(), 2);
        ordinary.remove(a.id());
        lifetimes.observe(b.id().untyped(), &second);
        let mut variants = MaterialShaderVariants::default();
        let mut shaders = Assets::<Shader>::default();
        variants.sync(&mut shaders, &lifetimes.sources());
        assert_eq!(shaders.len(), 3);
        lifetimes.forget(a.id().untyped());
        variants.sync(&mut shaders, &lifetimes.sources());
        assert_eq!(shaders.len(), 2);
        assert_eq!(lifetimes.sources()[0].bindings_wgsl, second.bindings_wgsl);
        lifetimes.forget(b.id().untyped());
        variants.sync(&mut shaders, &lifetimes.sources());
        assert_eq!(shaders.len(), 1);
        assert!(lifetimes.sources().is_empty());
    }
}
