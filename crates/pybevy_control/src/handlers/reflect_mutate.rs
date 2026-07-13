use std::any::TypeId;

use bevy::{
    ecs::{
        entity::Entity,
        reflect::{AppTypeRegistry, ReflectComponent},
        world::World,
    },
    prelude::ReflectDefault,
    reflect::{
        PartialReflect, ReflectMut, TypeInfo, TypeRegistry,
        enums::{DynamicEnum, DynamicVariant, EnumInfo, VariantInfo},
        list::DynamicList,
        structs::{DynamicStruct, StructInfo},
        tuple::DynamicTuple,
        tuple_struct::DynamicTupleStruct,
    },
};
use serde_json::{Map, Value};

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
/// Returns list of updated field names on success.
pub fn reflect_set_component(
    world: &mut World,
    entity: Entity,
    type_id: TypeId,
    fields: &Map<String, Value>,
) -> Result<Vec<String>, ReflectError> {
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

    // Drop registry lock before mutable world access
    drop(type_registry);

    // Get mutable component via reflection
    let mut entity_mut = world
        .get_entity_mut(entity)
        .map_err(|_| ReflectError::ComponentNotOnEntity)?;

    let Some(mut reflected) = reflect_component.reflect_mut(&mut entity_mut) else {
        return Err(ReflectError::ComponentNotOnEntity);
    };

    // Apply each converted field
    let mut updated = Vec::new();
    match reflected.reflect_mut() {
        ReflectMut::Struct(s) => {
            for (name, value) in converted {
                if let Some(field) = s.field_mut(&name) {
                    field
                        .try_apply(value.as_ref())
                        .map_err(|e| ReflectError::FieldError(format!("{name}: {e}")))?;
                    updated.push(name);
                } else {
                    return Err(ReflectError::FieldError(format!(
                        "{name}: field not found on component"
                    )));
                }
            }
        }
        _ => {
            return Err(ReflectError::NotAStruct);
        }
    }

    Ok(updated)
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
        let struct_info = match type_info {
            TypeInfo::Struct(info) => info,
            _ => {
                return Err(ReflectError::NotAStruct);
            }
        };

        match default_component.reflect_mut() {
            ReflectMut::Struct(s) => {
                for (field_name, field_value) in fields {
                    let actual_name =
                        resolve_field_name(field_name, struct_info).ok_or_else(|| {
                            ReflectError::FieldError(format!("{field_name}: field not found"))
                        })?;
                    let reflected =
                        json_field_to_reflect(field_name, field_value, struct_info, &type_registry)
                            .map_err(|e| ReflectError::FieldError(format!("{field_name}: {e}")))?;

                    if let Some(field) = s.field_mut(&actual_name) {
                        field
                            .try_apply(reflected.as_ref())
                            .map_err(|e| ReflectError::FieldError(format!("{field_name}: {e}")))?;
                    } else {
                        return Err(ReflectError::FieldError(format!(
                            "{field_name}: field not found on component"
                        )));
                    }
                }
            }
            _ => {
                return Err(ReflectError::NotAStruct);
            }
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
        Value::Number(n) => convert_number(n, target_type_id),
        Value::Bool(b) => Ok(Box::new(*b)),
        Value::String(s) => {
            // If targeting an enum, treat string as a unit variant name
            if let Some(TypeInfo::Enum(enum_info)) = target_type_info
                && enum_info.variant(s).is_some()
            {
                let mut dynamic = DynamicEnum::default();
                dynamic.set_represented_type(target_type_info);
                dynamic.set_variant(s, DynamicVariant::Unit);
                return Ok(Box::new(dynamic));
            }
            Ok(Box::new(s.clone()))
        }
        Value::Array(arr) => convert_array(arr, target_type_id, target_type_info, registry),
        Value::Object(obj) => convert_object(obj, target_type_id, target_type_info, registry),
        Value::Null => Err("null values not supported".into()),
    }
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

    // Enum handling: object with exactly one key = variant name
    if let Some(TypeInfo::Enum(enum_info)) = target_type_info {
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
        // Fall back: try f32 (most common in Bevy components)
        Ok(Box::new(
            n.as_f64()
                .ok_or_else(|| format!("expected number (falling back to f32), got {n}"))?
                as f32,
        ))
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
    };

    use super::*;

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
        assert_eq!(updated, vec!["translation"]);

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
        let result = super::convert_number(&n, TypeId::of::<isize>());
        assert!(result.is_ok());
        let boxed = result.unwrap();
        let val = boxed.try_downcast_ref::<isize>().unwrap();
        assert_eq!(*val, 42isize);
    }

    #[test]
    fn convert_number_isize_negative() {
        let n = serde_json::Number::from(-5i64);
        let result = super::convert_number(&n, TypeId::of::<isize>());
        assert!(result.is_ok());
        let boxed = result.unwrap();
        let val = boxed.try_downcast_ref::<isize>().unwrap();
        assert_eq!(*val, -5isize);
    }

    #[test]
    fn convert_number_usize() {
        let n = serde_json::Number::from(100u64);
        let result = super::convert_number(&n, TypeId::of::<usize>());
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
        assert!(matches!(result, Err(ReflectError::NotAStruct)));
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
    fn reflect_spawn_non_struct_with_fields_returns_not_a_struct() {
        // Enum with fields should return NotAStruct (can't apply struct fields)
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
        assert!(matches!(result, Err(ReflectError::NotAStruct)));
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
        let result = super::convert_number(&n, TypeId::of::<i32>());
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
