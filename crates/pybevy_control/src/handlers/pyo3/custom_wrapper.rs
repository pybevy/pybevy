use std::alloc::Layout;

use bevy::{
    ecs::ptr::Ptr,
    math::{Vec2, Vec3},
};
use pybevy_core::component_layout::{ComponentLayout, PrimitiveType, PrimitiveValue};

use crate::handlers::json_float::{float_to_json, nonfinite_float_from_json};

pub(crate) fn descriptor_matches(layout: &ComponentLayout, descriptor: Layout) -> bool {
    layout.schema().wrapper_size.mem_layout() == descriptor
}

pub(crate) fn fields_to_json(
    ptr: Ptr<'_>,
    layout: &ComponentLayout,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    // SAFETY: callers verify that the Bevy descriptor has the exact wrapper
    // layout recorded at component registration before calling this function.
    let data = unsafe { layout.wrapper_size.get_ref_ptr_as_mut(ptr) } as *const u8;
    let schema = layout.schema();
    // SAFETY: the caller checked the descriptor and `data` points at that
    // wrapper's data allocation. The shared core validates every field range.
    let values = unsafe { schema.read_values(data) }.map_err(|error| error.to_string())?;
    Ok(values
        .into_iter()
        .map(|(name, value)| (name, primitive_to_json(value)))
        .collect())
}

