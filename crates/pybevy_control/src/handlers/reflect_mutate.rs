use std::any::TypeId;

use bevy::{
    ecs::{
        entity::Entity,
        reflect::{AppTypeRegistry, ReflectComponent},
        world::World,
    },
    prelude::ReflectDefault,
    reflect::{
        PartialReflect, ReflectMut, ReflectRef, TypeInfo, TypeRegistry,
        enums::{DynamicEnum, DynamicVariant, EnumInfo, VariantInfo, VariantType},
        list::DynamicList,
        structs::{DynamicStruct, StructInfo},
        tuple::DynamicTuple,
        tuple_struct::DynamicTupleStruct,
    },
};
use serde_json::{Map, Value};

use super::json_float::{float_to_json, nonfinite_float_from_json};

/// Errors from reflection-based mutation
#[derive(Debug)]
pub enum ReflectError {
    /// TypeId not in AppTypeRegistry — fall back to Python
    NotRegistered,
    /// Type registered but lacks ReflectComponent data — fall back to Python
    NoReflectComponent,
    /// Can't create default for spawn — fall back to Python
    NoReflectDefault,
    /// Type is not a struct — fall back to Python
    NotAStruct,
    /// Entity doesn't have this component
    ComponentNotOnEntity,
    /// Field-level mutation error
    FieldError(String),
}

/// Try reflection-based field mutation on a component.
/// Returns each written field mapped to its post-write JSON value.
pub fn reflect_set_component(
    world: &mut World,
    entity: Entity,
    type_id: TypeId,
    fields: &Map<String, Value>,
) -> Result<Map<String, Value>, ReflectError> {
    // Clone AppTypeRegistry Arc — cheap, avoids borrow conflict with entity_mut later
    let registry_arc = world
        .get_resource::<AppTypeRegistry>()
        .ok_or(ReflectError::NotRegistered)?
        .clone();
    let type_registry = registry_arc.read();

    let registration = type_registry
        .get(type_id)
        .ok_or(ReflectError::NotRegistered)?;

    let reflect_component = registration
        .data::<ReflectComponent>()
        .ok_or(ReflectError::NoReflectComponent)?
        .clone();

    let type_info = registration.type_info();
    if matches!(type_info, TypeInfo::Enum(_)) {
        let replacement = enum_component_replacement(fields, type_id, type_info, &type_registry)?;

        let entity_ref = world
            .get_entity(entity)
            .map_err(|_| ReflectError::ComponentNotOnEntity)?;
        let mut candidate = reflect_component
            .reflect(entity_ref)
            .ok_or(ReflectError::ComponentNotOnEntity)?
            .reflect_clone()
            .map_err(|error| {
                ReflectError::FieldError(format!(
                    "variant: component cannot be cloned for an atomic update: {error}"
                ))
            })?;

        // A reflected enum may accept an incomplete dynamic variant while a
        // concrete enum rejects it. Validate against a detached concrete clone
        // before calling ReflectComponent::apply, whose implementation is
        // infallible and would otherwise panic on attacker-controlled input.
        candidate
            .try_apply(replacement.as_ref())
            .map_err(|error| ReflectError::FieldError(format!("variant: {error}")))?;

        let post = match candidate.reflect_ref() {
            ReflectRef::Enum(value) => Value::String(value.variant_name().to_string()),
            _ => Value::Null,
        };

        drop(type_registry);
        let entity_mut = world
            .get_entity_mut(entity)
            .map_err(|_| ReflectError::ComponentNotOnEntity)?;
        reflect_component.apply(entity_mut, candidate.as_partial_reflect());

        // get_component reports `variant` as a bare name, and set_component
        // accepts that same form, so the post-state has to use it too.
        return Ok(Map::from_iter([("variant".to_string(), post)]));
    }

    let struct_info = match type_info {
        TypeInfo::Struct(info) => info,
        _ => {
            return Err(ReflectError::NotAStruct);
        }
    };

    // Pre-convert all JSON field values to reflected values while we hold the registry lock
    let mut converted: Vec<(String, Box<dyn PartialReflect>)> = Vec::with_capacity(fields.len());
    for (field_name, field_value) in fields {
        // Resolve the actual Bevy field name (handles Python reserved word aliasing)
        let actual_name = resolve_field_name(field_name, struct_info)
            .ok_or_else(|| ReflectError::FieldError(format!("{field_name}: field not found")))?;
        let reflected = json_field_to_reflect(field_name, field_value, struct_info, &type_registry)
            .map_err(|e| ReflectError::FieldError(format!("{field_name}: {e}")))?;
        converted.push((actual_name, reflected));
    }

    let entity_ref = world
        .get_entity(entity)
        .map_err(|_| ReflectError::ComponentNotOnEntity)?;
    let mut candidate = reflect_component
        .reflect(entity_ref)
        .ok_or(ReflectError::ComponentNotOnEntity)?
        .to_dynamic();

    // Apply every field to a detached candidate. `try_apply` may leave its target
    // partially modified on error, so it must not operate on the live component.
    let mut updated_names = Vec::with_capacity(converted.len());
    match candidate.reflect_mut() {
        ReflectMut::Struct(s) => {
            for (name, value) in converted {
                if let Some(field) = s.field_mut(&name) {
                    field
                        .try_apply(value.as_ref())
                        .map_err(|e| ReflectError::FieldError(format!("{name}: {e}")))?;
                } else {
                    return Err(ReflectError::FieldError(format!(
                        "{name}: field not found on component"
                    )));
                }
                updated_names.push(name);
            }
        }
        _ => {
            return Err(ReflectError::NotAStruct);
        }
    }

    drop(type_registry);
    let entity_mut = world
        .get_entity_mut(entity)
        .map_err(|_| ReflectError::ComponentNotOnEntity)?;
    reflect_component.apply(entity_mut, candidate.as_ref());

    let entity_ref = world
        .get_entity(entity)
        .map_err(|_| ReflectError::ComponentNotOnEntity)?;
    let reflected = reflect_component
        .reflect(entity_ref)
        .ok_or(ReflectError::ComponentNotOnEntity)?;
    let ReflectRef::Struct(fields) = reflected.reflect_ref() else {
        return Err(ReflectError::NotAStruct);
    };
    let updated = updated_names
        .into_iter()
        .map(|name| {
            let value = fields.field(&name).map_or(Value::Null, reflect_to_json);
            (name, value)
        })
        .collect();

    Ok(updated)
}

/// Component-shaped math types that `scene::vector_to_json` renders as arrays.
/// Keep this list in lockstep with that function so a field looks the same
/// whether it came from get_component or from a set_component post-state.
fn vector_field_names(short_type_path: &str) -> Option<&'static [&'static str]> {
    match short_type_path {
        "Vec2" => Some(&["x", "y"]),
        "Vec3" => Some(&["x", "y", "z"]),
        "Vec4" | "Quat" => Some(&["x", "y", "z", "w"]),
        _ => None,
    }
}

/// Read named fields off a component via reflection, for reporting the state
/// after a write that did not go through `reflect_set_component`.
pub fn reflect_read_fields<'a>(
    world: &World,
    entity: Entity,
    type_id: TypeId,
    names: impl IntoIterator<Item = &'a String>,
) -> Map<String, Value> {
    let mut out = Map::new();
    let Some(registry_arc) = world.get_resource::<AppTypeRegistry>() else {
        return out;
    };
    let registry = registry_arc.read();
    let component = registry
        .get(type_id)
        .and_then(|registration| registration.data::<ReflectComponent>())
        .and_then(|reflect| {
            world
                .get_entity(entity)
                .ok()
                .and_then(|e| reflect.reflect(e))
        });
    let Some(component) = component else {
        return out;
    };
    let fields = match component.reflect_ref() {
        ReflectRef::Struct(fields) => fields,
        // An enum component has no named fields; it reports the same bare `variant`
        // name that get_component returns and set_component accepts.
        ReflectRef::Enum(value) => {
            let variant = Value::String(value.variant_name().to_string());
            for name in names {
                let value = if name == "variant" {
                    variant.clone()
                } else {
                    Value::Null
                };
                out.insert(name.clone(), value);
            }
            return out;
        }
        _ => return out,
    };
    for name in names {
        let value = fields.field(name).map_or(Value::Null, reflect_to_json);
        out.insert(name.clone(), value);
    }
    out
}

