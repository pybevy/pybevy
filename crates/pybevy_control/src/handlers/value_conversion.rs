use bevy::math::Vec3;
use pybevy_core::public_error;

use crate::handlers::json_float::{nonfinite_float_from_json, require_finite_float};

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
