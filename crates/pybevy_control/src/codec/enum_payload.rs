use serde_json::{Map, Value};

/// Splits a tagged enum object into its variant name and payload.
///
/// The payload is null when only `variant` is present, the bare `value` when
/// it is the only other key, and the remaining object otherwise.
pub(crate) fn normalize(fields: &Map<String, Value>) -> Option<(&str, Value)> {
    let Some(Value::String(name)) = fields.get("variant") else {
        return None;
    };
    let mut payload = fields.clone();
    payload.remove("variant");
    let payload = if payload.is_empty() {
        Value::Null
    } else if payload.len() == 1 && payload.contains_key("value") {
        payload.remove("value").unwrap()
    } else {
        Value::Object(payload)
    };
    Some((name, payload))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn split(value: Value) -> Option<(String, Value)> {
        normalize(value.as_object().unwrap()).map(|(name, payload)| (name.to_owned(), payload))
    }

    #[test]
    fn payload_shapes() {
        assert_eq!(
            split(json!({"variant": "Hidden"})),
            Some(("Hidden".into(), Value::Null))
        );
        assert_eq!(
            split(json!({"variant": "Px", "value": 12.0})),
            Some(("Px".into(), json!(12.0)))
        );
        assert_eq!(
            split(json!({"variant": "Rect", "min": 1, "max": 2})),
            Some(("Rect".into(), json!({"min": 1, "max": 2})))
        );
        assert_eq!(
            split(json!({"variant": "Wrap", "value": 1, "extra": 2})),
            Some(("Wrap".into(), json!({"value": 1, "extra": 2})))
        );
        assert_eq!(
            split(json!({"variant": "One", "other": 3})),
            Some(("One".into(), json!({"other": 3})))
        );
    }

    #[test]
    fn requires_a_string_variant() {
        assert_eq!(split(json!({"value": 1})), None);
        assert_eq!(split(json!({"variant": 1, "value": 1})), None);
    }
}
