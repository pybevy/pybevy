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
                    && !crate::python_parser::types::types_compatible(
                        rust_prop
                            .declared_property_type
                            .as_deref()
                            .unwrap_or(rust_ty),
                        py_ty,
                    )
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

/// Flags stub attributes that promise Python writability where the Rust
/// runtime is read-only: a frozen class rejects attribute assignment, and a
/// non-frozen class without a matching setter raises AttributeError.
/// Writable annotations in .pyi files parse as getter+setter properties, so
/// this is the stub side of the writability contract.
pub fn validate_stub_writability(rust: &PyClassDef, python: &PyClassDef) -> Vec<Diagnostic> {
    if rust.extends.is_some() {
        // Inherited setters from the base class are not visible to this check.
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    for stub_prop in python
        .properties
        .iter()
        .filter(|p| p.has_setter && p.has_getter)
    {
        if stub_prop.name.starts_with('_') {
            continue;
        }
        let rust_settable = rust
            .properties
            .iter()
            .any(|p| p.name == stub_prop.name && p.has_setter);
        if rust_settable {
            continue;
        }

        let mut diagnostic = if rust.frozen {
            Diagnostic::warning(
                DiagnosticCode::W022,
                format!(
                    "stub declares '{}' writable but the frozen class '{}' rejects attribute assignment",
                    stub_prop.name, rust.python_name
                ),
            )
            .with_note("frozen pyclass attributes are read-only at runtime; the stub must use @property")
        } else {
            Diagnostic::warning(
                DiagnosticCode::W022,
                format!(
                    "stub declares '{}' writable but class '{}' has no matching Rust setter",
                    stub_prop.name, rust.python_name
                ),
            )
            .with_note("attribute assignment raises AttributeError at runtime; declare a read-only @property or add the setter")
        };
        diagnostic = diagnostic
            .with_suggestion(Suggestion::new(
                "declare the attribute as a read-only @property in the stub, or generate the Rust setter",
            ));
        if let Some(location) = &rust.location {
            diagnostic = diagnostic.with_location(location.clone());
        }
        diagnostics.push(diagnostic);
    }

    diagnostics
}

/// Flags writable payload attributes on generated nested variant classes where
/// the storage-backed enum field is not `#[py_borrow]`. Such child writes are
/// detached snapshots; only borrowed live fields may be writable.
pub fn validate_nested_variant_payload_writability(
    rust: &PyClassDef,
    python_classes: &[PyClassDef],
) -> Vec<Diagnostic> {
    if !rust.is_enum || rust.storage_macro.is_none() {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    for variant in &rust.enum_variants {
        let stub_variants: Vec<&PyClassDef> = python_classes
            .iter()
            .filter(|class| {
                class
                    .extends
                    .as_deref()
                    .is_some_and(|base| base.eq_ignore_ascii_case(&rust.python_name))
                    && class.python_name.eq_ignore_ascii_case(&variant.name)
            })
            .collect();
        for stub_class in stub_variants {
            for stub_prop in stub_class
                .properties
                .iter()
                .filter(|p| p.has_setter && p.has_getter && !p.name.starts_with('_'))
            {
                let Some(payload) = variant
                    .payload_fields
                    .iter()
                    .find(|f| f.name == stub_prop.name)
                else {
                    continue;
                };
                if payload.borrowed {
                    continue;
                }

                let mut diagnostic = Diagnostic::warning(
                    DiagnosticCode::W022,
                    format!(
                        "stub variant '{}' declares '{}' writable but the '{}' field is a non-borrowed storage snapshot",
                        stub_class.python_name, stub_prop.name, rust.python_name
                    ),
                )
                .with_note("child writes to a non-borrowed storage field are detached; declare the stub attribute read-only or borrow the live field")
                .with_suggestion(Suggestion::new(
                    "declare the payload attribute as a read-only @property, or mark the field #[py_borrow] for live writes",
                ));
                if let Some(location) = &rust.location {
                    diagnostic = diagnostic.with_location(location.clone());
                }
                diagnostics.push(diagnostic);
            }
        }
    }

    diagnostics
}
