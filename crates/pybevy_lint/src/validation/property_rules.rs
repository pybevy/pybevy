use crate::{
    config::BevyConfig,
    model::PyClassDef,
    output::{Diagnostic, DiagnosticCode, Suggestion},
};

/// Validate properties match between Rust and Python
pub fn validate_properties(
    rust: &PyClassDef,
    python: &PyClassDef,
    config: Option<&BevyConfig>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for rust_prop in &rust.properties {
        if rust_prop.name.starts_with('_') {
            continue;
        }
        let py_prop = python.properties.iter().find(|p| p.name == rust_prop.name);

        match py_prop {
            Some(py_prop) => {
                // Check type compatibility
                if let (Some(rust_ty), Some(py_ty)) =
                    (&rust_prop.property_type, &py_prop.property_type)
                    && !crate::python_parser::types::types_compatible(rust_ty, py_ty)
                {
                    let mut diag = Diagnostic::warning(
                        DiagnosticCode::E006,
                        format!(
                            "property '{}::{}' type mismatch: Rust='{}' vs Python='{}'",
                            rust.python_name, rust_prop.name, rust_ty, py_ty
                        ),
                    );

                    if let Some(ref loc) = rust_prop.getter_location {
                        diag = diag.with_location(loc.clone());
                    }

                    diagnostics.push(diag);
                }

                // Check read/write access
                if rust_prop.has_getter && !py_prop.has_getter {
                    let mut diag = Diagnostic::warning(
                        DiagnosticCode::E007,
                        format!(
                            "property '{}::{}' has getter in Rust but not in Python stub",
                            rust.python_name, rust_prop.name
                        ),
                    );

                    if let Some(ref loc) = rust_prop.getter_location {
                        diag = diag.with_location(loc.clone());
                    }

                    diagnostics.push(diag);
                }

                if rust_prop.has_setter && !py_prop.has_setter {
                    let mut diag = Diagnostic::warning(
                        DiagnosticCode::E007,
                        format!(
                            "property '{}::{}' has setter in Rust but not marked writable in Python stub",
                            rust.python_name, rust_prop.name
                        ),
                    );

                    if let Some(ref loc) = rust_prop.setter_location {
                        diag = diag.with_location(loc.clone());
                    }

                    diagnostics.push(diag);
                }

                // Check for getter without setter (W007)
                // Skip if config marks this property as intentionally read-only
                let is_intentionally_readonly = config
                    .map(|c| c.is_intentional_readonly_property(&rust.python_name, &rust_prop.name))
                    .unwrap_or(false);

                if rust_prop.has_getter
                    && !rust_prop.has_setter
                    && !rust.frozen
                    && !is_intentionally_readonly
                {
                    let mut diag = Diagnostic::warning(
                        DiagnosticCode::W007,
                        format!(
                            "property '{}::{}' has getter but no setter (read-only)",
                            rust.python_name, rust_prop.name
                        ),
                    );

                    if let Some(ref loc) = rust_prop.getter_location {
                        diag = diag.with_location(loc.clone());
                    }

                    diag = diag.with_note(
                        "Consider adding a setter, or ignore if intentionally read-only (computed value, engine-managed)",
                    );

                    diagnostics.push(diag);
                }
            }
            None => {
                let mut diag = Diagnostic::error(
                    DiagnosticCode::E007,
                    format!(
                        "property '{}::{}' not found in Python stub",
                        rust.python_name, rust_prop.name
                    ),
                );

                if let Some(ref loc) = rust_prop.getter_location {
                    diag = diag.with_location(loc.clone());
                }

                let stub = generate_property_stub(rust_prop);
                diag = diag.with_suggestion(
                    Suggestion::new("Add property to stub").with_replacement(stub),
                );

                diagnostics.push(diag);
            }
        }
    }

    for rust_attr in &rust.class_attrs {
        if rust_attr.name.starts_with('_') {
            continue;
        }
        let py_attr = python.class_attrs.iter().find(|a| a.name == rust_attr.name);
        let py_prop = python.properties.iter().find(|p| p.name == rust_attr.name);

        if py_attr.is_none() && py_prop.is_none() {
            let mut diag = Diagnostic::error(
                DiagnosticCode::E007,
                format!(
                    "class attribute '{}::{}' not found in Python stub",
                    rust.python_name, rust_attr.name
                ),
            );

            if let Some(ref loc) = rust_attr.location {
                diag = diag.with_location(loc.clone());
            }

            diagnostics.push(diag);
        }
    }

    for py_prop in &python.properties {
        // Check if property exists as a Rust property OR as a class attribute OR as an enum variant
        let in_rust_props = rust.properties.iter().any(|p| p.name == py_prop.name);
        let in_rust_classattrs = rust.class_attrs.iter().any(|a| a.name == py_prop.name);
        let in_rust_enum_variants =
            rust.is_enum && rust.enum_variants.iter().any(|v| v.name == py_prop.name);

        if !in_rust_props && !in_rust_classattrs && !in_rust_enum_variants {
            let mut diag = Diagnostic::warning(
                DiagnosticCode::E003,
                format!(
                    "property '{}::{}' in Python stub not found in Rust",
                    rust.python_name, py_prop.name
                ),
            );

            // Use property location if available, otherwise use class location
            if let Some(ref loc) = py_prop.getter_location {
                diag = diag.with_location(loc.clone());
            } else if let Some(ref loc) = python.location {
                diag = diag.with_location(loc.clone());
            }

            diag = diag.with_note("Property may be inherited or generated by a macro");

            diagnostics.push(diag);
        }
    }

    diagnostics
}

/// Generate a property stub
fn generate_property_stub(prop: &crate::model::PropertyDef) -> String {
    let ty = prop
        .property_type
        .as_ref()
        .map(|t| crate::rust_parser::types::normalize_rust_type(t))
        .unwrap_or_else(|| "Any".to_string());

    format!("{}: {}", prop.name, ty)
}
