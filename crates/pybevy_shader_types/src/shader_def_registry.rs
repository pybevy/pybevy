//! Shader-def field names, keyed by the `@material` class that declared them.

use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};

static SHADER_DEF_REGISTRY: OnceLock<RwLock<HashMap<u64, Vec<String>>>> = OnceLock::new();

fn def_registry() -> &'static RwLock<HashMap<u64, Vec<String>>> {
    SHADER_DEF_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(crate) fn get_shader_def_names(material_type_id: u64) -> Option<Vec<String>> {
    def_registry().read().ok()?.get(&material_type_id).cloned()
}

pub(crate) fn register_shader_def_names(material_type_id: u64, names: Vec<String>) {
    if let Ok(mut map) = def_registry().write() {
        map.insert(material_type_id, names);
    }
}

pub(crate) fn clear_shader_def_names() {
    if let Some(reg) = SHADER_DEF_REGISTRY.get()
        && let Ok(mut map) = reg.write()
    {
        map.clear();
    }
}