pub(crate) fn values_from_json(
    layout: &ComponentLayout,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> (Vec<(String, usize, PrimitiveValue)>, Vec<String>) {
    let mut values = Vec::new();
    let mut errors = Vec::new();
    let schema = layout.schema();
    let buffer_size = schema.wrapper_size.size_bytes();

    for (name, json) in fields {
        let Some(field) = schema.get_field(name) else {
            errors.push(format!(
                "{name}: unknown field; available fields: {}",
                layout.field_names().join(", ")
            ));
            continue;
        };
        let size = field.field_type.size_bytes();
        let in_bounds = field
            .offset
            .checked_add(size)
            .is_some_and(|end| end <= buffer_size);
        if !in_bounds {
            errors.push(format!("{name}: field layout exceeds wrapper buffer"));
            continue;
        }
        match primitive_from_json(field.field_type, json) {
            Ok(value) => values.push((name.clone(), field.offset, value)),
            Err(error) => errors.push(format!("{name}: {error}")),
        }
    }

    (values, errors)
}

fn primitive_to_json(value: PrimitiveValue) -> serde_json::Value {
    match value {
        PrimitiveValue::F32(value) => float_to_json(f64::from(value)),
        PrimitiveValue::F64(value) => float_to_json(value),
        PrimitiveValue::I32(value) => serde_json::Value::from(value),
        PrimitiveValue::I64(value) => serde_json::Value::from(value),
        PrimitiveValue::U32(value) => serde_json::Value::from(value),
        PrimitiveValue::U64(value) => serde_json::Value::from(value),
        PrimitiveValue::Bool(value) => serde_json::Value::from(value),
        PrimitiveValue::Vec3(value) => serde_json::Value::Array(vec![
            float_to_json(f64::from(value.x)),
            float_to_json(f64::from(value.y)),
            float_to_json(f64::from(value.z)),
        ]),
        PrimitiveValue::Vec2(value) => serde_json::Value::Array(vec![
            float_to_json(f64::from(value.x)),
            float_to_json(f64::from(value.y)),
        ]),
    }
}

fn primitive_from_json(
    field_type: PrimitiveType,
    value: &serde_json::Value,
) -> Result<PrimitiveValue, String> {
    Ok(match field_type {
        PrimitiveType::F32 => {
            let value = json_f64(value)?;
            let narrowed = value as f32;
            if value.is_finite() && !narrowed.is_finite() {
                return Err("number is outside the f32 range".to_string());
            }
            PrimitiveValue::F32(narrowed)
        }
        PrimitiveType::F64 => PrimitiveValue::F64(json_f64(value)?),
        PrimitiveType::I32 => PrimitiveValue::I32(
            i32::try_from(json_i64(value)?)
                .map_err(|_| "integer is outside the i32 range".to_string())?,
        ),
        PrimitiveType::I64 => PrimitiveValue::I64(json_i64(value)?),
        PrimitiveType::U32 => PrimitiveValue::U32(
            u32::try_from(json_u64(value)?)
                .map_err(|_| "integer is outside the u32 range".to_string())?,
        ),
        PrimitiveType::U64 => PrimitiveValue::U64(json_u64(value)?),
        PrimitiveType::Bool => PrimitiveValue::Bool(
            value
                .as_bool()
                .ok_or_else(|| "expected a boolean".to_string())?,
        ),
        PrimitiveType::Vec3 => {
            let values = json_vector(value, 3)?;
            PrimitiveValue::Vec3(Vec3::new(values[0], values[1], values[2]))
        }
        PrimitiveType::Vec2 => {
            let values = json_vector(value, 2)?;
            PrimitiveValue::Vec2(Vec2::new(values[0], values[1]))
        }
    })
}

fn json_f64(value: &serde_json::Value) -> Result<f64, String> {
    value
        .as_f64()
        .or_else(|| nonfinite_float_from_json(value))
        .ok_or_else(|| "expected a number or non-finite float spelling".to_string())
}

fn json_i64(value: &serde_json::Value) -> Result<i64, String> {
    value
        .as_i64()
        .ok_or_else(|| "expected an integer in the i64 range".to_string())
}

fn json_u64(value: &serde_json::Value) -> Result<u64, String> {
    value
        .as_u64()
        .ok_or_else(|| "expected a non-negative integer in the u64 range".to_string())
}

fn json_vector(value: &serde_json::Value, expected_len: usize) -> Result<Vec<f32>, String> {
    let values = value
        .as_array()
        .filter(|values| values.len() == expected_len)
        .ok_or_else(|| format!("expected an array of {expected_len} numbers"))?;
    values
        .iter()
        .map(|value| {
            let value = json_f64(value)?;
            let narrowed = value as f32;
            if !value.is_finite() || narrowed.is_finite() {
                Ok(narrowed)
            } else {
                Err("number is outside the f32 range".to_string())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use pybevy_core::component_layout::ComponentLayout;

    use super::*;

    fn layout() -> ComponentLayout {
        ComponentLayout::from_fields(
            ptr::null(),
            "Motion".to_string(),
            &[
                ("speed".to_string(), PrimitiveType::F64),
                ("count".to_string(), PrimitiveType::I64),
                ("direction".to_string(), PrimitiveType::Vec2),
            ],
        )
        .unwrap()
    }

    #[test]
    fn json_values_follow_registered_field_types() {
        let fields = serde_json::json!({
            "speed": 2.5,
            "count": 4,
            "direction": [1.0, -1.0],
        });
        let (values, errors) = values_from_json(&layout(), fields.as_object().unwrap());
        assert!(errors.is_empty());
        assert_eq!(values.len(), 3);
    }

    #[test]
    fn json_values_report_unknown_and_invalid_fields() {
        let fields = serde_json::json!({"missing": 1, "count": 1.5});
        let (values, errors) = values_from_json(&layout(), fields.as_object().unwrap());
        assert!(values.is_empty());
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn wrapper_float_values_accept_and_preserve_nonfinite_spellings() {
        for (json, expected) in [
            (serde_json::json!("NaN"), f64::NAN),
            (serde_json::json!("Infinity"), f64::INFINITY),
            (serde_json::json!("-Infinity"), f64::NEG_INFINITY),
        ] {
            let value = primitive_from_json(PrimitiveType::F64, &json).unwrap();
            let PrimitiveValue::F64(value) = value else {
                panic!("expected an f64 primitive");
            };
            assert_eq!(value.is_nan(), expected.is_nan());
            assert_eq!(primitive_to_json(PrimitiveValue::F64(value)), json);
        }
    }
}
