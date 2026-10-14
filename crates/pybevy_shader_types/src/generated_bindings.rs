//! Preserve public material import names while selecting the drawing material's layout.
//! Live classes/reload generations may share an import but have different fields;
//! their pipeline keys supply exactly one of the generated shader definitions.

use std::collections::{BTreeMap, HashMap};

use bevy::{
    asset::{Assets, Handle},
    shader::{Shader, ShaderImport},
};

use crate::{GENERATED_MODULE_HEADER, expected_generated_material_import, hash_shader_path};

pub(crate) struct GeneratedBindingsShader {
    source: String,
    handle: Handle<Shader>,
}

pub(crate) type BindingsRegistry = HashMap<ShaderImport, GeneratedBindingsShader>;

pub(crate) fn binding_definition(id: u64) -> String {
    format!("PYBEVY_BINDINGS_{id:016X}")
}

/// The generated module is a WESL package module, importable as `pybevy::gen::<Class>`.
pub(crate) fn generated_bindings_shader(source: String, import_path: &str) -> Shader {
    let mut shader = Shader::from_wesl(source, import_path.to_owned());
    shader.import_path = ShaderImport::Custom(import_path.to_owned());
    shader
}

pub(crate) fn sync_generated_bindings<'a>(
    registry: &mut BindingsRegistry,
    shaders: &mut Assets<Shader>,
    sources: impl Iterator<Item = &'a str>,
) {
    let mut modules: HashMap<&str, BTreeMap<u64, &str>> = HashMap::new();
    for source in sources {
        if let Some(import) = expected_generated_material_import(source) {
            modules
                .entry(import)
                .or_default()
                .insert(hash_shader_path(source), source);
        }
    }
    registry.retain(|import, _| match import {
        ShaderImport::Custom(path) => modules.contains_key(path.as_str()),
        ShaderImport::AssetPath(_) => false,
    });
    for (import, variants) in modules {
        let source = if variants.len() == 1 {
            variants.values().next().unwrap().to_string()
        } else {
            let mut source = format!("{GENERATED_MODULE_HEADER}{import}\n");
            for (id, variant) in variants {
                source.push_str(&format!(
                    "@if({}) {{\n{}\n}}\n",
                    binding_definition(id),
                    variant.trim_end()
                ));
            }
            source
        };
        let key = ShaderImport::Custom(import.to_owned());
        if registry
            .get(&key)
            .is_some_and(|entry| entry.source == source)
        {
            continue;
        }
        let shader = generated_bindings_shader(source.clone(), import);
        if let Some(entry) = registry.get_mut(&key)
            && shaders.insert(entry.handle.id(), shader.clone()).is_ok()
        {
            entry.source = source;
            continue;
        }
        registry.insert(
            key,
            GeneratedBindingsShader {
                source,
                handle: shaders.add(shader),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        asset::Assets,
        shader::{Shader, ShaderImport, Source},
    };

    use super::{BindingsRegistry, binding_definition, sync_generated_bindings};
    use crate::hash_shader_path;

    const FIRST: &str = "// module pybevy::gen::Surface\nstruct Surface { first: f32, }\n";
    const SECOND: &str = "// module pybevy::gen::Surface\nstruct Surface { second: vec2<f32>, }\n";

    #[test]
    fn live_layouts_coexist_and_retired_layouts_are_pruned() {
        let mut registry = BindingsRegistry::new();
        let mut shaders = Assets::<Shader>::default();
        let import = ShaderImport::Custom("pybevy::gen::Surface".to_owned());
        sync_generated_bindings(&mut registry, &mut shaders, [FIRST].into_iter());
        let handle = registry[&import].handle.clone();
        let published = shaders.get(&handle).unwrap();
        assert_eq!(published.import_path, import);
        assert!(matches!(published.source, Source::Wesl(_)));
        assert!(published.imports.is_empty());
        sync_generated_bindings(&mut registry, &mut shaders, [SECOND, FIRST].into_iter());
        let source = shaders.get(&handle).unwrap().source.as_str();
        for variant in [FIRST, SECOND] {
            let gate = binding_definition(hash_shader_path(variant));
            assert!(source.contains(&format!("@if({gate}) {{\n{}\n}}\n", variant.trim_end())));
        }
        assert!(source.starts_with("// module pybevy::gen::Surface\n"));
        assert_eq!(registry[&import].handle.id(), handle.id());
        let combined = source.to_string();
        sync_generated_bindings(
            &mut registry,
            &mut shaders,
            [FIRST, SECOND, FIRST].into_iter(),
        );
        assert_eq!(shaders.get(&handle).unwrap().source.as_str(), combined);
        sync_generated_bindings(&mut registry, &mut shaders, [SECOND].into_iter());
        assert_eq!(shaders.get(&handle).unwrap().source.as_str(), SECOND);
        assert_eq!(registry[&import].handle.id(), handle.id());
        assert_eq!(shaders.len(), 1);
        sync_generated_bindings(&mut registry, &mut shaders, [].into_iter());
        assert!(registry.is_empty());
    }

    #[test]
    fn bindings_without_a_generated_module_header_are_not_published() {
        let mut registry = BindingsRegistry::new();
        let mut shaders = Assets::<Shader>::default();
        sync_generated_bindings(
            &mut registry,
            &mut shaders,
            ["struct Raw { value: f32, }\n"].into_iter(),
        );
        assert!(registry.is_empty());
        assert_eq!(shaders.len(), 0);
    }
}
