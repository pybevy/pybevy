use std::collections::HashMap;

use bevy::{ecs::world::World, prelude::Resource};
use pyo3::prelude::*;

use crate::bridge::ControlError;

/// Registry for custom tools registered from Python
#[derive(Resource, Default)]
pub struct CustomToolRegistry {
    pub tools: HashMap<String, CustomTool>,
}

pub struct CustomTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub python_function: Py<PyAny>,
}

/// Call a custom tool or register a new one
pub fn call_custom_tool(
    world: &mut World,
    name: String,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ControlError> {
    if name == "__register" {
        return register_tool(world, arguments);
    }

    // Look up the custom tool and clone the function ref with GIL
    let func = Python::attach(|py| {
        let registry = world.get_resource::<CustomToolRegistry>();
        registry
            .and_then(|r| r.tools.get(&name))
            .map(|t| t.python_function.clone_ref(py))
            .ok_or_else(|| ControlError::not_found(format!("Custom tool '{name}' not found")))
    })?;

    // Execute the Python function
    Python::attach(|py| {
        let args_str = serde_json::to_string(&arguments).unwrap_or_default();
        let py_args = py
            .import("json")
            .and_then(|json_mod| json_mod.call_method1("loads", (args_str,)))
            .map_err(|e| ControlError::internal(format!("Failed to parse arguments: {e}")))?;

        let result = func
            .bind(py)
            .call1((py_args,))
            .map_err(|e| ControlError::internal(format!("Custom tool error: {e}")))?;

        // Convert result to JSON
        let result_str: String = result
            .repr()
            .map(|r| r.to_string())
            .unwrap_or_else(|_| "null".to_string());

        Ok(serde_json::json!({
            "tool": name,
            "result": result_str,
        }))
    })
}

/// Register a new custom tool
fn register_tool(
    world: &mut World,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ControlError> {
    let name = arguments
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ControlError::invalid_params("Missing 'name'"))?;
    let description = arguments
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let input_schema = arguments
        .get("input_schema")
        .cloned()
        .unwrap_or(serde_json::json!({"type": "object", "properties": {}}));
    let python_function = arguments
        .get("python_function")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ControlError::invalid_params("Missing 'python_function'"))?;

    let full_name = if name.starts_with("custom.") {
        name.to_string()
    } else {
        format!("custom.{name}")
    };

    // Resolve the Python function
    let func = Python::attach(|py| {
        // Try to eval the function reference
        let c_code = std::ffi::CString::new(python_function)
            .map_err(|e| ControlError::invalid_params(format!("Invalid function: {e}")))?;
        let result = py
            .eval(&c_code, None, None)
            .map_err(|e| ControlError::internal(format!("Failed to resolve function: {e}")))?;
        Ok::<Py<PyAny>, ControlError>(result.unbind())
    })?;

    let mut registry = world.get_resource_or_insert_with(CustomToolRegistry::default);
    registry.tools.insert(
        full_name.clone(),
        CustomTool {
            name: full_name.clone(),
            description,
            input_schema,
            python_function: func,
        },
    );

    // Notify via SSE if available
    if let Some(broadcaster) = world.get_resource::<crate::bridge::SseEventBroadcaster>() {
        broadcaster.tool_registered(&full_name);
    }

    Ok(serde_json::json!({
        "registered": full_name,
    }))
}
