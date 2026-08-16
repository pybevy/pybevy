//! Bevy reflection adapter for wrapper-stored Python `@component` values.

use std::any::TypeId;

use bevy::{
    ecs::{
        component::Component,
        entity::{Entity, EntityMapper},
        reflect::{ReflectComponent, ReflectComponentFns},
        relationship::RelationshipHookMode,
        world::EntityWorldMut,
    },
    log::warn,
    math::{Vec2, Vec3},
    prelude::World,
    reflect::{FromReflect, PartialReflect, Reflect, TypeRegistry},
};
use pybevy_core::{
    component_layout::PrimitiveValue, component_wrapper::insert_wrapper_bytes,
    custom_component::CustomComponentRegistry,
};

/// One reflected value variant for every primitive supported by wrapper storage.
#[derive(Clone, Debug, PartialEq, Reflect)]
enum ReflectedPrimitiveValue {
    F32(f32),
    F64(f64),
    I32(i32),
    I64(i64),
    U32(u32),
    U64(u64),
    Bool(bool),
    Vec3 { x: f32, y: f32, z: f32 },
    Vec2 { x: f32, y: f32 },
}

impl From<PrimitiveValue> for ReflectedPrimitiveValue {
    fn from(value: PrimitiveValue) -> Self {
        match value {
            PrimitiveValue::F32(value) => Self::F32(value),
            PrimitiveValue::F64(value) => Self::F64(value),
            PrimitiveValue::I32(value) => Self::I32(value),
            PrimitiveValue::I64(value) => Self::I64(value),
            PrimitiveValue::U32(value) => Self::U32(value),
            PrimitiveValue::U64(value) => Self::U64(value),
            PrimitiveValue::Bool(value) => Self::Bool(value),
            PrimitiveValue::Vec3(value) => Self::Vec3 {
                x: value.x,
                y: value.y,
                z: value.z,
            },
            PrimitiveValue::Vec2(value) => Self::Vec2 {
                x: value.x,
                y: value.y,
            },
        }
    }
}