/// Convert a reflected value to a JSON value for post-write read-back.
/// Best-effort: numeric primitives pass through, structs/tuples/lists/enums recurse.
/// Falls back to Value::Null when the type is not representable.
pub fn reflect_to_json(value: &dyn PartialReflect) -> Value {
    // Primitives via try_downcast_ref
    if let Some(v) = value.try_downcast_ref::<f32>() {
        return float_to_json(f64::from(*v));
    }
    if let Some(v) = value.try_downcast_ref::<f64>() {
        return float_to_json(*v);
    }
    if let Some(v) = value.try_downcast_ref::<i8>() {
        return serde_json::json!(*v);
    }
    if let Some(v) = value.try_downcast_ref::<i16>() {
        return serde_json::json!(*v);
    }
    if let Some(v) = value.try_downcast_ref::<i32>() {
        return serde_json::json!(*v);
    }
    if let Some(v) = value.try_downcast_ref::<i64>() {
        return serde_json::json!(*v);
    }
    if let Some(v) = value.try_downcast_ref::<isize>() {
        return serde_json::json!(*v as i64);
    }
    if let Some(v) = value.try_downcast_ref::<u8>() {
        return serde_json::json!(*v);
    }
    if let Some(v) = value.try_downcast_ref::<u16>() {
        return serde_json::json!(*v);
    }
    if let Some(v) = value.try_downcast_ref::<u32>() {
        return serde_json::json!(*v);
    }
    if let Some(v) = value.try_downcast_ref::<u64>() {
        return serde_json::json!(*v);
    }
    if let Some(v) = value.try_downcast_ref::<usize>() {
        return serde_json::json!(*v as u64);
    }
    if let Some(v) = value.try_downcast_ref::<bool>() {
        return serde_json::json!(*v);
    }
    if let Some(v) = value.try_downcast_ref::<String>() {
        return Value::String(v.clone());
    }

    match value.reflect_ref() {
        ReflectRef::Struct(s) => {
            // Math vectors serialize as arrays, matching what get_component
            // reports and what set_component accepts back.
            if let Some(names) = vector_field_names(value.reflect_short_type_path()) {
                return Value::Array(
                    names
                        .iter()
                        .map(|name| s.field(name).map_or(Value::Null, reflect_to_json))
                        .collect(),
                );
            }
            let mut map = Map::new();
            for i in 0..s.field_len() {
                let name = s.name_at(i).unwrap_or("").to_string();
                let v = s.field_at(i).map(reflect_to_json).unwrap_or(Value::Null);
                map.insert(name, v);
            }
            Value::Object(map)
        }
        ReflectRef::TupleStruct(t) => {
            let mut arr = Vec::with_capacity(t.field_len());
            for i in 0..t.field_len() {
                arr.push(t.field(i).map(reflect_to_json).unwrap_or(Value::Null));
            }
            Value::Array(arr)
        }
        ReflectRef::Tuple(t) => {
            let mut arr = Vec::with_capacity(t.field_len());
            for i in 0..t.field_len() {
                arr.push(t.field(i).map(reflect_to_json).unwrap_or(Value::Null));
            }
            Value::Array(arr)
        }
        ReflectRef::List(l) => {
            let mut arr = Vec::with_capacity(l.len());
            for i in 0..l.len() {
                arr.push(l.get(i).map(reflect_to_json).unwrap_or(Value::Null));
            }
            Value::Array(arr)
        }
        ReflectRef::Array(a) => {
            let mut arr = Vec::with_capacity(a.len());
            for i in 0..a.len() {
                arr.push(a.get(i).map(reflect_to_json).unwrap_or(Value::Null));
            }
            Value::Array(arr)
        }
        ReflectRef::Enum(e) => {
            let variant_name = e.variant_name().to_string();
            // Option<T>: unwrap None to Value::Null, Some(x) to inner JSON
            if variant_name == "None" {
                return Value::Null;
            }
            if variant_name == "Some" && e.field_len() == 1 {
                return e.field_at(0).map(reflect_to_json).unwrap_or(Value::Null);
            }
            let inner = match e.variant_type() {
                VariantType::Unit => Value::Null,
                VariantType::Tuple => {
                    if e.field_len() == 1 {
                        e.field_at(0).map(reflect_to_json).unwrap_or(Value::Null)
                    } else {
                        let mut arr = Vec::with_capacity(e.field_len());
                        for i in 0..e.field_len() {
                            arr.push(e.field_at(i).map(reflect_to_json).unwrap_or(Value::Null));
                        }
                        Value::Array(arr)
                    }
                }
                VariantType::Struct => {
                    let mut map = Map::new();
                    for i in 0..e.field_len() {
                        let name = e.name_at(i).unwrap_or("").to_string();
                        let v = e.field_at(i).map(reflect_to_json).unwrap_or(Value::Null);
                        map.insert(name, v);
                    }
                    Value::Object(map)
                }
            };
            let mut obj = Map::new();
            obj.insert(variant_name, inner);
            Value::Object(obj)
        }
        _ => Value::Null,
    }
}

/// Create a component from JSON fields via reflection and insert it into an entity.
pub fn reflect_spawn_component(
    world: &mut World,
    entity: Entity,
    type_id: TypeId,
    fields: &Map<String, Value>,
) -> Result<(), ReflectError> {
    // Clone AppTypeRegistry Arc
    let registry_arc = world
        .get_resource::<AppTypeRegistry>()
        .ok_or(ReflectError::NotRegistered)?
        .clone();
    let type_registry = registry_arc.read();

    let registration = type_registry
        .get(type_id)
        .ok_or(ReflectError::NotRegistered)?;

    let reflect_component = registration
        .data::<ReflectComponent>()
        .ok_or(ReflectError::NoReflectComponent)?
        .clone();

    let reflect_default = registration
        .data::<ReflectDefault>()
        .ok_or(ReflectError::NoReflectDefault)?
        .clone();

    // Create default component instance
    let mut default_component = reflect_default.default();

    // Apply JSON fields if any
    if !fields.is_empty() {
        let type_info = registration.type_info();
        match type_info {
            TypeInfo::Enum(_) => {
                let replacement =
                    enum_component_replacement(fields, type_id, type_info, &type_registry)?;
                default_component
                    .try_apply(replacement.as_ref())
                    .map_err(|error| ReflectError::FieldError(format!("variant: {error}")))?;
            }
            TypeInfo::Struct(struct_info) => match default_component.reflect_mut() {
                ReflectMut::Struct(s) => {
                    for (field_name, field_value) in fields {
                        let actual_name =
                            resolve_field_name(field_name, struct_info).ok_or_else(|| {
                                ReflectError::FieldError(format!("{field_name}: field not found"))
                            })?;
                        let reflected = json_field_to_reflect(
                            field_name,
                            field_value,
                            struct_info,
                            &type_registry,
                        )
                        .map_err(|e| ReflectError::FieldError(format!("{field_name}: {e}")))?;

                        if let Some(field) = s.field_mut(&actual_name) {
                            field.try_apply(reflected.as_ref()).map_err(|e| {
                                ReflectError::FieldError(format!("{field_name}: {e}"))
                            })?;
                        } else {
                            return Err(ReflectError::FieldError(format!(
                                "{field_name}: field not found on component"
                            )));
                        }
                    }
                }
                _ => return Err(ReflectError::NotAStruct),
            },
            _ => return Err(ReflectError::NotAStruct),
        }
    }

    // Insert the component into the entity
    // registry_arc is independent of world, so holding the read guard is fine
    let mut entity_mut = world
        .get_entity_mut(entity)
        .map_err(|_| ReflectError::ComponentNotOnEntity)?;

    reflect_component.insert(
        &mut entity_mut,
        default_component.as_partial_reflect(),
        &type_registry,
    );

    Ok(())
}

