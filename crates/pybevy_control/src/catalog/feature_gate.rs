use crate::{
    plugin::ControlConfig,
    protocol::{ControlError, ControlOperation, FeatureGate, OperationTag},
};

impl FeatureGate {
    #[cfg(test)]
    fn schema_name(self) -> &'static str {
        match self {
            Self::Screenshot => "screenshot",
            Self::Manipulation => "manipulation",
            Self::ExecutePython => "execute_python",
        }
    }

    fn enabled(self, config: &ControlConfig) -> bool {
        match self {
            Self::Screenshot => config.screenshot,
            Self::Manipulation => config.manipulation,
            Self::ExecutePython => config.execute_python,
        }
    }

    fn disabled_message(self) -> &'static str {
        match self {
            Self::Screenshot => "Screenshot disabled",
            Self::Manipulation => "Manipulation disabled",
            Self::ExecutePython => "Python execution disabled",
        }
    }

    fn check(self, config: Option<&ControlConfig>) -> Result<(), &'static str> {
        if config.is_some_and(|config| self.enabled(config)) {
            Ok(())
        } else {
            Err(self.disabled_message())
        }
    }
}

/// Reject an operation, or any action of a schedule, whose feature switch is off.
pub(crate) fn authorize(
    operation: &ControlOperation,
    config: Option<&ControlConfig>,
) -> Result<(), ControlError> {
    if let ControlOperation::ScheduleActions(request) = operation {
        for (index, action) in request.actions.iter().enumerate() {
            if let Some(gate) =
                OperationTag::from_name(&action.tool).and_then(|tag| tag.spec().gate)
            {
                gate.check(config).map_err(|message| {
                    ControlError::forbidden(format!("action[{index}]: {message}"))
                })?;
            }
        }
        return Ok(());
    }
    match operation.spec().gate {
        Some(gate) => gate.check(config).map_err(ControlError::forbidden),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::*;
    use crate::{catalog::tools::list_tools, protocol::ErrorCode, schedule::tool_to_operation};

    fn config(screenshot: bool, manipulation: bool, execute_python: bool) -> ControlConfig {
        ControlConfig {
            port: 0,
            host: "127.0.0.1".into(),
            screenshot,
            manipulation,
            execute_python,
            api_discovery: true,
        }
    }

    fn placeholder(schema: &Value) -> Value {
        if let Some(first) = schema
            .get("anyOf")
            .or_else(|| schema.get("oneOf"))
            .and_then(Value::as_array)
            .and_then(|branches| branches.first())
        {
            return placeholder(first);
        }
        if let Some(first) = schema
            .get("enum")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
        {
            return first.clone();
        }
        match schema.get("type").and_then(Value::as_str) {
            Some("string") => json!("x"),
            Some("integer") => json!(1),
            Some("number") => json!(1.0),
            Some("boolean") => json!(false),
            Some("array") => json!([]),
            _ => json!({}),
        }
    }

    fn minimal_arguments(tool: &Value) -> Value {
        let schema = &tool["inputSchema"];
        let mut arguments = Map::new();
        for name in schema["required"].as_array().into_iter().flatten() {
            let name = name.as_str().unwrap();
            arguments.insert(name.to_owned(), placeholder(&schema["properties"][name]));
        }
        Value::Object(arguments)
    }

    #[test]
    fn operation_gates_match_published_tool_schemas() {
        for tool in list_tools() {
            let name = tool["name"].as_str().unwrap();
            let operation = tool_to_operation(name, &minimal_arguments(&tool))
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(
                operation.spec().gate.map(FeatureGate::schema_name),
                tool["feature_gate"].as_str(),
                "{name}"
            );
        }
    }

    #[test]
    fn disabled_switch_rejects_operation_as_forbidden() {
        let operation = tool_to_operation("run_code", &json!({"code": "1"})).unwrap();
        let error = authorize(&operation, Some(&config(true, true, false))).unwrap_err();
        assert_eq!(error.code, ErrorCode::Forbidden);
        assert_eq!(error.message, "Python execution disabled");
        assert!(authorize(&operation, Some(&config(false, false, true))).is_ok());
    }

    #[test]
    fn schedule_is_rejected_when_any_action_is_disabled() {
        let operation = ControlOperation::ScheduleActions(
            serde_json::from_value(json!({
                "actions": [
                    {"tool": "pause_time"},
                    {"tool": "capture_stats"},
                    {"tool": "spawn_entity", "args": {"components": {}}},
                ]
            }))
            .unwrap(),
        );
        let error = authorize(&operation, Some(&config(true, false, false))).unwrap_err();
        assert_eq!(error.code, ErrorCode::Forbidden);
        assert_eq!(error.message, "action[2]: Manipulation disabled");
        assert!(authorize(&operation, Some(&config(true, true, false))).is_ok());
    }

    #[test]
    fn missing_config_fails_closed() {
        let operation = tool_to_operation("capture_screenshot", &json!({})).unwrap();
        let error = authorize(&operation, None).unwrap_err();
        assert_eq!(error.message, "Screenshot disabled");
        let ungated = tool_to_operation("get_time_status", &json!({})).unwrap();
        assert!(authorize(&ungated, None).is_ok());
    }
}
