use bevy::ecs::world::World;

use crate::{
    protocol::{ControlError, EntityRef},
    pyo3::components::{remove_component, set_component, spawn_entity},
    scene::entity::{despawn_entity, has_embedded_errors},
};

/// Execute a batch of mutation operations in a single World access.
/// Each operation runs independently - failures don't abort the batch.
pub fn batch_mutate(
    world: &mut World,
    operations: Vec<serde_json::Value>,
) -> Result<serde_json::Value, ControlError> {
    let mut results = Vec::with_capacity(operations.len());

    for (i, op) in operations.iter().enumerate() {
        let result = validate_batch_operation(i, op).and_then(|action| match action {
            "set_component" => {
                let entity_ref = parse_entity_ref_from_op(op);
                let component = op
                    .get("component")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let fields = op.get("fields").cloned();

                match (entity_ref, component, fields) {
                    (Ok(entity), Some(comp), Some(f)) => set_component(world, entity, comp, f),
                    (Err(msg), _, _) => {
                        Err(ControlError::invalid_params(format!("op[{i}]: {msg}")))
                    }
                    (Ok(_), None, _) | (Ok(_), _, None) => Err(ControlError::invalid_params(
                        format!("op[{i}]: set_component requires entity, component, fields"),
                    )),
                }
            }
            "spawn" => {
                let components = op
                    .get("components")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                spawn_entity(world, components)
            }
            "despawn" => {
                let entity_ref = parse_entity_ref_from_op(op);
                match entity_ref {
                    Ok(entity) => despawn_entity(world, entity),
                    Err(msg) => Err(ControlError::invalid_params(format!("op[{i}]: {msg}"))),
                }
            }
            "remove_component" => {
                let entity_ref = parse_entity_ref_from_op(op);
                let component = op
                    .get("component")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                match (entity_ref, component) {
                    (Ok(entity), Some(comp)) => remove_component(world, entity, comp),
                    (Err(msg), _) => Err(ControlError::invalid_params(format!("op[{i}]: {msg}"))),
                    (Ok(_), None) => Err(ControlError::invalid_params(format!(
                        "op[{i}]: remove_component requires entity and component"
                    ))),
                }
            }
            _ => Err(ControlError::invalid_params(format!(
                "op[{i}]: unknown action '{action}'. Valid: set_component, spawn, despawn, remove_component"
            ))),
        });

        results.push(operation_entry(result));
    }

    Ok(batch_response(results))
}

fn operation_entry(result: Result<serde_json::Value, ControlError>) -> serde_json::Value {
    match result {
        Ok(val) => {
            let status = if has_embedded_errors(&val) {
                "partial"
            } else {
                "ok"
            };
            serde_json::json!({"status": status, "result": val})
        }
        Err(e) => serde_json::json!({"status": "error", "error": e.message}),
    }
}

fn validate_batch_operation(
    index: usize,
    operation: &serde_json::Value,
) -> Result<&str, ControlError> {
    let operation = operation.as_object().ok_or_else(|| {
        ControlError::invalid_params(format!("op[{index}]: operation must be an object"))
    })?;
    let action = operation
        .get("action")
        .ok_or_else(|| {
            ControlError::invalid_params(format!(
                "op[{index}]: missing required string field 'action'"
            ))
        })?
        .as_str()
        .ok_or_else(|| {
            ControlError::invalid_params(format!("op[{index}]: 'action' must be a string"))
        })?;

    let allowed = match action {
        "set_component" => &["action", "entity", "component", "fields"][..],
        "spawn" => &["action", "components"][..],
        "despawn" => &["action", "entity"][..],
        "remove_component" => &["action", "entity", "component"][..],
        _ => {
            return Err(ControlError::invalid_params(format!(
                "op[{index}]: unknown action '{action}'. Valid: set_component, spawn, despawn, remove_component"
            )));
        }
    };
    let unsupported = operation
        .keys()
        .filter(|key| !allowed.contains(&key.as_str()))
        .map(|key| format!("'{key}'"))
        .collect::<Vec<_>>();

    if unsupported.is_empty() {
        return Ok(action);
    }

    let field = if unsupported.len() == 1 {
        "field"
    } else {
        "fields"
    };
    Err(ControlError::invalid_params(format!(
        "op[{index}]: action '{action}' does not accept {field} {}; expected only: {}",
        unsupported.join(", "),
        allowed.join(", ")
    )))
}

/// Parse entity ref from a batch operation JSON object, with the same
/// spelling every other tool accepts: an all-digit string is a packed ID.
fn parse_entity_ref_from_op(op: &serde_json::Value) -> Result<EntityRef, String> {
    let Some(entity) = op.get("entity") else {
        return Err("Missing 'entity' (int or string)".into());
    };
    serde_json::from_value(entity.clone())
        .map_err(|_| "'entity' must be an integer ID or a string name".into())
}

