//! Preserve public material import names while selecting the drawing material's layout.
//! Live classes/reload generations may share an import but have different fields;
//! their pipeline keys supply exactly one of the generated shader definitions.

use std::collections::{BTreeMap, HashMap};

use bevy::{
    asset::{Assets, Handle},
    shader::{Shader, ShaderImport},
};

use crate::hash_shader_path;

pub(crate) struct GeneratedBindingsShader {
    source: String,
    handle: Handle<Shader>,
}

pub(crate) type BindingsRegistry = HashMap<ShaderImport, GeneratedBindingsShader>;

pub(crate) fn binding_definition(id: u64) -> String {
    format!("PYBEVY_BINDINGS_{id:016X}")
}

pub(crate) fn sync_generated_bindings<'a>(
    registry: &mut BindingsRegistry,
    shaders: &mut Assets<Shader>,
    sources: impl Iterator<Item = &'a str>,
) {
    let mut modules: HashMap<ShaderImport, BTreeMap<u64, &str>> = HashMap::new();
    for source in sources {
        let id = hash_shader_path(source);
        let shader = Shader::from_wgsl(source.to_owned(), format!("pybevy://gen/{id}"));
        modules
            .entry(shader.import_path)
            .or_default()
            .insert(id, source);
    }
    registry.retain(|import, _| modules.contains_key(import));
    for (import, variants) in modules {
        let source = if variants.len() == 1 {
            variants.values().next().unwrap().to_string()
        } else {
            let ShaderImport::Custom(path) = &import else {
                continue;
            };
            let mut source = format!("#define_import_path {path}\n");
            for (id, variant) in variants {
                source.push_str(&format!("#ifdef {}\n", binding_definition(id)));
                for line in variant.lines() {
                    if !line.trim().starts_with("#define_import_path ") {
                        source.push_str(line);
                        source.push('\n');
                    }
                }
                source.push_str("#endif\n");
            }
            source
        };
        if registry
            .get(&import)
            .is_some_and(|entry| entry.source == source)
        {
            continue;
        }
        let shader = Shader::from_wgsl(
            source.clone(),
            format!("pybevy://gen/{}", hash_shader_path(&source)),
        );
        if let Some(entry) = registry.get_mut(&import)
            && shaders.insert(entry.handle.id(), shader.clone()).is_ok()
        {
            entry.source = source;
            continue;
        }
        registry.insert(
            import,
            GeneratedBindingsShader {
                source,
                handle: shaders.add(shader),
            },
        );
    }
}
