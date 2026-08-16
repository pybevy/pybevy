use serde_json::{Number, Value};

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
}