pub(crate) fn batch_response(results: Vec<serde_json::Value>) -> serde_json::Value {
    let count = |status| {
        results
            .iter()
            .filter(|result| result.get("status").and_then(|value| value.as_str()) == Some(status))
            .count()
    };

    serde_json::json!({
        "succeeded": count("ok"),
        "partial": count("partial"),
        "failed": count("error"),
        "total": results.len(),
        "results": results,
    })
}

#[cfg(test)]
mod tests {
    use bevy::ecs::name::Name;

    use super::*;

    #[test]
    fn parse_entity_ref_from_op_entity_int() {
        let op = serde_json::json!({"entity": 42, "action": "set_component"});
        let result = parse_entity_ref_from_op(&op).unwrap();
        assert!(matches!(result, EntityRef::Id(42)));
    }

    #[test]
    fn parse_entity_ref_from_op_entity_string() {
        let op = serde_json::json!({"entity": "Player", "action": "set_component"});
        let result = parse_entity_ref_from_op(&op).unwrap();
        assert!(matches!(result, EntityRef::Name(ref s) if s == "Player"));
    }

    #[test]
    fn parse_entity_ref_from_op_numeric_string_is_an_id() {
        let op = serde_json::json!({"entity": "4294967244", "action": "despawn"});
        let result = parse_entity_ref_from_op(&op).unwrap();
        assert!(matches!(result, EntityRef::Id(4294967244)));
    }