fn enum_component_replacement(
    fields: &Map<String, Value>,
    type_id: TypeId,
    type_info: &'static TypeInfo,
    registry: &TypeRegistry,
) -> Result<Box<dyn PartialReflect>, ReflectError> {
    if fields.contains_key("variant") {
        if fields.len() == 1 {
            return json_to_reflect(
                fields.get("variant").unwrap(),
                type_id,
                Some(type_info),
                registry,
            )
            .map_err(|error| ReflectError::FieldError(format!("variant: {error}")));
        }
        return json_to_reflect(
            &Value::Object(fields.clone()),
            type_id,
            Some(type_info),
            registry,
        )
        .map_err(|error| ReflectError::FieldError(format!("variant: {error}")));
    }

    if fields.len() == 1 {
        return json_to_reflect(
            &Value::Object(fields.clone()),
            type_id,
            Some(type_info),
            registry,
        )
        .map_err(|error| ReflectError::FieldError(format!("variant: {error}")));
    }

    Err(ReflectError::FieldError(
        "enum component updates require a tagged 'variant' payload or one variant key".to_string(),
    ))
}

/// Resolve a field name on a struct, handling Python reserved word aliasing.
/// `global_` → `global` (trailing underscore stripped if exact name not found).
/// Returns the actual Bevy field name from the StructInfo.
fn resolve_field_name(field_name: &str, parent_info: &StructInfo) -> Option<String> {
    if parent_info.field(field_name).is_some() {
        return Some(field_name.to_string());
    }
    if let Some(stripped) = field_name.strip_suffix('_')
        && parent_info.field(stripped).is_some()
    {
        return Some(stripped.to_string());
    }
    None
}

/// Convert a named field's JSON value to a reflected value using the parent struct's type info.
/// Handles Python reserved word aliasing via `resolve_field_name`.
fn json_field_to_reflect(
    field_name: &str,
    value: &Value,
    parent_info: &StructInfo,
    registry: &TypeRegistry,
) -> Result<Box<dyn PartialReflect>, String> {
    let actual_name = resolve_field_name(field_name, parent_info)
        .ok_or_else(|| format!("field '{field_name}' not found"))?;

    let field_info = parent_info.field(&actual_name).unwrap();
    let target_type_id = field_info.type_id();
    let target_type_info = registry.get(target_type_id).map(|r| r.type_info());

    json_to_reflect(value, target_type_id, target_type_info, registry)
}

/// Recursively convert a JSON value to a Box<dyn PartialReflect>.
/// Uses target type info to determine the correct Rust type.
fn json_to_reflect(
    value: &Value,
    target_type_id: TypeId,
    target_type_info: Option<&'static TypeInfo>,
    registry: &TypeRegistry,
) -> Result<Box<dyn PartialReflect>, String> {
    // Handle Option<T>: if target is an Enum with "None" and "Some" variants,
    // unwrap JSON null → None, anything else → Some(inner)
    if let Some(TypeInfo::Enum(enum_info)) = target_type_info
        && is_option_enum(enum_info)
    {
        return convert_option(value, enum_info, target_type_info, registry);
    }

    match value {
        Value::Number(n) => convert_number(n, target_type_id, target_type_info),
        Value::Bool(b) => {
            if target_type_id == TypeId::of::<bool>() {
                Ok(Box::new(*b))
            } else {
                Err(format!(
                    "cannot convert bool {b} to {}",
                    target_type_name(target_type_info)
                ))
            }
        }
        Value::String(s) => {
            if target_type_id == TypeId::of::<f32>()
                && let Some(value) = nonfinite_float_from_json(value)
            {
                return Ok(Box::new(value as f32));
            }
            if target_type_id == TypeId::of::<f64>()
                && let Some(value) = nonfinite_float_from_json(value)
            {
                return Ok(Box::new(value));
            }
            if let Some(TypeInfo::Enum(enum_info)) = target_type_info {
                let variant = enum_info.variant(s).ok_or_else(|| {
                    let valid = (0..enum_info.variant_len())
                        .filter_map(|index| enum_info.variant_at(index))
                        .map(VariantInfo::name)
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("unknown variant '{s}'. Valid variants: {valid}")
                })?;
                return default_enum_variant(s, variant, target_type_info, registry);
            }
            if target_type_id == TypeId::of::<String>() {
                Ok(Box::new(s.clone()))
            } else {
                Err(format!(
                    "cannot convert string {s:?} to {}",
                    target_type_name(target_type_info)
                ))
            }
        }
        Value::Array(arr) => convert_array(arr, target_type_id, target_type_info, registry),
        Value::Object(obj) => convert_object(obj, target_type_id, target_type_info, registry),
        Value::Null => Err("null values not supported".into()),
    }
}

fn default_enum_variant(
    variant_name: &str,
    variant_info: &VariantInfo,
    type_info: Option<&'static TypeInfo>,
    registry: &TypeRegistry,
) -> Result<Box<dyn PartialReflect>, String> {
    let default_field = |type_id: TypeId, field_name: &str| {
        registry
            .get(type_id)
            .and_then(|registration| registration.data::<ReflectDefault>())
            .map(ReflectDefault::default)
            .ok_or_else(|| {
                format!(
                    "variant '{variant_name}' requires field '{field_name}' and its type has no reflected default"
                )
            })
    };

    let variant = match variant_info {
        VariantInfo::Unit(_) => DynamicVariant::Unit,
        VariantInfo::Tuple(info) => {
            let mut tuple = DynamicTuple::default();
            for index in 0..info.field_len() {
                let field = info.field_at(index).ok_or_else(|| {
                    format!("variant '{variant_name}' is missing field metadata at index {index}")
                })?;
                tuple.insert_boxed(default_field(field.type_id(), &format!(".{index}"))?);
            }
            DynamicVariant::Tuple(tuple)
        }
        VariantInfo::Struct(info) => {
            let mut value = DynamicStruct::default();
            for index in 0..info.field_len() {
                let field = info.field_at(index).ok_or_else(|| {
                    format!("variant '{variant_name}' is missing field metadata at index {index}")
                })?;
                value.insert_boxed(field.name(), default_field(field.type_id(), field.name())?);
            }
            DynamicVariant::Struct(value)
        }
    };

    let mut dynamic = DynamicEnum::default();
    dynamic.set_represented_type(type_info);
    dynamic.set_variant(variant_name, variant);
    Ok(Box::new(dynamic))
}

