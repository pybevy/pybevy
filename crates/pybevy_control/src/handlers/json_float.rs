use pybevy_core::public_error;
use serde_json::{Number, Value};

pub(crate) fn require_finite_float(value: f64) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(public_error::COMPONENT_FLOAT_NON_FINITE.to_string())
    }
}

pub(crate) fn reject_narrowed_nonfinite(request: &Value, written: &Value) -> Result<(), String> {
    match (request, written) {
        (Value::Number(number), Value::String(value))
            if number
                .as_f64()
                .is_some_and(|number| number.is_finite() && number.abs() > f64::from(f32::MAX))
                && matches!(value.as_str(), "NaN" | "Infinity" | "-Infinity") =>
        {
            Err("number is outside the f32 range".to_string())
        }
        (Value::Object(request), Value::Object(written)) => {
            for (name, value) in request {
                if let Some(stored) = written.get(name) {
                    reject_narrowed_nonfinite(value, stored)
                        .map_err(|error| format!("{name}: {error}"))?;
                }
            }
            Ok(())
        }
        (Value::Array(request), Value::Array(written)) => {
            for (index, (value, stored)) in request.iter().zip(written).enumerate() {
                reject_narrowed_nonfinite(value, stored)
                    .map_err(|error| format!("{index}: {error}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(crate) fn float_to_json(value: f64) -> Value {
    if value.is_nan() {
        return Value::String("NaN".to_string());
    }
    if value == f64::INFINITY {
        return Value::String("Infinity".to_string());
    }
    if value == f64::NEG_INFINITY {
        return Value::String("-Infinity".to_string());
    }
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}

pub(crate) fn nonfinite_float_from_json(value: &Value) -> Option<f64> {
    match value.as_str()? {
        "NaN" | "NAN" => Some(f64::NAN),
        "Infinity" | "INFINITY" => Some(f64::INFINITY),
        "-Infinity" | "NEG_INFINITY" => Some(f64::NEG_INFINITY),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonfinite_values_have_distinct_round_trip_spellings() {
        for (value, spelling) in [
            (f64::NAN, "NaN"),
            (f64::INFINITY, "Infinity"),
            (f64::NEG_INFINITY, "-Infinity"),
        ] {
            let json = float_to_json(value);
            assert_eq!(json, Value::String(spelling.to_string()));
            let decoded = nonfinite_float_from_json(&json).unwrap();
            assert_eq!(decoded.is_nan(), value.is_nan());
            if !value.is_nan() {
                assert_eq!(decoded, value);
            }
        }
    }

    #[test]
    fn finite_values_remain_json_numbers() {
        assert_eq!(float_to_json(1.25), serde_json::json!(1.25));
        assert!(nonfinite_float_from_json(&serde_json::json!(1.25)).is_none());
    }

    #[test]
    fn detects_only_new_nonfinite_narrowing_at_matching_paths() {
        let request = serde_json::json!({"value": {"fov": 1e39}, "untouched": 1.0});
        let written = serde_json::json!({"value": {"fov": "Infinity"}, "untouched": 1.0});
        assert_eq!(
            reject_narrowed_nonfinite(&request, &written).unwrap_err(),
            "value: fov: number is outside the f32 range"
        );
        assert!(
            reject_narrowed_nonfinite(
                &serde_json::json!({"elapsed": 1e39}),
                &serde_json::json!({"elapsed": 1e39, "existing": "Infinity"})
            )
            .is_ok()
        );
    }

    #[test]
    fn in_range_numbers_may_become_nonfinite_through_the_native_constructor() {
        let request = serde_json::json!({"normal_d": [0.0, 0.0, 0.0, 0.0]});
        let written = serde_json::json!({"normal_d": ["NaN", "NaN", "NaN", "NaN"]});
        assert!(reject_narrowed_nonfinite(&request, &written).is_ok());
        assert_eq!(
            reject_narrowed_nonfinite(
                &serde_json::json!({"value": -3.5e38}),
                &serde_json::json!({"value": "-Infinity"})
            )
            .unwrap_err(),
            "value: number is outside the f32 range"
        );
    }
}
