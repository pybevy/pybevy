//! Validation shared by raw and decorated material shader definitions.

use std::{
    collections::{HashMap, HashSet},
    sync::{OnceLock, RwLock},
};

use bevy::{render::render_resource::RenderPipelineDescriptor, shader::ShaderDefVal};

#[derive(Default)]
struct Registry {
    next: u64,
    ids: HashMap<Vec<String>, u64>,
    names: HashMap<u64, Vec<String>>,
}
static REGISTRY: OnceLock<RwLock<Registry>> = OnceLock::new();
fn registry() -> &'static RwLock<Registry> {
    REGISTRY.get_or_init(Default::default)
}
pub(crate) fn intern_names(names: &[String]) -> u64 {
    let mut registry = registry().write().unwrap_or_else(|e| e.into_inner());
    if let Some(id) = registry.ids.get(names) {
        return *id;
    }
    registry.next = registry
        .next
        .checked_add(1)
        .expect("Shader definition identity exhausted");
    let id = registry.next;
    registry.ids.insert(names.to_vec(), id);
    registry.names.insert(id, names.to_vec());
    id
}
pub(crate) fn get_names(id: u64) -> Vec<String> {
    registry()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .names
        .get(&id)
        .cloned()
        .unwrap_or_default()
}
pub(crate) fn clear_names() {
    let mut registry = registry().write().unwrap_or_else(|e| e.into_inner());
    registry.ids.clear();
    registry.names.clear();
}

pub fn validate_names(names: &[String]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for name in names {
        let mut chars = name.chars();
        if !chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(format!(
                "Shader define '{name}' must be a nonempty ASCII identifier"
            ));
        }
        if !seen.insert(name) {
            return Err(format!("Duplicate shader define '{name}'"));
        }
        if name.starts_with("VERTEX_")
            || name.starts_with("PYBEVY_")
            || matches!(
                name.as_str(),
                "PREPASS_PIPELINE"
                    | "DEFERRED_PREPASS"
                    | "DEPTH_PREPASS"
                    | "NORMAL_PREPASS"
                    | "MOTION_VECTOR_PREPASS"
                    | "MESH_PIPELINE"
                    | "MATERIAL_BIND_GROUP"
                    | "MATERIAL_BIND_GROUP_INDEX"
            )
        {
            return Err(format!(
                "Shader define '{name}' is reserved by the rendering pipeline"
            ));
        }
    }
    Ok(())
}

pub(crate) fn pipeline_collision<'a>(
    names: &'a [String],
    descriptor: &RenderPipelineDescriptor,
) -> Option<&'a str> {
    names
        .iter()
        .find(|name| {
            descriptor
                .vertex
                .shader_defs
                .iter()
                .chain(
                    descriptor
                        .fragment
                        .iter()
                        .flat_map(|f| f.shader_defs.iter()),
                )
                .any(|definition| match definition {
                    ShaderDefVal::Bool(existing, _)
                    | ShaderDefVal::Int(existing, _)
                    | ShaderDefVal::UInt(existing, _) => existing == *name,
                })
        })
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_names_have_content_identity_independent_of_classes() {
        let a = vec!["TEST_ORDER_A".into(), "TEST_ORDER_B".into()];
        let b = vec!["TEST_ORDER_B".into(), "TEST_ORDER_A".into()];
        let first = intern_names(&a);
        assert_eq!(intern_names(&a), first);
        assert_ne!(intern_names(&b), first);
        assert_eq!(get_names(first), a);
    }

    #[test]
    fn invalid_and_reserved_names_are_rejected() {
        for names in [
            vec!["".into()],
            vec!["BAD NAME".into()],
            vec!["ON".into(), "ON".into()],
            vec!["PREPASS_PIPELINE".into()],
            vec!["VERTEX_FUTURE".into()],
        ] {
            assert!(validate_names(&names).is_err());
        }
        assert!(validate_names(&["HAS_CUSTOM_DETAIL".into()]).is_ok());
    }
}
