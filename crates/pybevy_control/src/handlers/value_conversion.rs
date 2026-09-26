use bevy::math::Vec3;
use pybevy_core::public_error;

use crate::handlers::json_float::{nonfinite_float_from_json, require_finite_float};

pub(crate) enum EnumPayload {
    Unit,
    Single(serde_json::Value),
    Named(serde_json::Map<String, serde_json::Value>),
    Tuple(Vec<serde_json::Value>),
}

/// Build the one enum representation shared by reflected and Python values.
///
/// Named payloads keep their native field names beside `variant`; a single
/// tuple payload uses `value`, and larger tuples use their reflected indexes.
pub(crate) fn enum_to_json(variant: impl Into<String>, payload: EnumPayload) -> serde_json::Value {
    let mut fields = serde_json::Map::new();
    fields.insert(
        "variant".to_string(),
        serde_json::Value::String(variant.into()),
    );
    match payload {
        EnumPayload::Unit => {}
        EnumPayload::Single(value) => {
            fields.insert("value".to_string(), value);
        }
        EnumPayload::Named(values) => fields.extend(values),
        EnumPayload::Tuple(values) => {
            fields.extend(
                values
                    .into_iter()
                    .enumerate()
                    .map(|(index, value)| (index.to_string(), value)),
            );
        }
    }
    serde_json::Value::Object(fields)
}

pub(crate) fn vec3_from_json(value: &serde_json::Value) -> Result<Vec3, String> {
    const COORDINATES: [&str; 3] = ["x", "y", "z"];

    let values = match value {
        serde_json::Value::Array(values) => {
            if values.len() != COORDINATES.len() {
                return Err(public_error::mcp_vec3_input(format!(
                    "Vec3 requires exactly 3 coordinates, got {} elements",
                    values.len()
                )));
            }
            COORDINATES
                .iter()
                .zip(values)
                .map(|(name, value)| vec3_coordinate_from_json(name, value))
                .collect::<Result<Vec<_>, _>>()?
        }
        serde_json::Value::Object(fields) => {
            if let Some(name) = fields
                .keys()
                .find(|name| !COORDINATES.contains(&name.as_str()))
            {
                return Err(public_error::mcp_vec3_input(format!(
                    "Vec3 object has unknown coordinate '{name}'"
                )));
            }
            if let Some(name) = COORDINATES.iter().find(|name| !fields.contains_key(**name)) {
                return Err(public_error::mcp_vec3_input(format!(
                    "Vec3 object is missing coordinate '{name}'"
                )));
            }
            COORDINATES
                .iter()
                .map(|name| vec3_coordinate_from_json(name, &fields[*name]))
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => {
            return Err(public_error::mcp_vec3_input(format!(
                "Vec3 got {}",
                json_kind_name(value)
            )));
        }
    };

    Ok(Vec3::new(values[0], values[1], values[2]))
}

fn vec3_coordinate_from_json(name: &str, value: &serde_json::Value) -> Result<f32, String> {
    let value = json_f64(value)
        .map_err(|error| public_error::mcp_vec3_input(format!("Vec3.{name}: {error}")))?;
    let narrowed = value as f32;
    if value.is_finite() && !narrowed.is_finite() {
        return Err(public_error::mcp_vec3_input(format!(
            "Vec3.{name}: number is outside the f32 range"
        )));
    }
    Ok(narrowed)
}

fn json_f64(value: &serde_json::Value) -> Result<f64, String> {
    value
        .as_f64()
        .or_else(|| nonfinite_float_from_json(value))
        .ok_or_else(|| {
            if value.is_null() {
                public_error::COMPONENT_FLOAT_REQUIRED
            } else {
                public_error::COMPONENT_FLOAT_EXPECTED_NUMBER
            }
            .to_string()
        })
        .and_then(require_finite_float)
}

fn json_kind_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::{EnumPayload, enum_to_json};

    #[test]
    fn enum_json_uses_one_shape_for_every_payload_kind() {
        assert_eq!(
            enum_to_json("Hidden", EnumPayload::Unit),
            serde_json::json!({"variant": "Hidden"})
        );
        assert_eq!(
            enum_to_json("Px", EnumPayload::Single(serde_json::json!(12.0))),
            serde_json::json!({"variant": "Px", "value": 12.0})
        );
        assert_eq!(
            enum_to_json(
                "Linear",
                EnumPayload::Named(serde_json::Map::from_iter([
                    ("start".to_string(), serde_json::json!(1.0)),
                    ("end".to_string(), serde_json::json!(4.0)),
                ])),
            ),
            serde_json::json!({"variant": "Linear", "start": 1.0, "end": 4.0})
        );
        assert_eq!(
            enum_to_json(
                "Pair",
                EnumPayload::Tuple(vec![serde_json::json!(1), serde_json::json!(2)]),
            ),
            serde_json::json!({"variant": "Pair", "0": 1, "1": 2})
        );
    }
}
