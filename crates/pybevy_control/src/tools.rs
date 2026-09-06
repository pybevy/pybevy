//! Generate MCP tool definitions from the ControlOperation JSON schema.
//!
//! Transforms schemars output into the MCP tools/list response format.

use serde_json::{Map, Value, json};

use crate::bridge::ControlOperation;

/// Generate the full list of MCP tool definitions from ControlOperation's JSON schema.
///
/// Each tool has: `name`, `description`, `inputSchema`, and `feature_gate`.
pub fn list_tools() -> Vec<Value> {
    let root = schemars::schema_for!(ControlOperation);
    let root_value = serde_json::to_value(&root).expect("schema serialization");

    // Extract definitions for $ref resolution (schemars 1.x uses $defs, 0.8 uses definitions)
    let definitions = root_value
        .get("$defs")
        .or_else(|| root_value.get("definitions"))
        .and_then(|d| d.as_object())
        .cloned()
        .unwrap_or_default();

    // The schema is a oneOf array at the top level
    let variants = root_value
        .get("oneOf")
        .and_then(|o| o.as_array())
        .expect("ControlOperation schema must have oneOf");

    let mut tools = Vec::new();

    for variant in variants {
        let obj = match variant.as_object() {
            Some(o) => o,
            None => continue,
        };

        // Extract tool name from properties.tool.const (schemars 1.x)
        // or properties.tool.enum[0] (schemars 0.8)
        let tool_prop = obj.get("properties").and_then(|p| p.get("tool"));
        let tool_name = tool_prop
            .and_then(|t| {
                t.get("const").and_then(|c| c.as_str()).or_else(|| {
                    t.get("enum")
                        .and_then(|e| e.as_array())
                        .and_then(|a| a.first())
                        .and_then(|v| v.as_str())
                })
            })
            .unwrap_or("");

        // Skip variants marked with #[schemars(extend("x-hidden" = true))]
        let is_hidden = obj
            .get("x-hidden")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if tool_name.is_empty() || is_hidden {
            continue;
        }

        let description = obj
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("");

        // Build inputSchema: take properties (minus "tool"), required (minus "tool").
        // For newtype variants, schemars puts params in allOf/$ref instead of inline properties.
        let mut properties = obj
            .get("properties")
            .and_then(|p| p.as_object())
            .cloned()
            .unwrap_or_default();
        properties.remove("tool");

        let mut required: Vec<String> = obj
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .filter(|s| *s != "tool")
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        // Merge properties/required from $ref or allOf (newtype variant pattern).
        // schemars generates newtype variants as { "$ref": "...", "properties": { "tool": ... } }
        let refs_to_merge: Vec<Value> = if let Some(ref_path) = obj.get("$ref") {
            vec![json!({"$ref": ref_path})]
        } else if let Some(all_of) = obj.get("allOf").and_then(|a| a.as_array()) {
            all_of.clone()
        } else {
            vec![]
        };
        for entry in &refs_to_merge {
            let mut resolved = entry.clone();
            resolve_refs(&mut resolved, &definitions);
            if let Some(ref_props) = resolved.get("properties").and_then(|p| p.as_object()) {
                for (k, v) in ref_props {
                    properties.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
            if let Some(ref_req) = resolved.get("required").and_then(|r| r.as_array()) {
                for r in ref_req {
                    if let Some(s) = r.as_str()
                        && !required.contains(&s.to_string())
                    {
                        required.push(s.to_string());
                    }
                }
            }
        }

        // Resolve $ref in property values and clean up schemars noise
        for (_key, prop_value) in properties.iter_mut() {
            resolve_refs(prop_value, &definitions);
            clean_schema_noise(prop_value);
        }

        required.sort();

        let mut input_schema = json!({
            "type": "object",
            "properties": properties,
            "additionalProperties": false,
        });

        if !required.is_empty() {
            input_schema["required"] = json!(required);
        }

        let gate = obj.get("x-feature-gate").and_then(|v| v.as_str());

        tools.push(json!({
            "name": tool_name,
            "description": description,
            "inputSchema": input_schema,
            "feature_gate": gate,
        }));
    }

    tools
}

/// Resolve `$ref` and `allOf` wrappers inline, replacing them with the referenced definition.
fn resolve_refs(value: &mut Value, definitions: &Map<String, Value>) {
    match value {
        Value::Object(obj) => {
            // Handle allOf with a single $ref entry (schemars pattern for described refs)
            if let Some(all_of) = obj.get("allOf").and_then(|a| a.as_array())
                && all_of.len() == 1
                && let Some(ref_path) = all_of[0].get("$ref").and_then(|r| r.as_str())
            {
                let def_name = ref_path
                    .trim_start_matches("#/$defs/")
                    .trim_start_matches("#/definitions/");
                if let Some(def) = definitions.get(def_name) {
                    // Preserve the description from the field, merge with definition
                    let desc = obj.get("description").cloned();
                    *value = def.clone();
                    if let (Some(d), Some(obj)) = (desc, value.as_object_mut()) {
                        obj.insert("description".to_string(), d);
                    }
                    return;
                }
            }
            // Handle direct $ref
            if let Some(ref_path) = obj.get("$ref").and_then(|r| r.as_str()) {
                let def_name = ref_path
                    .trim_start_matches("#/$defs/")
                    .trim_start_matches("#/definitions/");
                if let Some(def) = definitions.get(def_name) {
                    *value = def.clone();
                    return;
                }
            }
            // Recurse into children
            for (_, v) in obj.iter_mut() {
                resolve_refs(v, definitions);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                resolve_refs(v, definitions);
            }
        }
        _ => {}
    }
}

/// Strip schemars noise that wastes MCP context: `format`, null defaults,
/// `minimum` on integers, and simplify `["type", "null"]` arrays to just
/// `"type"` (MCP always allows omission).
fn clean_schema_noise(value: &mut Value) {
    match value {
        Value::Object(obj) => {
            if let Some(any_of) = obj.get("anyOf").and_then(Value::as_array) {
                let non_null = any_of
                    .iter()
                    .filter(|entry| entry.get("type") != Some(&json!("null")))
                    .collect::<Vec<_>>();
                if non_null.len() == 1 && non_null.len() < any_of.len() {
                    let description = obj.get("description").cloned();
                    *value = non_null[0].clone();
                    if let (Some(description), Some(replacement)) =
                        (description, value.as_object_mut())
                    {
                        replacement.insert("description".to_string(), description);
                    }
                    clean_schema_noise(value);
                    return;
                }
            }
            obj.remove("format");
            if obj.get("default").is_some_and(Value::is_null) {
                obj.remove("default");
            }
            // Remove minimum: 0 on unsigned integers (not useful for LLMs)
            if obj.get("minimum") == Some(&json!(0)) {
                obj.remove("minimum");
            }
            // Simplify ["integer", "null"] -> "integer", ["boolean", "null"] -> "boolean", etc.
            if let Some(type_val) = obj.get("type").cloned()
                && let Some(arr) = type_val.as_array()
            {
                let non_null: Vec<&Value> =
                    arr.iter().filter(|v| v.as_str() != Some("null")).collect();
                if non_null.len() == 1 {
                    obj.insert("type".to_string(), non_null[0].clone());
                }
            }
            // Recurse into nested schemas (items, properties, etc.)
            for (_, v) in obj.iter_mut() {
                clean_schema_noise(v);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                clean_schema_noise(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_tools_produces_expected_tools() {
        let tools = list_tools();
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
            .collect();

        // Check expected MCP tools are present
        assert!(names.contains(&"query_entities"), "missing query_entities");
        assert!(
            names.contains(&"capture_screenshot"),
            "missing capture_screenshot"
        );
        assert!(names.contains(&"capture_stats"), "missing capture_stats");
        assert!(names.contains(&"compare_frames"), "missing compare_frames");
        assert!(names.contains(&"spawn_entity"), "missing spawn_entity");
        assert!(names.contains(&"set_resource"), "missing set_resource");
        assert!(names.contains(&"get_resource"), "missing get_resource");
        assert!(
            names.contains(&"get_system_list"),
            "missing get_system_list"
        );
        assert!(names.contains(&"set_asset"), "missing set_asset");
        assert!(
            names.contains(&"schedule_actions"),
            "missing schedule_actions"
        );
        assert!(names.contains(&"pause_time"), "missing pause_time");

        // Internal tools must NOT appear
        assert!(
            !names.contains(&"list_entities"),
            "list_entities should be hidden"
        );
        assert!(
            !names.contains(&"get_entity"),
            "get_entity should be hidden"
        );
        assert!(
            !names.contains(&"list_resources"),
            "list_resources should be hidden"
        );
        assert!(
            !names.contains(&"capture_with_gizmos"),
            "capture_with_gizmos should be hidden"
        );
        assert!(
            names.contains(&"query_spatial_neighborhood"),
            "missing query_spatial_neighborhood"
        );
        assert!(
            names.contains(&"check_all_overlaps"),
            "missing check_all_overlaps"
        );
    }

    #[test]
    fn list_tools_have_descriptions() {
        let tools = list_tools();
        for tool in &tools {
            let name = tool["name"].as_str().unwrap();
            let desc = tool["description"].as_str().unwrap();
            assert!(!desc.is_empty(), "tool {name} has empty description");
        }
    }

    #[test]
    fn get_system_list_advertises_internal_filter() {
        let tools = list_tools();
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == "get_system_list")
            .expect("get_system_list tool");
        let include_internal = &tool["inputSchema"]["properties"]["include_internal"];
        assert_eq!(include_internal["type"], "boolean");
        assert_eq!(include_internal["default"], false);
    }

    #[test]
    fn get_resource_advertises_an_actual_resource_example() {
        let tools = list_tools();
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == "get_resource")
            .expect("get_resource tool");
        let description = tool["inputSchema"]["properties"]["resource_type"]["description"]
            .as_str()
            .expect("resource_type description");

        assert!(description.contains("ClearColor"));
        assert!(!description.contains("AmbientLight"));
    }

    #[test]
    fn list_tools_have_input_schema() {
        let tools = list_tools();
        for tool in &tools {
            let name = tool["name"].as_str().unwrap();
            let schema = tool.get("inputSchema");
            assert!(schema.is_some(), "tool {name} missing inputSchema");
            assert_eq!(
                schema.unwrap()["type"],
                "object",
                "tool {name} inputSchema.type != object"
            );
        }
    }

    #[test]
    fn list_tools_reject_unknown_top_level_parameters() {
        for tool in list_tools() {
            assert_eq!(
                tool["inputSchema"]["additionalProperties"], false,
                "tool {} must advertise a closed argument object",
                tool["name"]
            );
        }
    }

    #[test]
    fn spatial_neighborhood_radius_advertises_non_negative_minimum() {
        let tools = list_tools();
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == "query_spatial_neighborhood")
            .unwrap();

        assert_eq!(tool["inputSchema"]["properties"]["radius"]["minimum"], 0.0);
    }

    #[test]
    fn turnaround_distance_advertises_positive_minimum() {
        let tools = list_tools();
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == "capture_turnaround")
            .unwrap();

        assert_eq!(
            tool["inputSchema"]["properties"]["distance"]["exclusiveMinimum"],
            0.0
        );
    }

    #[test]
    fn turnaround_view_count_advertises_supported_range() {
        let tools = list_tools();
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == "capture_turnaround")
            .unwrap();
        let view_count = &tool["inputSchema"]["properties"]["view_count"];

        assert_eq!(view_count["minimum"], 1);
        assert_eq!(view_count["maximum"], 20);
    }

    #[test]
    fn manipulation_object_params_advertise_object_type() {
        // Regression: spawn_entity.components / set_component.fields /
        // set_resource.value / set_asset.fields are serde_json::Value, which
        // schemars renders without a "type". MCP clients then send the argument
        // as a JSON string and the handlers reject it ("must be a JSON object").
        // Each object param must advertise type: object.
        let tools = list_tools();
        let find = |name: &str| -> &Value { tools.iter().find(|t| t["name"] == name).unwrap() };
        for (tool, param) in [
            ("spawn_entity", "components"),
            ("set_component", "fields"),
            ("set_resource", "value"),
            ("set_asset", "fields"),
        ] {
            assert_eq!(
                find(tool)["inputSchema"]["properties"][param]["type"],
                "object",
                "{tool}.{param} must advertise type: object so clients send an object, not a string",
            );
        }
    }

    #[test]
    fn set_resource_describes_custom_field_declarations() {
        let tools = list_tools();
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == "set_resource")
            .unwrap();
        assert!(tool["description"].as_str().unwrap().contains("@dataclass"));
        assert!(
            tool["inputSchema"]["properties"]["value"]["description"]
                .as_str()
                .unwrap()
                .contains("@dataclass")
        );
    }

    #[test]
    fn batch_operations_advertise_object_items_schema() {
        // `Vec<serde_json::Value>` normally becomes `{"items": true}`. That is
        // valid JSON Schema, but Moonshot and DeepSeek require `items` to be a
        // schema object when validating function parameters.
        let tools = list_tools();
        let batch = tools.iter().find(|tool| tool["name"] == "batch").unwrap();
        let operations = &batch["inputSchema"]["properties"]["operations"];

        assert_eq!(operations["type"], "array");
        assert_eq!(operations["items"]["type"], "object");
    }

    #[test]
    fn schedule_action_args_advertise_object_schema_without_null_default() {
        // Unconstrained `serde_json::Value` otherwise emits only
        // `{"default": null, "description": ...}`, which DeepSeek cannot
        // interpret as a function-parameter schema.
        let tools = list_tools();
        let schedule = tools
            .iter()
            .find(|tool| tool["name"] == "schedule_actions")
            .unwrap();
        let args = &schedule["inputSchema"]["properties"]["actions"]["items"]["properties"]["args"];

        assert_eq!(args["type"], "object");
        assert!(args.get("default").is_none());
    }

    #[test]
    fn list_tools_feature_gates_correct() {
        let tools = list_tools();
        let find = |name: &str| -> &Value { tools.iter().find(|t| t["name"] == name).unwrap() };

        assert_eq!(find("capture_screenshot")["feature_gate"], "screenshot");
        assert_eq!(find("capture_stats")["feature_gate"], "screenshot");
        assert_eq!(find("compare_frames")["feature_gate"], "screenshot");
        assert_eq!(find("spawn_entity")["feature_gate"], "manipulation");
        assert_eq!(find("run_code")["feature_gate"], "execute_python");
        assert!(find("query_entities")["feature_gate"].is_null());
        assert!(find("pause_time")["feature_gate"].is_null());
    }

    #[test]
    fn list_tools_no_tool_discriminator_in_schema() {
        let tools = list_tools();
        for tool in &tools {
            let name = tool["name"].as_str().unwrap();
            let props = tool["inputSchema"]["properties"].as_object();
            if let Some(p) = props {
                assert!(
                    !p.contains_key("tool"),
                    "tool {name} has 'tool' discriminator in inputSchema"
                );
            }
        }
    }

    #[test]
    fn list_tools_refs_resolved() {
        let tools = list_tools();
        let json = serde_json::to_string(&tools).unwrap();
        assert!(
            !json.contains("$ref"),
            "unresolved $ref found in tool schemas"
        );
    }

    #[test]
    fn capture_screenshot_has_defaults() {
        let tools = list_tools();
        let tool = tools
            .iter()
            .find(|t| t["name"] == "capture_screenshot")
            .unwrap();
        let props = &tool["inputSchema"]["properties"];

        assert_eq!(props["delay_frames"]["default"], 2);
        assert_eq!(props["hide_ui"]["default"], true);
        assert_eq!(props["entity"]["anyOf"][0]["type"], "integer");
        assert_eq!(props["entity"]["anyOf"][1]["type"], "string");
    }

    #[test]
    fn capture_timeline_has_visibility_defaults() {
        let tools = list_tools();
        let tool = tools
            .iter()
            .find(|t| t["name"] == "capture_timeline")
            .unwrap();
        let props = &tool["inputSchema"]["properties"];

        assert_eq!(props["hide_ui"]["default"], true);
        assert_eq!(props["gizmos"]["default"], false);
    }

    #[test]
    fn capture_depth_grid_density_has_positive_minimum() {
        let tools = list_tools();
        let tool = tools.iter().find(|t| t["name"] == "capture_depth").unwrap();

        assert_eq!(
            tool["inputSchema"]["properties"]["grid_density"]["minimum"],
            1
        );
    }

    #[test]
    fn frame_analysis_tools_advertise_bounded_inputs() {
        let tools = list_tools();
        let stats = tools.iter().find(|t| t["name"] == "capture_stats").unwrap();
        let stats_properties = &stats["inputSchema"]["properties"];
        assert_eq!(stats_properties["grid"]["default"], 1);
        assert_eq!(stats_properties["grid"]["minimum"], 1);
        assert_eq!(stats_properties["grid"]["maximum"], 16);
        assert_eq!(stats_properties["sample_points"]["maxItems"], 256);
        assert_eq!(stats_properties["max_width"]["minimum"], 1);

        let compare = tools
            .iter()
            .find(|t| t["name"] == "compare_frames")
            .unwrap();
        let epsilon = &compare["inputSchema"]["properties"]["epsilon"];
        assert_eq!(epsilon["minimum"], 0.0);
        assert_eq!(epsilon["maximum"], 1.0);
    }
}
