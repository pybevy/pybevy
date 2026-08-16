//! Static MCP resource definitions exposed by the engine.
//!
//! Single source of truth for the resources advertised through `resources/list`.
//! Python (`pybevy.mcp.definitions`) consumes these via the `rust_resource_definitions`
//! PyO3 binding so that adding/renaming a resource only requires a change here.

use serde_json::{Value, json};

/// Generate the full list of MCP resource definitions exposed by the engine.
///
/// Each entry has either `uri` or `uriTemplate`, plus `name`, `description`,
/// `mimeType`, and `feature_gate`.
/// `feature_gate` is either `null` or the name of a feature flag the bridge
/// must enable in order to advertise the resource (e.g. `api_discovery`).
pub fn list_resources() -> Vec<Value> {
    vec![
        json!({
            "uri": "guide://index",
            "name": "Guide Index",
            "description": "List of available guides with names and descriptions",
            "mimeType": "application/json",
            "feature_gate": null,
        }),
        json!({
            "uri": "api://index",
            "name": "API Index",
            "description": "Module names with class/function lists (lightweight, no content)",
            "mimeType": "application/json",
            "feature_gate": "api_discovery",
        }),
        json!({
            "uri": "scene://entities",
            "name": "Entity List",
            "description": "All entities with their component types and Names",
            "mimeType": "application/json",
            "feature_gate": null,
        }),
        json!({
            "uri": "scene://resources",
            "name": "Resource List",
            "description": "All resources and their values",
            "mimeType": "application/json",
            "feature_gate": null,
        }),
        json!({
            "uri": "scene://systems",
            "name": "Scene System List",
            "description": "Scene-owned systems by stage, excluding engine internals",
            "mimeType": "application/json",
            "feature_gate": null,
        }),
        json!({
            "uri": "scene://systems/all",
            "name": "Full System List",
            "description": "Complete scheduler diagnostic including Bevy, PyBevy, and host-internal systems",
            "mimeType": "application/json",
            "feature_gate": null,
        }),
        json!({
            "uri": "scene://debug",
            "name": "Debug Info",
            "description": "FPS, CPU, GPU, RAM, VRAM, entity/asset counts, system profiling",
            "mimeType": "application/json",
            "feature_gate": null,
        }),
        json!({
            "uri": "scene://components",
            "name": "Component Registry",
            "description": "Registered component bridges and resource bridges; sample of named entities with detected component types.",
            "mimeType": "application/json",
            "feature_gate": null,
        }),
        json!({
            "uriTemplate": "scene://entity/{name_or_id}",
            "name": "Entity Detail (templated)",
            "description": "Inspect a single entity by Name or numeric ID. Example: scene://entity/MainCamera or scene://entity/4294967296.",
            "mimeType": "application/json",
            "feature_gate": null,
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_resources_produces_expected_uris() {
        let resources = list_resources();
        let uris: Vec<&str> = resources
            .iter()
            .filter_map(|r| r.get("uri").and_then(|u| u.as_str()))
            .collect();

        assert!(uris.contains(&"guide://index"));
        assert!(uris.contains(&"api://index"));
        assert!(uris.contains(&"scene://entities"));
        assert!(uris.contains(&"scene://resources"));
        assert!(uris.contains(&"scene://systems"));
        assert!(uris.contains(&"scene://systems/all"));
        assert!(uris.contains(&"scene://debug"));
        assert!(uris.contains(&"scene://components"));
        assert!(!uris.contains(&"scene://entity/{name_or_id}"));

        let templates: Vec<&str> = resources
            .iter()
            .filter_map(|r| r.get("uriTemplate").and_then(|u| u.as_str()))
            .collect();
        assert_eq!(templates, vec!["scene://entity/{name_or_id}"]);
    }

    #[test]
    fn every_resource_has_required_fields() {
        for res in list_resources() {
            assert!(
                res.get("uri").and_then(|v| v.as_str()).is_some()
                    || res.get("uriTemplate").and_then(|v| v.as_str()).is_some()
            );
            assert!(res.get("name").and_then(|v| v.as_str()).is_some());
            assert!(res.get("description").and_then(|v| v.as_str()).is_some());
            assert!(res.get("mimeType").and_then(|v| v.as_str()).is_some());
            // feature_gate must be present (may be null)
            assert!(res.get("feature_gate").is_some());
        }
    }

    #[test]
    fn api_index_is_feature_gated() {
        let resources = list_resources();
        let api_index = resources
            .iter()
            .find(|r| r.get("uri").and_then(|u| u.as_str()) == Some("api://index"))
            .expect("api://index missing");
        assert_eq!(
            api_index.get("feature_gate").and_then(|v| v.as_str()),
            Some("api_discovery")
        );
    }
}
