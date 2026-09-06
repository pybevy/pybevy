use crate::{
    model::{EnumVariantKind, PyClassDef},
    output::{Diagnostic, DiagnosticCode, Suggestion},
};

/// Check if classes match at a high level
pub fn validate_class_match(rust: &PyClassDef, python: &PyClassDef) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    if let (Some(rust_extends), Some(py_extends)) = (&rust.extends, &python.extends) {
        // Normalize: PyComponent -> Component, PyResource -> Resource
        let rust_base = normalize_base_class(rust_extends);
        let py_base = normalize_base_class(py_extends);

        if rust_base != py_base {
            let mut diag = Diagnostic::warning(
                DiagnosticCode::E008, // Reuses E008
                format!(
                    "class '{}' has different base class: Rust='{}' vs Python='{}'",
                    rust.python_name, rust_extends, py_extends
                ),
            );

            if let Some(ref loc) = rust.location {
                diag = diag.with_location(loc.clone());
            }

            diagnostics.push(diag);
        }
    }

    diagnostics
}

/// Create diagnostic for missing class in Python stub
pub fn missing_class_in_stub(rust: &PyClassDef) -> Diagnostic {
    let mut diag = Diagnostic::error(
        DiagnosticCode::E001,
        format!(
            "class '{}' (Rust: {}) not found in Python stubs",
            rust.python_name, rust.rust_name
        ),
    );

    if let Some(ref loc) = rust.location {
        diag = diag.with_location(loc.clone());
    }

    diag = diag.with_note("Add class definition to the appropriate .pyi file".to_string());

    let stub = generate_stub_suggestion(rust);
    diag = diag.with_suggestion(Suggestion::new("Add class stub").with_replacement(stub));

    diag
}

/// Normalize base class name for comparison
fn normalize_base_class(name: &str) -> String {
    let name = name.trim();

    // Qualified paths (`pybevy_core :: PyAsset`) name the class by their
    // final segment.
    let name = name.rsplit("::").next().unwrap_or(name).trim();

    // Strip Py prefix
    let name = if name.starts_with("Py") && name.len() > 2 {
        &name[2..]
    } else {
        name
    };

    name.to_string()
}

/// Generate a basic Python stub suggestion for a Rust class
fn generate_stub_suggestion(rust: &PyClassDef) -> String {
    let mut stub = String::new();

    if let Some(ref extends) = rust.extends {
        let base = normalize_base_class(extends);
        stub.push_str(&format!("class {}({}):\n", rust.python_name, base));
    } else {
        stub.push_str(&format!("class {}:\n", rust.python_name));
    }

    if let Some(ref ctor) = rust.constructor {
        stub.push_str("    def __init__(\n");
        stub.push_str("        self,\n");
        for param in &ctor.parameters {
            let ty = param.param_type.as_deref().unwrap_or("Any");
            let normalized_ty = crate::rust_parser::types::normalize_rust_type(ty);
            if let Some(ref default) = param.default_value {
                stub.push_str(&format!(
                    "        {}: {} = {},\n",
                    param.name, normalized_ty, default
                ));
            } else {
                stub.push_str(&format!("        {}: {},\n", param.name, normalized_ty));
            }
        }
        stub.push_str("    ) -> None: ...\n\n");
    }

    for prop in &rust.properties {
        let ty = prop.property_type.as_deref().unwrap_or("Any");
        let normalized_ty = crate::rust_parser::types::normalize_rust_type(ty);
        stub.push_str(&format!("    {}: {}\n", prop.name, normalized_ty));
    }

    if stub.ends_with(":\n") {
        stub.push_str("    ...\n");
    }

    stub
}

/// Detects when a PyO3 enum has both enum variants and classattr aliases for the same values.
/// See docs/tech/enum-design.md for the recommended pattern.
pub fn validate_enum_classattr_redundancy(rust: &PyClassDef) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    if !rust.is_enum || rust.class_attrs.is_empty() {
        return diagnostics;
    }

    for classattr in &rust.class_attrs {
        let classattr_lower = classattr.name.to_lowercase();

        // Find a matching enum variant (case-insensitive comparison)
        let matching_variant = rust.enum_variants.iter().find(|v| {
            // Only unit variants and empty tuple variants are candidates for redundancy
            // Tuple variants with data need constructors, so classattrs don't apply
            matches!(v.kind, EnumVariantKind::Unit | EnumVariantKind::EmptyTuple)
                && v.name.to_lowercase() == classattr_lower
        });

        if let Some(variant) = matching_variant {
            let mut diag = Diagnostic::warning(
                DiagnosticCode::W008,
                format!(
                    "enum '{}' has redundant classattr '{}' aliasing variant '{}'",
                    rust.python_name, classattr.name, variant.name
                ),
            );

            if let Some(ref loc) = classattr.location {
                diag = diag.with_location(loc.clone());
            }

            diag = diag
                .with_note("This creates two ways to access the same value (anti-pattern)")
                .with_note("See docs/tech/enum-design.md for recommended patterns")
                .with_suggestion(
                    Suggestion::new("Either remove the classattr OR convert to wrapper struct")
                        .with_replacement(format!(
                            "Option 1: Remove #[classattr] for {}\nOption 2: Convert enum to wrapper struct with classattr only",
                            classattr.name
                        )),
                );

            diagnostics.push(diag);
        }
    }

    diagnostics
}

/// A catch-all arm that yields a concrete value reports the wrong variant to
/// Python instead of the one bevy actually held.
pub fn validate_conversion_fallbacks(rust: &PyClassDef) -> Vec<Diagnostic> {
    rust.silent_fallbacks
        .iter()
        .map(|fallback| {
            let mut diag = Diagnostic::warning(
                DiagnosticCode::W011,
                format!(
                    "conversion from '{}' to '{}' maps unlisted variants to a concrete value",
                    fallback.source_type, rust.python_name
                ),
            );
            if let Some(ref loc) = fallback.location {
                diag = diag.with_location(loc.clone());
            }
            diag.with_note(
                "Python then reports a variant bevy never held, and a round trip writes it back",
            )
            .with_suggestion(Suggestion::new(
                "cover every variant, or return an error naming the unmapped one",
            ))
        })
        .collect()
}