    #[test]
    fn parse_entity_ref_from_op_missing_entity() {
        let op = serde_json::json!({"action": "set_component"});
        let result = parse_entity_ref_from_op(&op);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("entity"));
    }

    #[test]
    fn parse_entity_ref_from_op_entity_invalid_type() {
        let op = serde_json::json!({"entity": [1, 2, 3]});
        let result = parse_entity_ref_from_op(&op);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("entity"));
    }

    #[test]
    fn batch_mutate_empty_operations() {
        let mut world = World::new();
        let result = batch_mutate(&mut world, vec![]).unwrap();
        assert_eq!(result["total"], 0);
        assert_eq!(result["succeeded"], 0);
    }

    #[test]
    fn batch_mutate_unknown_action() {
        let mut world = World::new();
        let ops = vec![serde_json::json!({"action": "nonexistent"})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["total"], 1);
        assert_eq!(result["succeeded"], 0);
        let results = result["results"].as_array().unwrap();
        assert_eq!(results[0]["status"], "error");
    }

    #[test]
    fn batch_mutate_despawn_success() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();
        let ops = vec![serde_json::json!({"action": "despawn", "entity": entity.to_bits()})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["succeeded"], 1);
        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn batch_mutate_despawn_missing_entity() {
        let mut world = World::new();
        let ops = vec![serde_json::json!({"action": "despawn"})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["succeeded"], 0);
    }

    #[test]
    fn batch_mutate_set_component_missing_fields() {
        let mut world = World::new();
        let ops = vec![serde_json::json!({"action": "set_component"})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["succeeded"], 0);
    }

    #[test]
    fn batch_mutate_mixed_success_and_failure() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();
        let ops = vec![
            serde_json::json!({"action": "despawn", "entity": entity.to_bits()}),
            serde_json::json!({"action": "unknown_action"}),
        ];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["total"], 2);
        assert_eq!(result["succeeded"], 1);
    }

    #[test]
    fn batch_mutate_remove_component_missing_params() {
        let mut world = World::new();
        let ops = vec![serde_json::json!({"action": "remove_component"})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["succeeded"], 0);
        let results = result["results"].as_array().unwrap();
        assert_eq!(results[0]["status"], "error");
    }

    #[test]
    fn batch_mutate_remove_component_with_entity_no_component() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();
        let ops = vec![serde_json::json!({
            "action": "remove_component",
            "entity": entity.to_bits(),
            "component": "NonExistent"
        })];
        let result = batch_mutate(&mut world, ops).unwrap();
        // Should fail because component is not in registry
        assert_eq!(result["succeeded"], 0);
    }

    #[test]
    fn batch_mutate_spawn_empty() {
        let mut world = World::new();
        let ops = vec![serde_json::json!({"action": "spawn"})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["succeeded"], 1);
        let results = result["results"].as_array().unwrap();
        let entity_id = results[0]["result"]["entity_id"].as_u64();
        assert!(entity_id.is_some());
    }

    #[test]
    fn batch_mutate_spawn_with_unknown_components() {
        let mut world = World::new();
        let ops = vec![serde_json::json!({"action": "spawn", "components": {"UnknownComp": {}}})];
        let result = batch_mutate(&mut world, ops).unwrap();
        // Unknown components now fail fast - batch reports error status
        assert_eq!(result["succeeded"], 0);
        let results = result["results"].as_array().unwrap();
        assert_eq!(results[0]["status"], "error");
    }

    #[test]
    fn batch_mutate_multiple_despawns() {
        let mut world = World::new();
        let e1 = world.spawn(Name::new("A")).id();
        let e2 = world.spawn(Name::new("B")).id();
        let ops = vec![
            serde_json::json!({"action": "despawn", "entity": e1.to_bits()}),
            serde_json::json!({"action": "despawn", "entity": e2.to_bits()}),
        ];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["total"], 2);
        assert_eq!(result["succeeded"], 2);
        assert!(world.get_entity(e1).is_err());
        assert!(world.get_entity(e2).is_err());
    }

    #[test]
    fn batch_mutate_despawn_by_name() {
        let mut world = World::new();
        world.spawn(Name::new("Target"));
        let ops = vec![serde_json::json!({"action": "despawn", "entity": "Target"})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["succeeded"], 1);
    }

    #[test]
    fn batch_mutate_no_action_field() {
        let mut world = World::new();
        let ops = vec![serde_json::json!({"entity": 42})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["succeeded"], 0);
        let results = result["results"].as_array().unwrap();
        assert_eq!(results[0]["status"], "error");
        assert_eq!(
            results[0]["error"],
            "op[0]: missing required string field 'action'"
        );
    }

    #[test]
    fn batch_mutate_rejects_unsupported_fields_before_mutating() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("kept")).id();
        let entity_count = world.iter_entities().count();
        let ops = vec![
            serde_json::json!({"action": "spawn", "args": {"components": {}}}),
            serde_json::json!({"action": "spawn", "name": "hidden"}),
            serde_json::json!({
                "action": "set_component",
                "entity": entity.to_bits(),
                "component": "Name",
                "fields": {"name": "changed"},
                "args": {}
            }),
            serde_json::json!({
                "action": "despawn",
                "entity": entity.to_bits(),
                "extra": true
            }),
            serde_json::json!({
                "action": "remove_component",
                "entity": entity.to_bits(),
                "component": "Name",
                "extra": true
            }),
            serde_json::json!([]),
        ];

        let result = batch_mutate(&mut world, ops).unwrap();

        assert_eq!(result["total"], 6);
        assert_eq!(result["succeeded"], 0);
        assert_eq!(result["failed"], 6);
        assert_eq!(result["partial"], 0);
        let results = result["results"].as_array().unwrap();
        assert_eq!(
            results[0]["error"],
            "op[0]: action 'spawn' does not accept field 'args'; expected only: action, components"
        );
        assert_eq!(
            results[1]["error"],
            "op[1]: action 'spawn' does not accept field 'name'; expected only: action, components"
        );
        assert_eq!(
            results[2]["error"],
            "op[2]: action 'set_component' does not accept field 'args'; expected only: action, entity, component, fields"
        );
        assert_eq!(
            results[3]["error"],
            "op[3]: action 'despawn' does not accept field 'extra'; expected only: action, entity"
        );
        assert_eq!(
            results[4]["error"],
            "op[4]: action 'remove_component' does not accept field 'extra'; expected only: action, entity, component"
        );
        assert_eq!(results[5]["error"], "op[5]: operation must be an object");
        assert_eq!(world.iter_entities().count(), entity_count);
        assert_eq!(world.get::<Name>(entity).unwrap().as_str(), "kept");
    }

    #[test]
    fn operation_entry_marks_embedded_errors_partial() {
        let with_errors = serde_json::json!({"new_values": {}, "errors": ["some field error"]});
        let without_errors = serde_json::json!({"new_values": {"translation": [1.0, 2.0, 3.0]}});

        let partial = operation_entry(Ok(with_errors.clone()));
        assert_eq!(partial["status"], "partial");
        assert_eq!(partial["result"], with_errors);
        assert_eq!(operation_entry(Ok(without_errors))["status"], "ok");
        let error = operation_entry(Err(ControlError::invalid_params("bad")));
        assert_eq!(
            error,
            serde_json::json!({"status": "error", "error": "bad"})
        );
    }

    #[test]
    fn batch_response_counts_each_terminal_status() {
        let response = batch_response(vec![
            serde_json::json!({"status": "ok"}),
            serde_json::json!({"status": "partial"}),
            serde_json::json!({"status": "error"}),
            serde_json::json!({"status": "error"}),
        ]);

        assert_eq!(response["total"], 4);
        assert_eq!(response["succeeded"], 1);
        assert_eq!(response["partial"], 1);
        assert_eq!(response["failed"], 2);
    }
}