impl From<ReflectedPrimitiveValue> for PrimitiveValue {
    fn from(value: ReflectedPrimitiveValue) -> Self {
        match value {
            ReflectedPrimitiveValue::F32(value) => Self::F32(value),
            ReflectedPrimitiveValue::F64(value) => Self::F64(value),
            ReflectedPrimitiveValue::I32(value) => Self::I32(value),
            ReflectedPrimitiveValue::I64(value) => Self::I64(value),
            ReflectedPrimitiveValue::U32(value) => Self::U32(value),
            ReflectedPrimitiveValue::U64(value) => Self::U64(value),
            ReflectedPrimitiveValue::Bool(value) => Self::Bool(value),
            ReflectedPrimitiveValue::Vec3 { x, y, z } => Self::Vec3(Vec3::new(x, y, z)),
            ReflectedPrimitiveValue::Vec2 { x, y } => Self::Vec2(Vec2::new(x, y)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Reflect)]
struct ReflectedPythonField {
    name: String,
    value: ReflectedPrimitiveValue,
}

#[derive(Clone, Debug, PartialEq, Reflect)]
struct ReflectedPythonComponent {
    qualified_name: String,
    fields: Vec<ReflectedPythonField>,
}

/// Static reflected envelope grouping all Python wrapper components on one entity.
///
/// Bevy reflection is keyed by Rust `TypeId`, while Python component classes are
/// registered dynamically with one Bevy `ComponentId` per class. Grouping their
/// owned snapshots into one ordinary reflected component bridges those identity
/// models without changing query or storage semantics in the live World.
#[derive(Clone, Component, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub(crate) struct ReflectedPythonComponents {
    components: Vec<ReflectedPythonComponent>,
}

/// Register the envelope and its custom materialization behavior.
pub(crate) fn register_custom_component_reflection(registry: &mut TypeRegistry) {
    if registry
        .get(TypeId::of::<ReflectedPythonComponents>())
        .is_none()
    {
        registry.register::<ReflectedPythonComponents>();
    }
    // Type presence alone does not prove that our custom materializer is still
    // installed: ordinary registration supplies the derived component functions.
    // Replacing the type data is cheap and keeps every live-world entry point safe.
    let registration = registry
        .get_mut(TypeId::of::<ReflectedPythonComponents>())
        .expect("the reflected Python component envelope is registered");
    let mut functions = ReflectComponentFns::new::<ReflectedPythonComponents>();
    functions.insert = materialize_insert;
    functions.apply_or_insert_mapped = materialize_apply_or_insert;
    registration.insert(ReflectComponent::new(functions));
}

fn reflected_snapshot(component: &dyn PartialReflect) -> ReflectedPythonComponents {
    ReflectedPythonComponents::from_reflect(component).unwrap_or_else(|| {
        panic!(
            "reflected value for {} has an incompatible shape",
            std::any::type_name::<ReflectedPythonComponents>()
        )
    })
}

fn materialize_insert(
    entity: &mut EntityWorldMut<'_>,
    component: &dyn PartialReflect,
    _registry: &TypeRegistry,
) {
    materialize(entity, reflected_snapshot(component));
}

fn materialize_apply_or_insert(
    entity: &mut EntityWorldMut<'_>,
    component: &dyn PartialReflect,
    _registry: &TypeRegistry,
    _mapper: &mut dyn EntityMapper,
    _relationship_hook_mode: RelationshipHookMode,
) {
    materialize(entity, reflected_snapshot(component));
}

fn materialize(entity: &mut EntityWorldMut<'_>, snapshot: ReflectedPythonComponents) {
    for component in snapshot.components {
        let Some((component_id, schema)) = entity
            .world()
            .get_resource::<CustomComponentRegistry>()
            .and_then(|registry| {
                let id = registry.id_by_qualified_name(&component.qualified_name)?;
                Some((id, registry.wrapper_schema(id)?.clone()))
            })
        else {
            warn!(
                "skipping reflected Python component `{}` because its wrapper schema is not registered",
                component.qualified_name
            );
            continue;
        };

        let fields = component
            .fields
            .into_iter()
            .map(|field| (field.name, PrimitiveValue::from(field.value)))
            .collect::<Vec<_>>();
        let bytes = match schema.serialize_values(&fields) {
            Ok(bytes) => bytes,
            Err(error) => {
                warn!(
                    "skipping reflected Python component `{}` because its saved schema does not match the registered class: {error}",
                    component.qualified_name
                );
                continue;
            }
        };
        if let Err(error) = insert_wrapper_bytes(entity, component_id, schema.wrapper_size, &bytes)
        {
            warn!(
                "skipping reflected Python component `{}` because insertion failed: {error}",
                component.qualified_name
            );
        }
    }
}

/// Extract all wrapper-stored Python components present on one entity.
pub(crate) fn extract_custom_components(
    world: &World,
    entity: Entity,
) -> Result<Option<ReflectedPythonComponents>, String> {
    let Some(registry) = world.get_resource::<CustomComponentRegistry>() else {
        return Ok(None);
    };
    let entity_ref = world
        .get_entity(entity)
        .map_err(|error| format!("entity {entity:?} disappeared during extraction: {error}"))?;
    let mut registered = registry
        .ids_by_qualified_name()
        .filter_map(|(name, id)| {
            registry
                .wrapper_schema(id)
                .map(|schema| (name.to_string(), id, schema))
        })
        .collect::<Vec<_>>();
    registered.sort_by(|left, right| left.0.cmp(&right.0));

    let mut components = Vec::new();
    for (qualified_name, component_id, schema) in registered {
        let Ok(ptr) = entity_ref.get_by_id(component_id) else {
            continue;
        };
        let descriptor = world
            .components()
            .get_info(component_id)
            .ok_or_else(|| format!("component `{qualified_name}` has no Bevy descriptor"))?;
        if descriptor.layout() != schema.wrapper_size.mem_layout() {
            return Err(format!(
                "component `{qualified_name}` wrapper schema does not match its Bevy descriptor"
            ));
        }
        // SAFETY: the descriptor layout was checked against the wrapper size.
        let data = unsafe { schema.wrapper_size.get_ref_ptr_as_mut(ptr) } as *const u8;
        // SAFETY: `data` addresses the wrapper allocation checked above.
        let fields = unsafe { schema.read_values(data) }
            .map_err(|error| format!("component `{qualified_name}`: {error}"))?
            .into_iter()
            .map(|(name, value)| ReflectedPythonField {
                name,
                value: value.into(),
            })
            .collect();
        components.push(ReflectedPythonComponent {
            qualified_name,
            fields,
        });
    }

    Ok((!components.is_empty()).then_some(ReflectedPythonComponents { components }))
}
