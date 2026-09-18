use crate::{
    bevy_parser::types::{BevyItem, BevyItemKind},
    model::{EnumVariantKind, MacroInfo, PyClassDef},
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

/// Reject Python enum-family bases on stubs for native `#[pyenum]` wrappers.
pub fn validate_pyenum_stub_base(rust: &PyClassDef, python: &PyClassDef) -> Vec<Diagnostic> {
    if !matches!(rust.macro_info, Some(MacroInfo::BevyEnum { .. })) {
        return Vec::new();
    }

    let Some(base) = python.extends.as_deref() else {
        return Vec::new();
    };
    let base_name = base
        .rsplit(['.', ':'])
        .find(|part| !part.is_empty())
        .unwrap_or(base);
    if !matches!(base_name, "Enum" | "IntEnum" | "Flag" | "IntFlag") {
        return Vec::new();
    }

    let mut diagnostic = Diagnostic::error(
        DiagnosticCode::E017,
        format!(
            "pyenum '{}' is a native value class, but its stub inherits Python '{}'",
            rust.python_name, base
        ),
    )
    .with_note("Declare the stub as a plain class with typed variant attributes")
    .with_suggestion(Suggestion::new(format!(
        "replace `class {}({base}):` with `class {}:`",
        rust.python_name, rust.python_name
    )));
    if let Some(location) = &python.location {
        diagnostic = diagnostic.with_location(location.clone());
    }

    vec![diagnostic]
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

/// Flags value enums declared as a plain `#[pyclass]` instead of `#[pyenum]`.
///
/// Value enums are extracted by value from Python (`from_py_object`). The
/// Decision Guide maps them to `#[pyenum(T)]`, which generates conversions,
/// `__copy__`, and repr, and registers variant-topology metadata. Plain
/// `#[pyclass]` enums that skip extraction (messages, manual adapters) are
/// out of scope here.
pub fn validate_value_enum_without_pyenum(rust: &PyClassDef) -> Vec<Diagnostic> {
    if !rust.is_enum || !rust.from_py_object || rust.macro_info.is_some() {
        return Vec::new();
    }

    let mut diag = Diagnostic::warning(
        DiagnosticCode::W013,
        format!(
            "value enum '{}' uses plain #[pyclass] instead of #[pyenum]",
            rust.python_name
        ),
    );

    if let Some(ref loc) = rust.location {
        diag = diag.with_location(loc.clone());
    }

    diag = diag
        .with_note("Ordinary value enums map to #[pyenum(T)] in the decision guide")
        .with_note("pyenum generates conversions, __copy__, and repr, and records variant topology for Bevy audits")
        .with_suggestion(Suggestion::new(
            "declare the Bevy enum relationship with #[pyenum(T, ...)]",
        ));

    vec![diag]
}

/// Flags payload-free immutable value enums that omit Python hashing even
/// though both the wrapper and upstream contracts define compatible values.
pub fn validate_enum_hashability(rust: &PyClassDef, bevy: &BevyItem) -> Vec<Diagnostic> {
    let ordinary_pyenum = matches!(
        &rust.macro_info,
        Some(MacroInfo::BevyEnum {
            manual: false,
            message: false,
            component: false,
            resource: false,
            ..
        })
    );
    let upstream_hashes = bevy
        .trait_impls
        .iter()
        .any(|trait_name| trait_name == "Hash" || trait_name.ends_with("::Hash"));
    let payload_free = !rust.enum_variants.is_empty()
        && rust.enum_variants.iter().all(|variant| {
            matches!(
                variant.kind,
                EnumVariantKind::Unit | EnumVariantKind::EmptyTuple
            )
        });
    let wrapper_hashes = rust.hash || rust.methods.iter().any(|method| method.name == "__hash__");

    if !ordinary_pyenum
        || !rust.is_enum
        || bevy.kind != BevyItemKind::Enum
        || !rust.frozen
        || !rust.eq
        || rust.eq_int
        || !payload_free
        || !upstream_hashes
        || wrapper_hashes
    {
        return Vec::new();
    }

    let mut diagnostic = Diagnostic::warning(
        DiagnosticCode::W015,
        format!(
            "payload-free immutable value enum '{}' has value equality and upstream Hash but omits Python hashing",
            rust.python_name
        ),
    )
    .with_note(format!("upstream type '{}' implements Hash", bevy.full_path))
    .with_note("eq_int enums are excluded because equality with an integer requires an integer-compatible hash")
    .with_suggestion(
        Suggestion::new("derive Hash on the wrapper and enable PyO3 hashing")
            .with_replacement("add `Hash` to `#[derive(...)]` and `hash` to `#[pyclass(...)]`"),
    );
    if let Some(location) = &rust.location {
        diagnostic = diagnostic.with_location(location.clone());
    }

    vec![diagnostic]
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

/// Flags a `#[pyclass]` that names no `module = "pybevy...."`.
///
/// PyO3 then leaves `__module__` as `builtins`, so the class names no import
/// path in reprs, tracebacks, and attribute errors ("type object
/// 'builtins.EulerRot' has no attribute 'X'").
/// `tests/surface/test_class_module_names.py` pins this for classes declared
/// in a public stub; this rule covers every declaration, including the private
/// `_`-prefixed helpers that reach users only through repr and error text.
pub fn validate_pyclass_module_attribute(rust: &PyClassDef) -> Vec<Diagnostic> {
    let declared = rust.python_module_path.as_deref();
    if declared.is_some_and(|module| module == "pybevy" || module.starts_with("pybevy.")) {
        return Vec::new();
    }

    let message = match declared {
        Some(module) => format!(
            "pyclass '{}' declares module '{}', which is not a pybevy module",
            rust.python_name, module
        ),
        None => format!(
            "pyclass '{}' declares no module, so its __module__ is 'builtins'",
            rust.python_name
        ),
    };

    let suggested = rust
        .module_path
        .as_deref()
        .and_then(|path| path.split('.').next())
        .map(|module| format!("pybevy.{module}"))
        .unwrap_or_else(|| "pybevy.<module>".to_string());

    let mut diagnostic = Diagnostic::warning(DiagnosticCode::W016, message)
        .with_note("'builtins' names no import path in reprs, tracebacks, or attribute errors")
        .with_note(
            "the canonical Bevy public item path fixes the module; see 'Public module placement'",
        )
        .with_suggestion(Suggestion::new(format!(
            "add module = \"{suggested}\" to the #[pyclass(...)] attribute"
        )));
    if let Some(location) = &rust.location {
        diagnostic = diagnostic.with_location(location.clone());
    }

    vec![diagnostic]
}
