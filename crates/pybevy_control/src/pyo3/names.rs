use std::sync::Arc;

use bevy::ecs::{component::ComponentId, world::World};
use pybevy_core::{
    ComponentBridge, CustomComponentInfo, CustomResourceInfo, ResourceBridge,
    custom_component::CustomComponentRegistry,
    custom_resource::CustomResourceRegistry,
    registry::global_registry::{all_component_bridges, all_resource_bridges},
};
use pyo3::{prelude::*, types::PyType};

use crate::protocol::ControlError;

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

/// Registry entries whose short or qualified name equals the requested name.
struct Matches<'a, T> {
    requested: &'a str,
    targets: Vec<T>,
    choices: Vec<String>,
}

impl<'a, T> Matches<'a, T> {
    fn new(requested: &'a str) -> Self {
        Self {
            requested,
            targets: Vec::new(),
            choices: Vec::new(),
        }
    }

    /// Qualified names contain a `.`, so `qualified` runs only when it can match
    /// or when a short-name match needs it to name the choice.
    fn offer(&mut self, name: &str, qualified: impl FnOnce() -> Option<String>, target: T) {
        let qualified = (self.requested.contains('.') || name == self.requested)
            .then(qualified)
            .flatten();
        if name == self.requested || qualified.as_deref() == Some(self.requested) {
            self.choices
                .push(qualified.unwrap_or_else(|| name.to_string()));
            self.targets.push(target);
        }
    }

    fn finish(mut self, kind: &str) -> Result<Option<T>, ControlError> {
        if self.targets.len() > 1 {
            self.choices.sort_unstable();
            self.choices.dedup();
            return Err(ControlError::invalid_params(format!(
                "Ambiguous {kind} '{}'. Use a qualified name: {}",
                self.requested,
                self.choices.join(", ")
            )));
        }
        Ok(self.targets.pop())
    }
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
    let mut matches = Matches::new(requested);
    for bridge in all_component_bridges() {
        matches.offer(
            bridge.name(),
            || Python::attach(|py| qualified_type_name(&bridge.py_type(py))),
            ComponentTarget::Native(bridge.clone()),
        );
    }
    if let Some(info) = world.get_resource::<CustomComponentInfo>() {
        for (id, entry) in info.iter() {
            matches.offer(
                &entry.name,
                || custom_component_name(world, id),
                ComponentTarget::Custom(id),
            );
        }
    }
    matches.finish("component")
}

pub(super) fn resolve_resource(
    world: &World,
    requested: &str,
) -> Result<Option<ResourceTarget>, ControlError> {
    let mut matches = Matches::new(requested);
    for bridge in all_resource_bridges() {
        matches.offer(
            bridge.name(),
            || Python::attach(|py| qualified_type_name(&bridge.py_type(py))),
            ResourceTarget::Native(bridge.clone()),
        );
    }
    if let Some(info) = world.get_resource::<CustomResourceInfo>() {
        for (id, entry) in info.iter() {
            matches.offer(
                &entry.name,
                || custom_resource_name(world, id),
                ResourceTarget::Custom(id),
            );
        }
    }
    matches.finish("resource")
}
