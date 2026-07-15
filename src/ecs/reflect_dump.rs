//! Private reflection export used by the parity snapshot oracle.

use std::collections::{BTreeMap, BTreeSet};

use bevy::{
    asset::{ReflectHandle, UntypedAssetId, UntypedHandle},
    ecs::reflect::{AppTypeRegistry, ReflectComponent},
    reflect::{
        PartialReflect, ReflectRef, TypeInfo, TypeRegistry,
        serde::{ReflectSerializerProcessor, TypedReflectSerializer},
    },
};
use pybevy_core::registry::global_registry;
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use serde::{Serialize, Serializer};
use serde_json::Value;

use super::{PyEntity, world::PyWorld};

struct ParityHandleSerializeProcessor;

#[derive(Serialize)]
#[serde(rename_all = "lowercase", tag = "kind", content = "id")]
enum ParityHandleReference {
    Index(u64),
    Uuid(String),
}

impl ParityHandleReference {
    fn from_handle(handle: &UntypedHandle) -> Self {
        match handle.id() {
            UntypedAssetId::Index { index, .. } => Self::Index(index.to_bits()),
            UntypedAssetId::Uuid { uuid, .. } => Self::Uuid(uuid.as_u128().to_string()),
        }
    }
}

impl ReflectSerializerProcessor for ParityHandleSerializeProcessor {
    fn try_serialize<S>(
        &self,
        value: &dyn PartialReflect,
        registry: &TypeRegistry,
        serializer: S,
    ) -> Result<Result<S::Ok, S>, S::Error>
    where
        S: Serializer,
    {
        let Some(value) = value.try_as_reflect() else {
            return Ok(Err(serializer));
        };
        if let Some(handle) = value.downcast_ref::<UntypedHandle>() {
            return Ok(Ok(
                ParityHandleReference::from_handle(handle).serialize(serializer)?
            ));
        }
        let Some(registration) = registry.get(value.type_id()) else {
            return Ok(Err(serializer));
        };
        let Some(reflect_handle) = registration.data::<ReflectHandle>() else {
            return Ok(Err(serializer));
        };
        let handle = reflect_handle
            .downcast_handle_untyped(value.as_any())
            .expect("ReflectHandle type data must match its registered handle type");
        Ok(Ok(
            ParityHandleReference::from_handle(&handle).serialize(serializer)?
        ))
    }
}

/// Dump requested native components through Bevy reflection.
///
/// This is intentionally private API. The parity harness calls it while its
/// validity-fenced `World` callback is active, so the reflected and Python
/// views are captured without an intervening schedule run.
#[pyfunction(name = "_reflect_component_dump")]
pub fn reflect_component_dump(
    world: &PyWorld,
    entities: Vec<(String, PyEntity)>,
    component_fields: BTreeMap<String, Vec<String>>,
) -> PyResult<String> {
    let world = world.world_mut()?;
    let registry = world
        .get_resource::<AppTypeRegistry>()
        .ok_or_else(|| PyRuntimeError::new_err("AppTypeRegistry is not installed"))?
        .clone();
    let registry = registry.read();
    let handle_processor = ParityHandleSerializeProcessor;

    let requested: BTreeSet<&str> = component_fields.keys().map(String::as_str).collect();
    let mut bridges = global_registry::all_component_bridges();
    bridges.sort_by_key(|bridge| bridge.name());

    let available: BTreeSet<&str> = bridges.iter().map(|bridge| bridge.name()).collect();
    let missing: Vec<&str> = requested.difference(&available).copied().collect();
    if !missing.is_empty() {
        return Err(PyRuntimeError::new_err(format!(
            "reflection dump has no component bridge for: {}",
            missing.join(", ")
        )));
    }

    let mut reflected_types = BTreeMap::<String, Vec<String>>::new();
    let mut unreflected_types = Vec::new();
    for bridge in &bridges {
        if !requested.contains(bridge.name()) {
            continue;
        }
        let registration = registry.get(bridge.bevy_type_id()).ok_or_else(|| {
            PyRuntimeError::new_err(format!("{} is absent from AppTypeRegistry", bridge.name()))
        })?;
        if registration.data::<ReflectComponent>().is_none() {
            unreflected_types.push(bridge.name().to_string());
            continue;
        }
        let fields = match registration.type_info() {
            TypeInfo::Struct(info) => info.iter().map(|field| field.name().to_string()).collect(),
            TypeInfo::TupleStruct(info) => (0..info.field_len())
                .map(|index| index.to_string())
                .collect(),
            TypeInfo::Opaque(_) => {
                unreflected_types.push(bridge.name().to_string());
                continue;
            }
            _ => Vec::new(),
        };
        reflected_types.insert(bridge.name().to_string(), fields);
    }

    let mut reflected_entities = BTreeMap::<String, BTreeMap<String, Value>>::new();
    for (entity_name, py_entity) in entities {
        let entity = world.get_entity(py_entity.0).map_err(|error| {
            PyRuntimeError::new_err(format!("cannot reflect entity {entity_name:?}: {error}"))
        })?;
        let mut components = BTreeMap::new();
        for bridge in &bridges {
            if !requested.contains(bridge.name()) {
                continue;
            }
            if !reflected_types.contains_key(bridge.name()) {
                continue;
            }
            if component_fields
                .get(bridge.name())
                .is_none_or(Vec::is_empty)
            {
                continue;
            }
            let registration = registry.get(bridge.bevy_type_id()).ok_or_else(|| {
                PyRuntimeError::new_err(format!("{} is absent from AppTypeRegistry", bridge.name()))
            })?;
            let reflect_component = registration.data::<ReflectComponent>().ok_or_else(|| {
                PyRuntimeError::new_err(format!(
                    "{} has no ReflectComponent registration",
                    bridge.name()
                ))
            })?;
            let Some(value) = reflect_component.reflect(entity) else {
                continue;
            };
            let requested_fields = &component_fields[bridge.name()];
            let mut serialized_fields = serde_json::Map::new();
            for field_name in requested_fields {
                let field = match value.reflect_ref() {
                    ReflectRef::Struct(value) => value.field(field_name),
                    ReflectRef::TupleStruct(value) => field_name
                        .parse::<usize>()
                        .ok()
                        .and_then(|index| value.field(index)),
                    _ => None,
                }
                .ok_or_else(|| {
                    PyRuntimeError::new_err(format!(
                        "reflected {} has no requested field {field_name:?}",
                        bridge.name()
                    ))
                })?;
                let serialized = serde_json::to_value(TypedReflectSerializer::with_processor(
                    field,
                    &registry,
                    &handle_processor,
                ))
                .map_err(|error| {
                    PyRuntimeError::new_err(format!(
                        "failed to serialize reflected {}.{field_name} on {entity_name:?}: {error}",
                        bridge.name()
                    ))
                })?;
                serialized_fields.insert(field_name.clone(), serialized);
            }
            components.insert(bridge.name().to_string(), Value::Object(serialized_fields));
        }
        reflected_entities.insert(entity_name, components);
    }

    serde_json::to_string(&serde_json::json!({
        "types": reflected_types,
        "unreflected_types": unreflected_types,
        "entities": reflected_entities,
    }))
    .map_err(|error| PyRuntimeError::new_err(error.to_string()))
}
