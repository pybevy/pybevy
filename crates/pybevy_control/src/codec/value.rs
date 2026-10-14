use bevy::math::{Quat, Vec3};
use pybevy_core::public_error;

use crate::codec::json_float::{nonfinite_float_from_json, require_finite_float};

/// Field names of a linear RGBA color, in array order.
pub(crate) const LINEAR_RGBA_CHANNELS: [&str; 4] = ["red", "green", "blue", "alpha"];

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

pub(crate) fn unit_quat_from_json(value: &serde_json::Value) -> Result<Quat, String> {
    const COORDINATES: [&str; 4] = ["x", "y", "z", "w"];
    let values: Vec<&serde_json::Value> = match value {
        serde_json::Value::Array(values) if values.len() == COORDINATES.len() => {
            values.iter().collect()
        }
        serde_json::Value::Array(values) => {
            return Err(format!(
                "Quat expects [x, y, z, w], got {} elements",
                values.len()
            ));
        }
        serde_json::Value::Object(fields) => {
            if let Some(name) = fields
                .keys()
                .find(|name| !COORDINATES.contains(&name.as_str()))
            {
                return Err(format!("Quat has unknown coordinate '{name}'"));
            }
            COORDINATES
                .iter()
                .map(|name| {
                    fields
                        .get(*name)
                        .ok_or_else(|| format!("Quat is missing coordinate '{name}'"))
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => return Err("Quat expects [x, y, z, w] or an x/y/z/w object".to_string()),
    };
    let values = COORDINATES
        .iter()
        .zip(values)
        .map(|(name, value)| {
            let value = json_f64(value).map_err(|error| format!("Quat.{name}: {error}"))?;
            let narrowed = value as f32;
            if !narrowed.is_finite() {
                return Err(format!("Quat.{name}: number is outside the f32 range"));
            }
            Ok(narrowed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let length = values
        .iter()
        .map(|value| f64::from(*value).powi(2))
        .sum::<f64>()
        .sqrt();
    if length == 0.0 || !length.is_finite() {
        return Err("Quat rotation must have finite, non-zero length".to_string());
    }
    Ok(Quat::from_xyzw(
        (f64::from(values[0]) / length) as f32,
        (f64::from(values[1]) / length) as f32,
        (f64::from(values[2]) / length) as f32,
        (f64::from(values[3]) / length) as f32,
    ))
}

fn vec3_coordinate_from_json(name: &str, value: &serde_json::Value) -> Result<f32, String> {
    json_f32(value).map_err(|error| public_error::mcp_vec3_input(format!("Vec3.{name}: {error}")))
}

pub(crate) fn json_f64(value: &serde_json::Value) -> Result<f64, String> {
    value
        .as_f64()
        .or_else(|| nonfinite_float_from_json(value))
        .ok_or_else(|| {
            if value.is_null() {
                public_error::COMPONENT_FLOAT_REQUIRED.to_string()
            } else {
                public_error::component_float_expected_number(value)
            }
        })
        .and_then(require_finite_float)
}

pub(crate) fn json_f32(value: &serde_json::Value) -> Result<f32, String> {
    let narrowed = json_f64(value)? as f32;
    if narrowed.is_finite() {
        Ok(narrowed)
    } else {
        Err(public_error::COMPONENT_FLOAT_OUT_OF_F32_RANGE.to_string())
    }
}

pub(crate) fn json_kind_name(value: &serde_json::Value) -> &'static str {
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
    use std::{f32::consts::FRAC_1_SQRT_2, f64::consts::PI};

    use pybevy_core::public_error;
    use serde_json::json;

    use super::{EnumPayload, enum_to_json, json_f32, json_f64, unit_quat_from_json};

    #[test]
    fn json_f64_accepts_json_numbers() {
        for (value, expected) in [
            (json!(PI), PI),
            (json!(42), 42.0),
            (json!(-7.5), -7.5),
            (json!(0), 0.0),
        ] {
            assert_eq!(json_f64(&value), Ok(expected));
        }
    }

    #[test]
    fn json_f64_names_the_value_it_rejects() {
        for (value, message) in [
            (
                json!("not a number"),
                r#"expected a number, got "not a number""#,
            ),
            (json!(true), "expected a number, got true"),
            (json!([1, 2, 3]), "expected a number, got [1,2,3]"),
            (json!(null), public_error::COMPONENT_FLOAT_REQUIRED),
        ] {
            assert_eq!(json_f64(&value), Err(message.to_string()));
        }
        for spelling in ["NaN", "Infinity", "-Infinity"] {
            assert_eq!(
                json_f64(&json!(spelling)),
                Err(public_error::COMPONENT_FLOAT_NON_FINITE.to_string())
            );
        }
    }

    #[test]
    fn json_f32_rejects_numbers_outside_the_f32_range() {
        assert_eq!(json_f32(&json!(1.5)), Ok(1.5));
        assert_eq!(
            json_f32(&json!(1e39)),
            Err(public_error::COMPONENT_FLOAT_OUT_OF_F32_RANGE.to_string())
        );
    }

    #[test]
    fn quaternion_json_normalizes_arrays_and_objects() {
        for value in [
            serde_json::json!([0.0, 0.0, 0.7, 0.7]),
            serde_json::json!({"x": 0.0, "y": 0.0, "z": 0.7, "w": 0.7}),
        ] {
            let quat = unit_quat_from_json(&value).unwrap();
            assert!((quat.length() - 1.0).abs() < 1e-6);
            assert!((quat.z - FRAC_1_SQRT_2).abs() < 1e-6);
        }
    }

    #[test]
    fn quaternion_json_rejects_zero_and_nonfinite_lengths() {
        assert_eq!(
            unit_quat_from_json(&serde_json::json!([0, 0, 0, 0])).unwrap_err(),
            "Quat rotation must have finite, non-zero length"
        );
        assert!(unit_quat_from_json(&serde_json::json!([0, 0, 0, "Infinity"])).is_err());
    }

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