fn target_type_name(target_type_info: Option<&'static TypeInfo>) -> &'static str {
    target_type_info
        .map(|info| info.type_path_table().short_path())
        .unwrap_or("the requested field type")
}

/// Check if an enum type is `Option<T>` (has exactly "None" and "Some" variants).
fn is_option_enum(enum_info: &EnumInfo) -> bool {
    enum_info.variant_len() == 2
        && enum_info.variant("None").is_some()
        && enum_info.variant("Some").is_some()
}

/// Convert a JSON value to an Option<T> reflected enum.
/// JSON null → None variant, anything else → Some(inner_value).
fn convert_option(
    value: &Value,
    enum_info: &EnumInfo,
    type_info: Option<&'static TypeInfo>,
    registry: &TypeRegistry,
) -> Result<Box<dyn PartialReflect>, String> {
    if value.is_null() {
        // None variant
        let mut dynamic = DynamicEnum::default();
        dynamic.set_represented_type(type_info);
        dynamic.set_variant("None", DynamicVariant::Unit);
        return Ok(Box::new(dynamic));
    }

    // Some variant — extract inner type from the Some variant's first field
    let some_variant = enum_info
        .variant("Some")
        .ok_or("Option enum missing 'Some' variant")?;

    let inner_type_id = match some_variant {
        VariantInfo::Tuple(tuple_info) if tuple_info.field_len() == 1 => {
            tuple_info.field_at(0).unwrap().type_id()
        }
        _ => return Err("Option 'Some' variant has unexpected shape".into()),
    };

    let inner_type_info = registry.get(inner_type_id).map(|r| r.type_info());
    let inner_value = json_to_reflect(value, inner_type_id, inner_type_info, registry)?;

    let mut tuple = DynamicTuple::default();
    tuple.insert_boxed(inner_value);

    let mut dynamic = DynamicEnum::default();
    dynamic.set_represented_type(type_info);
    dynamic.set_variant("Some", DynamicVariant::Tuple(tuple));
    Ok(Box::new(dynamic))
}

/// Convert a JSON array to a reflected struct value.
/// Supports array shorthand: [1, 2, 3] → Vec3 { x: 1.0, y: 2.0, z: 3.0 }
fn convert_array(
    arr: &[Value],
    _target_type_id: TypeId,
    target_type_info: Option<&'static TypeInfo>,
    registry: &TypeRegistry,
) -> Result<Box<dyn PartialReflect>, String> {
    if let Some(TypeInfo::Struct(info)) = target_type_info {
        if info.field_len() == arr.len() {
            return build_dynamic_struct_from_fields(info, target_type_info, |i| {
                let field_info = info.field_at(i).unwrap();
                let field_type_id = field_info.type_id();
                let field_type_info = registry.get(field_type_id).map(|r| r.type_info());
                json_to_reflect(&arr[i], field_type_id, field_type_info, registry)
            });
        }
        return Err(format!(
            "array length {} doesn't match struct field count {}",
            arr.len(),
            info.field_len()
        ));
    }

    // TupleStruct shorthand: [x, y] → TupleStruct(x, y) (e.g., UVec2, IVec2)
    if let Some(TypeInfo::TupleStruct(info)) = target_type_info {
        if info.field_len() == arr.len() {
            let mut dynamic = DynamicTupleStruct::default();
            dynamic.set_represented_type(target_type_info);
            for (i, arr_item) in arr.iter().enumerate().take(info.field_len()) {
                let field_info = info.field_at(i).unwrap();
                let field_type_id = field_info.type_id();
                let field_type_info = registry.get(field_type_id).map(|r| r.type_info());
                let value = json_to_reflect(arr_item, field_type_id, field_type_info, registry)?;
                dynamic.insert_boxed(value);
            }
            return Ok(Box::new(dynamic));
        }
        return Err(format!(
            "array length {} doesn't match tuple struct field count {}",
            arr.len(),
            info.field_len()
        ));
    }

    if let Some(TypeInfo::List(list_info)) = target_type_info {
        let item_type_id = list_info.item_ty().id();
        let item_type_info = registry.get(item_type_id).map(|r| r.type_info());

        let mut dynamic_list = DynamicList::default();
        dynamic_list.set_represented_type(target_type_info);
        for item in arr {
            let reflected = json_to_reflect(item, item_type_id, item_type_info, registry)?;
            dynamic_list.push_box(reflected);
        }
        return Ok(Box::new(dynamic_list));
    }

    Err(format!(
        "cannot convert {}-element array to non-struct type",
        arr.len()
    ))
}

/// Convert a JSON object to a reflected struct or enum value.
///
/// For structs: `{"field": value, ...}` → `DynamicStruct`
/// For enums: `{"VariantName": value}` → `DynamicEnum` with the named variant.
///   - `{"Srgba": {"red": 1.0, ...}}` → struct variant
///   - `{"TupleVariant": [1, 2]}` → tuple variant
///   - `{"UnitVariant": null}` or `{"UnitVariant": {}}` → unit variant
fn convert_object(
    obj: &Map<String, Value>,
    _target_type_id: TypeId,
    target_type_info: Option<&'static TypeInfo>,
    registry: &TypeRegistry,
) -> Result<Box<dyn PartialReflect>, String> {
    if let Some(TypeInfo::Struct(info)) = target_type_info {
        let mut dynamic = DynamicStruct::default();
        dynamic.set_represented_type(target_type_info);

        for (key, val) in obj {
            let field_info = info
                .field(key)
                .ok_or_else(|| format!("unknown field '{key}'"))?;
            let field_type_id = field_info.type_id();
            let field_type_info = registry.get(field_type_id).map(|r| r.type_info());
            let field_val = json_to_reflect(val, field_type_id, field_type_info, registry)?;
            dynamic.insert_boxed(key, field_val);
        }

        return Ok(Box::new(dynamic));
    }

    // Enum handling. Accept both the compact one-key form and the tagged form
    // returned by component introspection.
    if let Some(TypeInfo::Enum(enum_info)) = target_type_info {
        if let Some(Value::String(variant_name)) = obj.get("variant") {
            let variant_info = enum_info.variant(variant_name).ok_or_else(|| {
                let valid: Vec<&str> = (0..enum_info.variant_len())
                    .filter_map(|i| enum_info.variant_at(i).map(|variant| variant.name()))
                    .collect();
                format!(
                    "unknown variant '{variant_name}'. Valid variants: {}",
                    valid.join(", ")
                )
            })?;
            let payload = tagged_enum_payload(obj, variant_info)?;
            return convert_enum_variant(
                variant_name,
                &payload,
                enum_info,
                target_type_info,
                registry,
            );
        }
        if obj.len() != 1 {
            return Err(format!(
                "enum value must be an object with exactly one key (the variant name), got {} keys",
                obj.len()
            ));
        }
        let (variant_name, variant_value) = obj.iter().next().unwrap();
        return convert_enum_variant(
            variant_name,
            variant_value,
            enum_info,
            target_type_info,
            registry,
        );
    }

    Err("cannot convert object to non-struct type".into())
}

fn tagged_enum_payload(
    obj: &Map<String, Value>,
    variant_info: &VariantInfo,
) -> Result<Value, String> {
    let mut fields = obj.clone();
    fields.remove("variant");

    match variant_info {
        VariantInfo::Unit(_) if fields.is_empty() => Ok(Value::Null),
        VariantInfo::Unit(_) => Err("unit variant does not accept payload fields".to_string()),
        VariantInfo::Struct(_) => Ok(Value::Object(fields)),
        VariantInfo::Tuple(info) if info.field_len() == 1 => {
            if fields.len() != 1 {
                return Err(format!(
                    "single-value tuple variant expects one payload field, got {}",
                    fields.len()
                ));
            }
            Ok(fields.into_values().next().unwrap())
        }
        VariantInfo::Tuple(info) => {
            let values = (0..info.field_len())
                .map(|index| {
                    fields
                        .remove(&index.to_string())
                        .ok_or_else(|| format!("tuple variant is missing payload field '{index}'"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if fields.is_empty() {
                Ok(Value::Array(values))
            } else {
                Err("tuple variant has unknown payload fields".to_string())
            }
        }
    }
}

/// Convert a named variant + JSON value into a DynamicEnum.
fn convert_enum_variant(
    variant_name: &str,
    variant_value: &Value,
    enum_info: &EnumInfo,
    type_info: Option<&'static TypeInfo>,
    registry: &TypeRegistry,
) -> Result<Box<dyn PartialReflect>, String> {
    let variant_info = enum_info.variant(variant_name).ok_or_else(|| {
        let valid: Vec<&str> = (0..enum_info.variant_len())
            .filter_map(|i| enum_info.variant_at(i).map(|v| v.name()))
            .collect();
        format!(
            "unknown variant '{variant_name}'. Valid variants: {}",
            valid.join(", ")
        )
    })?;

    let dynamic_variant = match variant_info {
        VariantInfo::Unit(_) => DynamicVariant::Unit,
        VariantInfo::Struct(struct_info) => {
            let mut dynamic = DynamicStruct::default();
            match variant_value {
                Value::Object(fields) => {
                    for (key, val) in fields {
                        let field_info = struct_info.field(key).ok_or_else(|| {
                            format!("unknown field '{key}' on variant '{variant_name}'")
                        })?;
                        let field_type_id = field_info.type_id();
                        let field_type_info = registry.get(field_type_id).map(|r| r.type_info());
                        let field_val =
                            json_to_reflect(val, field_type_id, field_type_info, registry)?;
                        dynamic.insert_boxed(key, field_val);
                    }
                }
                Value::Null => {
                    // Null = use defaults (no fields set)
                }
                _ => {
                    return Err(format!(
                        "variant '{variant_name}' is a struct variant, expected object or null"
                    ));
                }
            }
            DynamicVariant::Struct(dynamic)
        }
        VariantInfo::Tuple(tuple_info) => {
            let mut dynamic = DynamicTuple::default();
            match variant_value {
                Value::Array(arr) => {
                    if arr.len() != tuple_info.field_len() {
                        return Err(format!(
                            "variant '{variant_name}' expects {} fields, got {}",
                            tuple_info.field_len(),
                            arr.len()
                        ));
                    }
                    for (i, val) in arr.iter().enumerate() {
                        let field_info = tuple_info.field_at(i).unwrap();
                        let field_type_id = field_info.type_id();
                        let field_type_info = registry.get(field_type_id).map(|r| r.type_info());
                        let field_val =
                            json_to_reflect(val, field_type_id, field_type_info, registry)?;
                        dynamic.insert_boxed(field_val);
                    }
                }
                // Single-field tuple variant: unwrap the value directly
                _ if tuple_info.field_len() == 1 => {
                    let field_info = tuple_info.field_at(0).unwrap();
                    let field_type_id = field_info.type_id();
                    let field_type_info = registry.get(field_type_id).map(|r| r.type_info());
                    let field_val =
                        json_to_reflect(variant_value, field_type_id, field_type_info, registry)?;
                    dynamic.insert_boxed(field_val);
                }
                _ => {
                    return Err(format!(
                        "variant '{variant_name}' is a tuple variant with {} fields, expected array",
                        tuple_info.field_len()
                    ));
                }
            }
            DynamicVariant::Tuple(dynamic)
        }
    };

    let mut dynamic = DynamicEnum::default();
    dynamic.set_represented_type(type_info);
    dynamic.set_variant(variant_name, dynamic_variant);
    Ok(Box::new(dynamic))
}

/// Build a DynamicStruct from positional field values using the struct's NamedField info.
fn build_dynamic_struct_from_fields(
    info: &StructInfo,
    type_info: Option<&'static TypeInfo>,
    mut value_fn: impl FnMut(usize) -> Result<Box<dyn PartialReflect>, String>,
) -> Result<Box<dyn PartialReflect>, String> {
    let mut dynamic = DynamicStruct::default();
    dynamic.set_represented_type(type_info);

    for i in 0..info.field_len() {
        let field_info = info
            .field_at(i)
            .ok_or_else(|| format!("field at index {i} not found"))?;
        let value = value_fn(i)?;
        dynamic.insert_boxed(field_info.name(), value);
    }

    Ok(Box::new(dynamic))
}

/// Convert a JSON number to the correct numeric Reflect type.
fn convert_number(
    n: &serde_json::Number,
    target_type_id: TypeId,
    target_type_info: Option<&'static TypeInfo>,
) -> Result<Box<dyn PartialReflect>, String> {
    if target_type_id == TypeId::of::<f32>() {
        Ok(Box::new(
            n.as_f64()
                .ok_or_else(|| format!("expected number for f32, got {n}"))? as f32,
        ))
    } else if target_type_id == TypeId::of::<f64>() {
        Ok(Box::new(n.as_f64().ok_or_else(|| {
            format!("expected number for f64, got {n}")
        })?))
    } else if target_type_id == TypeId::of::<i32>() {
        Ok(Box::new(
            n.as_i64()
                .ok_or_else(|| format!("expected integer for i32, got {n}"))? as i32,
        ))
    } else if target_type_id == TypeId::of::<u32>() {
        Ok(Box::new(
            n.as_u64()
                .ok_or_else(|| format!("expected unsigned integer for u32, got {n}"))?
                as u32,
        ))
    } else if target_type_id == TypeId::of::<i64>() {
        Ok(Box::new(n.as_i64().ok_or_else(|| {
            format!("expected integer for i64, got {n}")
        })?))
    } else if target_type_id == TypeId::of::<u64>() {
        Ok(Box::new(n.as_u64().ok_or_else(|| {
            format!("expected unsigned integer for u64, got {n}")
        })?))
    } else if target_type_id == TypeId::of::<usize>() {
        Ok(Box::new(
            n.as_u64()
                .ok_or_else(|| format!("expected unsigned integer for usize, got {n}"))?
                as usize,
        ))
    } else if target_type_id == TypeId::of::<i8>() {
        Ok(Box::new(
            n.as_i64()
                .ok_or_else(|| format!("expected integer for i8, got {n}"))? as i8,
        ))
    } else if target_type_id == TypeId::of::<u8>() {
        Ok(Box::new(
            n.as_u64()
                .ok_or_else(|| format!("expected unsigned integer for u8, got {n}"))?
                as u8,
        ))
    } else if target_type_id == TypeId::of::<i16>() {
        Ok(Box::new(
            n.as_i64()
                .ok_or_else(|| format!("expected integer for i16, got {n}"))? as i16,
        ))
    } else if target_type_id == TypeId::of::<u16>() {
        Ok(Box::new(
            n.as_u64()
                .ok_or_else(|| format!("expected unsigned integer for u16, got {n}"))?
                as u16,
        ))
    } else if target_type_id == TypeId::of::<isize>() {
        Ok(Box::new(
            n.as_i64()
                .ok_or_else(|| format!("expected integer for isize, got {n}"))?
                as isize,
        ))
    } else {
        let target = target_type_info
            .map(|info| info.type_path_table().short_path())
            .unwrap_or("the requested field type");
        Err(format!("cannot convert number {n} to {target}"))
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::App,
        color::{Color, Srgba},
        ecs::reflect::AppTypeRegistry,
        math::{UVec2, Vec3},
        prelude::*,
        reflect::Typed,
        ui::GridTrack,
    };

    use super::*;

    #[test]
    fn non_numeric_target_rejects_number_without_fallback() {
        let error = convert_number(
            &serde_json::Number::from(10),
            TypeId::of::<GridTrack>(),
            Some(GridTrack::type_info()),
        )
        .unwrap_err();

        assert_eq!(error, "cannot convert number 10 to GridTrack");
    }

    #[test]
    fn non_bool_target_rejects_bool_without_fallback() {
        let error = json_to_reflect(
            &serde_json::json!(true),
            TypeId::of::<GridTrack>(),
            Some(GridTrack::type_info()),
            &TypeRegistry::default(),
        )
        .unwrap_err();

        assert_eq!(error, "cannot convert bool true to GridTrack");
    }

    #[test]
    fn non_string_target_rejects_string_without_fallback() {
        let error = json_to_reflect(
            &serde_json::json!("Auto"),
            TypeId::of::<GridTrack>(),
            Some(GridTrack::type_info()),
            &TypeRegistry::default(),
        )
        .unwrap_err();

        assert_eq!(error, "cannot convert string \"Auto\" to GridTrack");
    }

    /// Helper to set up a minimal world with type registry and Transform registered.
    fn setup_world_with_transform() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        // MinimalPlugins doesn't register Transform — we need to do it explicitly
        // so the AppTypeRegistry knows about Transform, ReflectComponent, ReflectDefault
        app.register_type::<Transform>();
        app.update();

        let entity = app
            .world_mut()
            .spawn(Transform::from_xyz(1.0, 2.0, 3.0))
            .id();
        (app, entity)
    }

    #[test]
    fn reflect_set_translation_struct_format() {
        let (mut app, entity) = setup_world_with_transform();
        let world = app.world_mut();

        let mut fields = Map::new();
        fields.insert(
            "translation".into(),
            serde_json::json!({"x": 10.0, "y": 20.0, "z": 30.0}),
        );

        let result = reflect_set_component(world, entity, TypeId::of::<Transform>(), &fields);

        assert!(
            result.is_ok(),
            "reflect_set_component failed: {:?}",
            result.as_ref().err().map(|e| match e {
                ReflectError::FieldError(s) => s.as_str(),
                _ => "other error",
            })
        );
        let updated = result.unwrap();
        // Vec3 reports as an array, the same shape get_component emits.
        assert_eq!(
            updated,
            Map::from_iter([(
                "translation".to_string(),
                serde_json::json!([10.0, 20.0, 30.0])
            )])
        );

        let t = world.get::<Transform>(entity).unwrap();
        assert_eq!(t.translation, Vec3::new(10.0, 20.0, 30.0));
    }

    #[test]
    fn reflect_set_translation_array_format() {
        let (mut app, entity) = setup_world_with_transform();
        let world = app.world_mut();

        let mut fields = Map::new();
        fields.insert("translation".into(), serde_json::json!([10.0, 20.0, 30.0]));

        let result = reflect_set_component(world, entity, TypeId::of::<Transform>(), &fields);

        assert!(
            result.is_ok(),
            "reflect_set_component failed: {:?}",
            result.as_ref().err().map(|e| match e {
                ReflectError::FieldError(s) => s.as_str(),
                _ => "other error",
            })
        );

        let t = world.get::<Transform>(entity).unwrap();
        assert_eq!(t.translation, Vec3::new(10.0, 20.0, 30.0));
    }

    #[test]
    fn reflected_nonfinite_floats_round_trip_as_distinct_strings() {
        let (mut app, entity) = setup_world_with_transform();
        let world = app.world_mut();
        let fields = Map::from_iter([(
            "translation".to_string(),
            serde_json::json!(["NaN", "Infinity", "-Infinity"]),
        )]);

        let updated =
            reflect_set_component(world, entity, TypeId::of::<Transform>(), &fields).unwrap();

        assert_eq!(
            updated["translation"],
            serde_json::json!(["NaN", "Infinity", "-Infinity"])
        );
        let translation = world.get::<Transform>(entity).unwrap().translation;
        assert!(translation.x.is_nan());
        assert_eq!(translation.y, f32::INFINITY);
        assert_eq!(translation.z, f32::NEG_INFINITY);
    }

    #[test]
    fn reflect_set_multiple_fields() {
        let (mut app, entity) = setup_world_with_transform();
        let world = app.world_mut();

        let mut fields = Map::new();
        fields.insert("translation".into(), serde_json::json!([5.0, 10.0, 15.0]));
        fields.insert("scale".into(), serde_json::json!([2.0, 2.0, 2.0]));

        let result = reflect_set_component(world, entity, TypeId::of::<Transform>(), &fields);
        assert!(result.is_ok());

        let t = world.get::<Transform>(entity).unwrap();
        assert_eq!(t.translation, Vec3::new(5.0, 10.0, 15.0));
        assert_eq!(t.scale, Vec3::new(2.0, 2.0, 2.0));
    }

    #[test]
    fn reflect_spawn_with_fields() {
        let (mut app, _) = setup_world_with_transform();
        let world = app.world_mut();
        let entity = world.spawn_empty().id();

        let mut fields = Map::new();
        fields.insert("translation".into(), serde_json::json!([0.0, 5.0, 0.0]));

        let result = reflect_spawn_component(world, entity, TypeId::of::<Transform>(), &fields);
        assert!(result.is_ok(), "reflect_spawn_component failed");

        let t = world.get::<Transform>(entity).unwrap();
        assert_eq!(t.translation, Vec3::new(0.0, 5.0, 0.0));
    }

    #[test]
    fn reflect_spawn_default_no_fields() {
        let (mut app, _) = setup_world_with_transform();
        let world = app.world_mut();
        let entity = world.spawn_empty().id();

        let fields = Map::new();
        let result = reflect_spawn_component(world, entity, TypeId::of::<Transform>(), &fields);
        assert!(result.is_ok());

        let t = world.get::<Transform>(entity).unwrap();
        assert_eq!(t.translation, Vec3::ZERO);
    }

    /// A private test-only type that is never registered with the type registry.
    struct UnregisteredTestType;

    #[test]
    fn reflect_fallback_unregistered_type() {
        let (mut app, entity) = setup_world_with_transform();
        let world = app.world_mut();

        let bogus_type_id = TypeId::of::<UnregisteredTestType>();
        let fields = Map::new();

        let result = reflect_set_component(world, entity, bogus_type_id, &fields);
        assert!(matches!(result, Err(ReflectError::NotRegistered)));
    }

    #[test]
    fn reflect_set_nonexistent_field() {
        let (mut app, entity) = setup_world_with_transform();
        let world = app.world_mut();

        let mut fields = Map::new();
        fields.insert("nonexistent".into(), serde_json::json!(42.0));

        let result = reflect_set_component(world, entity, TypeId::of::<Transform>(), &fields);
        assert!(matches!(result, Err(ReflectError::FieldError(_))));
    }

    #[test]
    fn reflect_component_not_on_entity() {
        let (mut app, _) = setup_world_with_transform();
        let world = app.world_mut();
        // Spawn empty entity without Transform
        let entity = world.spawn_empty().id();

        let mut fields = Map::new();
        fields.insert("translation".into(), serde_json::json!([1.0, 2.0, 3.0]));

        let result = reflect_set_component(world, entity, TypeId::of::<Transform>(), &fields);
        assert!(matches!(result, Err(ReflectError::ComponentNotOnEntity)));
    }

    #[test]
    fn reflect_set_integer_to_float_field() {
        let (mut app, entity) = setup_world_with_transform();
        let world = app.world_mut();

        // JSON integers should convert to f32 for Vec3 fields
        let mut fields = Map::new();
        fields.insert("translation".into(), serde_json::json!([1, 2, 3]));

        let result = reflect_set_component(world, entity, TypeId::of::<Transform>(), &fields);
        assert!(result.is_ok());

        let t = world.get::<Transform>(entity).unwrap();
        assert_eq!(t.translation, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn convert_number_isize() {
        let n = serde_json::Number::from(42i64);
        let result = super::convert_number(&n, TypeId::of::<isize>(), None);
        assert!(result.is_ok());
        let boxed = result.unwrap();
        let val = boxed.try_downcast_ref::<isize>().unwrap();
        assert_eq!(*val, 42isize);
    }

    #[test]
    fn convert_number_isize_negative() {
        let n = serde_json::Number::from(-5i64);
        let result = super::convert_number(&n, TypeId::of::<isize>(), None);
        assert!(result.is_ok());
        let boxed = result.unwrap();
        let val = boxed.try_downcast_ref::<isize>().unwrap();
        assert_eq!(*val, -5isize);
    }

    #[test]
    fn convert_number_usize() {
        let n = serde_json::Number::from(100u64);
        let result = super::convert_number(&n, TypeId::of::<usize>(), None);
        assert!(result.is_ok());
        let boxed = result.unwrap();
        let val = boxed.try_downcast_ref::<usize>().unwrap();
        assert_eq!(*val, 100usize);
    }

    /// A non-struct type that IS registered in the type registry.
    #[derive(Component, Reflect, Default)]
    #[reflect(Component, Default)]
    enum TestEnumComponent {
        #[default]
        VariantA,
        VariantB,
    }

    #[derive(Reflect, Default)]
    #[reflect(Default)]
    struct TestEnumPayload {
        value: u32,
    }

    #[derive(Component, Reflect)]
    #[reflect(Component)]
    enum TestPayloadEnumComponent {
        First(TestEnumPayload),
        Second(TestEnumPayload),
    }

    #[derive(Reflect)]
    struct TestIncompleteEnumPayload {
        retained: u32,
        required: u32,
    }

    #[derive(Component, Reflect)]
    #[reflect(Component)]
    enum TestIncompletePayloadEnumComponent {
        First(TestIncompleteEnumPayload),
        Second(TestIncompleteEnumPayload),
    }

    #[test]
    fn reflect_set_non_struct_returns_not_a_struct() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<TestEnumComponent>();
        app.update();

        let world = app.world_mut();
        let entity = world.spawn(TestEnumComponent::VariantA).id();

        let fields = Map::new();
        let result =
            reflect_set_component(world, entity, TypeId::of::<TestEnumComponent>(), &fields);
        assert!(matches!(result, Err(ReflectError::FieldError(_))));
    }

    #[test]
    fn reflect_set_enum_component_replaces_variant() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<TestEnumComponent>();
        app.update();

        let world = app.world_mut();
        let entity = world.spawn(TestEnumComponent::VariantA).id();
        let mut fields = Map::new();
        fields.insert("variant".into(), serde_json::json!("VariantB"));

        let result =
            reflect_set_component(world, entity, TypeId::of::<TestEnumComponent>(), &fields);

        assert_eq!(
            result.unwrap(),
            Map::from_iter([("variant".to_string(), serde_json::json!("VariantB"))])
        );
        assert!(matches!(
            world.get::<TestEnumComponent>(entity),
            Some(TestEnumComponent::VariantB)
        ));
    }

    #[test]
    fn reflect_set_enum_component_replaces_a_payload_variant() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<TestEnumPayload>();
        app.register_type::<TestPayloadEnumComponent>();
        app.update();

        let world = app.world_mut();
        let entity = world
            .spawn(TestPayloadEnumComponent::First(TestEnumPayload {
                value: 1,
            }))
            .id();
        let fields = Map::from_iter([
            ("variant".to_string(), serde_json::json!("Second")),
            ("value".to_string(), serde_json::json!({"value": 42})),
        ]);

        let result = reflect_set_component(
            world,
            entity,
            TypeId::of::<TestPayloadEnumComponent>(),
            &fields,
        );

        assert_eq!(
            result.unwrap(),
            Map::from_iter([("variant".to_string(), serde_json::json!("Second"))])
        );
        let Some(TestPayloadEnumComponent::Second(payload)) =
            world.get::<TestPayloadEnumComponent>(entity)
        else {
            panic!("payload enum variant was not replaced");
        };
        assert_eq!(payload.value, 42);
    }

    #[test]
    fn reflect_set_enum_component_defaults_a_payload_when_available() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<TestEnumPayload>();
        app.register_type::<TestPayloadEnumComponent>();
        app.update();

        let world = app.world_mut();
        let entity = world
            .spawn(TestPayloadEnumComponent::First(TestEnumPayload {
                value: 7,
            }))
            .id();
        let fields = Map::from_iter([("variant".to_string(), serde_json::json!("Second"))]);

        reflect_set_component(
            world,
            entity,
            TypeId::of::<TestPayloadEnumComponent>(),
            &fields,
        )
        .unwrap();

        let Some(TestPayloadEnumComponent::Second(payload)) =
            world.get::<TestPayloadEnumComponent>(entity)
        else {
            panic!("payload enum variant was not replaced");
        };
        assert_eq!(payload.value, 0);
    }

    #[test]
    fn reflect_set_enum_component_rejects_incomplete_variant_atomically() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<TestIncompleteEnumPayload>();
        app.register_type::<TestIncompletePayloadEnumComponent>();
        app.update();

        let world = app.world_mut();
        let entity = world
            .spawn(TestIncompletePayloadEnumComponent::First(
                TestIncompleteEnumPayload {
                    retained: 7,
                    required: 11,
                },
            ))
            .id();
        let fields = Map::from_iter([
            ("variant".to_string(), serde_json::json!("Second")),
            ("value".to_string(), serde_json::json!({"retained": 42})),
        ]);

        let result = reflect_set_component(
            world,
            entity,
            TypeId::of::<TestIncompletePayloadEnumComponent>(),
            &fields,
        );

        assert!(matches!(result, Err(ReflectError::FieldError(_))));
        let Some(TestIncompletePayloadEnumComponent::First(payload)) =
            world.get::<TestIncompletePayloadEnumComponent>(entity)
        else {
            panic!("failed enum update changed the live variant");
        };
        assert_eq!(payload.retained, 7);
        assert_eq!(payload.required, 11);
    }

    #[test]
    fn reflect_spawn_non_struct_no_fields_succeeds() {
        // Enum with Default + ReflectComponent can spawn with empty fields
        // (no struct check needed when there are no fields to apply)
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<TestEnumComponent>();
        app.update();

        let world = app.world_mut();
        let entity = world.spawn_empty().id();

        let fields = Map::new();
        let result =
            reflect_spawn_component(world, entity, TypeId::of::<TestEnumComponent>(), &fields);
        assert!(result.is_ok());
    }

    #[test]
    fn reflect_spawn_enum_with_variant_field_succeeds() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<TestEnumComponent>();
        app.update();

        let world = app.world_mut();
        let entity = world.spawn_empty().id();

        let mut fields = Map::new();
        fields.insert("variant".into(), serde_json::json!("VariantB"));

        let result =
            reflect_spawn_component(world, entity, TypeId::of::<TestEnumComponent>(), &fields);
        assert!(result.is_ok());
        assert!(matches!(
            world.get::<TestEnumComponent>(entity),
            Some(TestEnumComponent::VariantB)
        ));
    }

    /// A component with an Option<Vec3> field for testing Option unwrapping.
    #[derive(Component, Reflect, Default)]
    #[reflect(Component, Default)]
    struct OptionalFieldComponent {
        value: Option<Vec3>,
    }

    #[test]
    fn reflect_set_option_field_with_value() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<OptionalFieldComponent>();
        app.register_type::<Option<Vec3>>();
        app.update();

        let world = app.world_mut();
        let entity = world.spawn(OptionalFieldComponent { value: None }).id();

        let mut fields = Map::new();
        fields.insert("value".into(), serde_json::json!([1.0, 2.0, 3.0]));

        let result = reflect_set_component(
            world,
            entity,
            TypeId::of::<OptionalFieldComponent>(),
            &fields,
        );

        assert!(result.is_ok(), "Setting Option field failed: {:?}", result);

        let comp = world.get::<OptionalFieldComponent>(entity).unwrap();
        assert_eq!(comp.value, Some(Vec3::new(1.0, 2.0, 3.0)));
    }

    #[test]
    fn reflect_set_option_field_to_null() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<OptionalFieldComponent>();
        app.register_type::<Option<Vec3>>();
        app.update();

        let world = app.world_mut();
        let entity = world
            .spawn(OptionalFieldComponent {
                value: Some(Vec3::ONE),
            })
            .id();

        let mut fields = Map::new();
        fields.insert("value".into(), serde_json::json!(null));

        let result = reflect_set_component(
            world,
            entity,
            TypeId::of::<OptionalFieldComponent>(),
            &fields,
        );

        assert!(
            result.is_ok(),
            "Setting Option to null failed: {:?}",
            result
        );

        let comp = world.get::<OptionalFieldComponent>(entity).unwrap();
        assert_eq!(comp.value, None);
    }

    #[derive(Component, Reflect, Default)]
    #[reflect(Component, Default)]
    struct VecFieldComponent {
        weights: Vec<f32>,
    }

    #[test]
    fn convert_number_error_includes_type_info() {
        let n = serde_json::Number::from_f64(std::f64::consts::PI).unwrap();
        let result = super::convert_number(&n, TypeId::of::<i32>(), None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("i32"),
            "Error should mention target type: {err}"
        );
        assert!(err.contains("3.14"), "Error should mention value: {err}");
    }

    #[derive(Component, Reflect, Default)]
    #[reflect(Component, Default)]
    struct TupleStructFieldComponent {
        pos: UVec2,
    }

    #[test]
    fn reflect_set_tuple_struct_field() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<TupleStructFieldComponent>();
        app.register_type::<UVec2>();
        app.update();

        let world = app.world_mut();
        let entity = world
            .spawn(TupleStructFieldComponent { pos: UVec2::ZERO })
            .id();

        let mut fields = Map::new();
        fields.insert("pos".into(), serde_json::json!([700, 300]));

        let result = reflect_set_component(
            world,
            entity,
            TypeId::of::<TupleStructFieldComponent>(),
            &fields,
        );
        assert!(
            result.is_ok(),
            "Setting TupleStruct field failed: {:?}",
            result
        );

        let comp = world.get::<TupleStructFieldComponent>(entity).unwrap();
        assert_eq!(comp.pos, UVec2::new(700, 300));
    }

    #[test]
    fn reflect_set_vec_field() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<VecFieldComponent>();
        app.register_type::<Vec<f32>>();
        app.update();

        let world = app.world_mut();
        let entity = world
            .spawn(VecFieldComponent {
                weights: vec![0.0; 4],
            })
            .id();

        let mut fields = Map::new();
        fields.insert("weights".into(), serde_json::json!([1.0, 0.5, 0.0, 0.25]));

        let result =
            reflect_set_component(world, entity, TypeId::of::<VecFieldComponent>(), &fields);
        assert!(result.is_ok(), "Setting Vec field failed: {:?}", result);

        let comp = world.get::<VecFieldComponent>(entity).unwrap();
        assert_eq!(comp.weights, vec![1.0, 0.5, 0.0, 0.25]);
    }

    /// A component with a Color field for testing enum mutation.
    #[derive(Component, Reflect, Default)]
    #[reflect(Component, Default)]
    struct ColorComponent {
        color: Color,
    }

    #[test]
    fn reflect_set_enum_field_struct_variant() {
        // Test: {"Srgba": {"red": 1.0, "green": 0.5, "blue": 0.0, "alpha": 1.0}} → Color::Srgba
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<ColorComponent>();
        app.register_type::<Color>();
        app.register_type::<Srgba>();
        app.update();

        let world = app.world_mut();
        let entity = world.spawn(ColorComponent::default()).id();

        let mut fields = Map::new();
        fields.insert(
            "color".into(),
            serde_json::json!({"Srgba": {"red": 1.0, "green": 0.5, "blue": 0.0, "alpha": 1.0}}),
        );

        let result = reflect_set_component(world, entity, TypeId::of::<ColorComponent>(), &fields);

        assert!(
            result.is_ok(),
            "Setting Color enum field failed: {:?}",
            result
        );

        let comp = world.get::<ColorComponent>(entity).unwrap();
        match comp.color {
            Color::Srgba(c) => {
                assert!((c.red - 1.0).abs() < 0.001);
                assert!((c.green - 0.5).abs() < 0.001);
                assert!((c.blue - 0.0).abs() < 0.001);
                assert!((c.alpha - 1.0).abs() < 0.001);
            }
            other => panic!("Expected Color::Srgba, got {:?}", other),
        }
    }

    /// A component with a unit-enum field for testing string-to-unit-variant.
    #[derive(Component, Reflect, Default, Debug, PartialEq)]
    #[reflect(Component, Default)]
    struct ModeComponent {
        mode: TestMode,
    }

    #[derive(Reflect, Default, Debug, PartialEq, Clone)]
    enum TestMode {
        #[default]
        Auto,
        Manual,
        Disabled,
    }

    #[test]
    fn reflect_set_enum_field_string_unit_variant() {
        // Test: "Manual" → TestMode::Manual
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<ModeComponent>();
        app.register_type::<TestMode>();
        app.update();

        let world = app.world_mut();
        let entity = world.spawn(ModeComponent::default()).id();
        assert_eq!(
            world.get::<ModeComponent>(entity).unwrap().mode,
            TestMode::Auto
        );

        let mut fields = Map::new();
        fields.insert("mode".into(), serde_json::json!("Manual"));

        let result = reflect_set_component(world, entity, TypeId::of::<ModeComponent>(), &fields);

        assert!(
            result.is_ok(),
            "Setting enum unit variant via string failed: {:?}",
            result
        );

        let comp = world.get::<ModeComponent>(entity).unwrap();
        assert_eq!(comp.mode, TestMode::Manual);
    }

    #[test]
    fn reflect_set_enum_field_object_unit_variant() {
        // Test: {"Disabled": null} → TestMode::Disabled
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<ModeComponent>();
        app.register_type::<TestMode>();
        app.update();

        let world = app.world_mut();
        let entity = world.spawn(ModeComponent::default()).id();

        let mut fields = Map::new();
        fields.insert("mode".into(), serde_json::json!({"Disabled": null}));

        let result = reflect_set_component(world, entity, TypeId::of::<ModeComponent>(), &fields);

        assert!(
            result.is_ok(),
            "Setting enum unit variant via object failed: {:?}",
            result
        );

        let comp = world.get::<ModeComponent>(entity).unwrap();
        assert_eq!(comp.mode, TestMode::Disabled);
    }

    #[test]
    fn reflect_set_enum_field_invalid_variant() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<ModeComponent>();
        app.register_type::<TestMode>();
        app.update();

        let world = app.world_mut();
        let entity = world.spawn(ModeComponent::default()).id();

        let mut fields = Map::new();
        fields.insert("mode".into(), serde_json::json!("NonExistent"));

        let result = reflect_set_component(world, entity, TypeId::of::<ModeComponent>(), &fields);

        // String "NonExistent" doesn't match any variant, falls through to String type
        // which will fail at try_apply because TestMode != String
        assert!(matches!(result, Err(ReflectError::FieldError(_))));
    }

    #[derive(Component, Reflect, Default)]
    #[reflect(Component, Default)]
    struct GlobalFieldComponent {
        global: f32,
        other: f32,
    }

    #[test]
    fn resolve_field_name_exact_match() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<GlobalFieldComponent>();
        app.update();

        let registry_arc = app
            .world_mut()
            .get_resource::<AppTypeRegistry>()
            .unwrap()
            .clone();
        let registry = registry_arc.read();
        let reg = registry.get(TypeId::of::<GlobalFieldComponent>()).unwrap();
        let struct_info = match reg.type_info() {
            TypeInfo::Struct(info) => info,
            _ => panic!("Expected struct type info"),
        };

        // Exact match
        assert_eq!(
            super::resolve_field_name("global", struct_info),
            Some("global".to_string())
        );
        assert_eq!(
            super::resolve_field_name("other", struct_info),
            Some("other".to_string())
        );
    }

    #[test]
    fn resolve_field_name_trailing_underscore_alias() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<GlobalFieldComponent>();
        app.update();

        let registry_arc = app
            .world_mut()
            .get_resource::<AppTypeRegistry>()
            .unwrap()
            .clone();
        let registry = registry_arc.read();
        let reg = registry.get(TypeId::of::<GlobalFieldComponent>()).unwrap();
        let struct_info = match reg.type_info() {
            TypeInfo::Struct(info) => info,
            _ => panic!("Expected struct type info"),
        };

        // Python reserved word alias: global_ → global
        assert_eq!(
            super::resolve_field_name("global_", struct_info),
            Some("global".to_string())
        );
        // Non-existent field
        assert_eq!(super::resolve_field_name("nonexistent", struct_info), None);
        // Trailing underscore on non-existent field
        assert_eq!(super::resolve_field_name("nonexistent_", struct_info), None);
    }
}
