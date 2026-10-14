use std::sync::Arc;

use bevy::ecs::{component::ComponentId, world::World};
use pybevy_core::{
    ComponentBridge, CustomComponentInfo, CustomResourceInfo, ResourceBridge,
    custom_component::CustomComponentRegistry,
    custom_resource::CustomResourceRegistry,
    registry::global_registry::{all_component_bridges, all_resource_bridges},
};
use pyo3::{prelude::*, types::PyType};

use crate::bridge::ControlError;

pub(super) enum ComponentTarget {
    Native(Arc<dyn ComponentBridge>),
    Custom(ComponentId),
}

pub(super) enum ResourceTarget {
    Native(Arc<dyn ResourceBridge>),
    Custom(ComponentId),
}

pub(super) fn qualified_type_name(class: &Bound<'_, PyType>) -> Option<String> {
    let module = class.getattr("__module__").ok()?.extract::<String>().ok()?;
    let qualname = class
        .getattr("__qualname__")
        .ok()?
        .extract::<String>()
        .ok()?;
    Some(format!("{module}.{qualname}"))
}

fn ambiguous(kind: &str, requested: &str, choices: &mut Vec<String>) -> ControlError {
    choices.sort_unstable();
    choices.dedup();
    ControlError::invalid_params(format!(
        "Ambiguous {kind} '{requested}'. Use a qualified name: {}",
        choices.join(", ")
    ))
}

pub(super) fn custom_component_name(world: &World, id: ComponentId) -> Option<String> {
    world
        .get_resource::<CustomComponentRegistry>()?
        .ids_by_qualified_name()
        .find(|(_, candidate)| *candidate == id)
        .map(|(name, _)| name.to_string())
}

pub(super) fn custom_resource_name(world: &World, id: ComponentId) -> Option<String> {
    world
        .get_resource::<CustomResourceRegistry>()?
        .ids_by_qualified_name()
        .find(|(_, candidate)| *candidate == id)
        .map(|(name, _)| name.to_string())
}

pub(super) fn resolve_component(
    world: &World,
    requested: &str,
) -> Result<Option<ComponentTarget>, ControlError> {
    let qualified = requested.contains('.');
    let mut matches = Vec::new();
    let mut choices = Vec::new();

    for bridge in all_component_bridges() {
        let native_name = if qualified || bridge.name() == requested {
            Python::attach(|py| qualified_type_name(&bridge.py_type(py)))
        } else {
            None
        };
        if bridge.name() == requested || native_name.as_deref() == Some(requested) {
            choices.push(native_name.unwrap_or_else(|| bridge.name().to_string()));
            matches.push(ComponentTarget::Native(bridge));
        }
    }

    if let Some(info) = world.get_resource::<CustomComponentInfo>() {
        for (id, entry) in info.iter() {
            let full_name = custom_component_name(world, id);
            if entry.name == requested || full_name.as_deref() == Some(requested) {
                choices.push(full_name.unwrap_or_else(|| entry.name.clone()));
                matches.push(ComponentTarget::Custom(id));
            }
        }
    }

    if matches.len() > 1 {
        return Err(ambiguous("component", requested, &mut choices));
    }
    Ok(matches.pop())
}

pub(super) fn resolve_resource(
    world: &World,
    requested: &str,
) -> Result<Option<ResourceTarget>, ControlError> {
    let qualified = requested.contains('.');
    let mut matches = Vec::new();
    let mut choices = Vec::new();

    for bridge in all_resource_bridges() {
        let native_name = if qualified || bridge.name() == requested {
            Python::attach(|py| qualified_type_name(&bridge.py_type(py)))
        } else {
            None
        };
        if bridge.name() == requested || native_name.as_deref() == Some(requested) {
            choices.push(native_name.unwrap_or_else(|| bridge.name().to_string()));
            matches.push(ResourceTarget::Native(bridge));
        }
    }

    if let Some(info) = world.get_resource::<CustomResourceInfo>() {
        for (id, entry) in info.iter() {
            let full_name = custom_resource_name(world, id);
            if entry.name == requested || full_name.as_deref() == Some(requested) {
                choices.push(full_name.unwrap_or_else(|| entry.name.clone()));
                matches.push(ResourceTarget::Custom(id));
            }
        }
    }

    if matches.len() > 1 {
        return Err(ambiguous("resource", requested, &mut choices));
    }
    Ok(matches.pop())
}
