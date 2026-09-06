use crate::{
    model::{BridgeStorageKind, PyClassDef},
    output::{Diagnostic, DiagnosticCode},
};

/// Validate that a component's `from_numpy()` stub parameters match the
/// bridge's `view_fields` + `batch_only_fields` (from `#[pycomponent(..., bridge)]`).
pub fn validate_bridge_from_numpy(
    rust_class: &PyClassDef,
    py_class: &PyClassDef,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let bridge_info = match &rust_class.bridge_info {
        Some(info) => info,
        None => return diagnostics,
    };

    if bridge_info.storage_kind != BridgeStorageKind::Component {
        return diagnostics;
    }

    // Collect expected from_numpy fields: view_fields ∪ batch_only_fields
    let mut expected_fields: Vec<&str> = bridge_info
        .view_fields
        .iter()
        .chain(bridge_info.batch_only_fields.iter())
        .map(|s| s.as_str())
        .collect();
    expected_fields.sort();

    if expected_fields.is_empty() {
        return diagnostics;
    }

    // Only validate stub if the Rust class actually exposes from_numpy as a static method.
    // Components may have view_fields (for View API) without exposing from_numpy to Python.
    let rust_has_from_numpy = rust_class
        .static_methods
        .iter()
        .any(|m| m.name == "from_numpy");
    if !rust_has_from_numpy {
        return diagnostics;
    }

    // Find from_numpy in the Python stub's static methods
    let from_numpy = py_class
        .static_methods
        .iter()
        .find(|m| m.name == "from_numpy");

    let from_numpy = match from_numpy {
        Some(m) => m,
        None => {
            let mut diag = Diagnostic::error(
                DiagnosticCode::E009,
                format!(
                    "'{}' has bridge with {} batch fields but no from_numpy() in stub",
                    rust_class.python_name,
                    expected_fields.len()
                ),
            );
            if let Some(loc) = &bridge_info.location {
                diag = diag.with_location(loc.clone());
            }
            diag = diag.with_note(format!(
                "Expected from_numpy() with fields: {}",
                expected_fields.join(", ")
            ));
            diagnostics.push(diag);
            return diagnostics;
        }
    };

    // Collect actual stub parameter names (excluding **kwargs-style)
    let mut stub_fields: Vec<&str> = from_numpy
        .parameters
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    stub_fields.sort();

    // Check for missing fields (in bridge but not in stub)
    let missing: Vec<&str> = expected_fields
        .iter()
        .filter(|f| !stub_fields.contains(f))
        .copied()
        .collect();

    // Check for extra fields (in stub but not in bridge)
    let extra: Vec<&str> = stub_fields
        .iter()
        .filter(|f| !expected_fields.contains(f))
        .copied()
        .collect();

    if !missing.is_empty() || !extra.is_empty() {
        let mut parts = Vec::new();
        if !missing.is_empty() {
            parts.push(format!("missing from stub: {}", missing.join(", ")));
        }
        if !extra.is_empty() {
            parts.push(format!("extra in stub: {}", extra.join(", ")));
        }

        let mut diag = Diagnostic::error(
            DiagnosticCode::E009,
            format!(
                "'{}' from_numpy() stub fields don't match bridge view_fields: {}",
                rust_class.python_name,
                parts.join("; ")
            ),
        );
        if let Some(loc) = &bridge_info.location {
            diag = diag.with_location(loc.clone());
        }
        diag = diag.with_note(format!("bridge fields: [{}]", expected_fields.join(", ")));
        diag = diag.with_note(format!(
            "stub from_numpy fields: [{}]",
            stub_fields.join(", ")
        ));
        diagnostics.push(diag);
    }

    diagnostics
}

/// Eligible constructor parameter types for view_fields (zero-copy View API access)
const VIEW_ELIGIBLE_TYPES: &[&str] = &["f32", "f64", "bool", "i32", "u32", "i64", "u64"];

/// Eligible constructor parameter types for batch_only_fields (from_numpy only, not View)
const BATCH_ONLY_ELIGIBLE_TYPES: &[&str] = &["PyColor", "Color"];

/// Check whether a constructor parameter type is eligible for view_fields or batch_only_fields.
/// Returns `Some("view")`, `Some("batch_only")`, or `None`.
fn field_eligibility(param_type: Option<&str>) -> Option<&'static str> {
    let ty = param_type?;
    if VIEW_ELIGIBLE_TYPES.contains(&ty) {
        Some("view")
    } else if BATCH_ONLY_ELIGIBLE_TYPES.contains(&ty) {
        Some("batch_only")
    } else {
        None
    }
}

/// Detect component fields that could benefit from view_fields/batch_only_fields exposure
/// but are not currently listed in the bridge's view_fields/batch_only_fields.
pub fn detect_unexposed_eligible_fields(
    rust_class: &PyClassDef,
    _py_class: &PyClassDef,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let bridge_info = match &rust_class.bridge_info {
        Some(info) => info,
        None => return diagnostics,
    };

    if bridge_info.storage_kind != BridgeStorageKind::Component {
        return diagnostics;
    }

    // Skip no_insert components: no from_numpy benefit (view_fields still
    // possible but rarer; skip to reduce noise)
    if bridge_info.no_insert {
        return diagnostics;
    }

    // Get constructor parameters as the source of truth for component fields
    let constructor = match &rust_class.constructor {
        Some(c) => c,
        None => return diagnostics,
    };

    let all_exposed: Vec<&str> = bridge_info
        .view_fields
        .iter()
        .chain(bridge_info.batch_only_fields.iter())
        .chain(bridge_info.view_only_fields.iter())
        .map(|s| s.as_str())
        .collect();

    let mut unexposed_view: Vec<String> = Vec::new();
    let mut unexposed_batch: Vec<String> = Vec::new();

    for param in &constructor.parameters {
        let name = param.name.as_str();
        // Skip if already exposed
        if all_exposed.contains(&name) {
            continue;
        }

        // Skip constructor params that don't have a corresponding getter property.
        // These are likely convenience parameters (e.g., SpatialListener::new(gap))
        // rather than stored fields.
        let has_getter = rust_class
            .properties
            .iter()
            .any(|p| p.name == name && p.has_getter);
        if !has_getter {
            continue;
        }

        match field_eligibility(param.param_type.as_deref()) {
            Some("view") => unexposed_view.push(name.to_string()),
            Some("batch_only") => unexposed_batch.push(name.to_string()),
            _ => {}
        }
    }

    if !unexposed_view.is_empty() || !unexposed_batch.is_empty() {
        let mut parts = Vec::new();
        if !unexposed_view.is_empty() {
            parts.push(format!(
                "view_fields candidates: {}",
                unexposed_view.join(", ")
            ));
        }
        if !unexposed_batch.is_empty() {
            parts.push(format!(
                "batch_only_fields candidates: {}",
                unexposed_batch.join(", ")
            ));
        }

        let mut diag = Diagnostic::warning(
            DiagnosticCode::W009,
            format!(
                "'{}' has eligible fields not in bridge view_fields: {}",
                rust_class.python_name,
                parts.join("; ")
            ),
        );
        if let Some(loc) = &bridge_info.location {
            diag = diag.with_location(loc.clone());
        }
        diagnostics.push(diag);
    }

    diagnostics
}
