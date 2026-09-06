pub mod bridge_rules;
pub mod class_rules;
pub mod method_rules;
pub mod property_rules;
pub mod style_rules;

use crate::{
    config::Config,
    model::{
        EnumVariantDef, EnumVariantKind, MacroInfo, MethodDef, ParameterDef, PropertyDef,
        PyClassDef,
    },
    output::{Diagnostic, DiagnosticCode},
};

/// Validate all Rust classes against Python stubs
pub fn validate_all(
    rust_classes: &[PyClassDef],
    python_classes: &[PyClassDef],
    config: Option<&Config>,
) -> Vec<Diagnostic> {
    validate_all_impl(rust_classes, python_classes, config, true)
}

/// Validate a filtered subset without treating out-of-scope exceptions as stale.
pub fn validate_scoped(
    rust_classes: &[PyClassDef],
    python_classes: &[PyClassDef],
    config: Option<&Config>,
) -> Vec<Diagnostic> {
    validate_all_impl(rust_classes, python_classes, config, false)
}

fn validate_all_impl(
    rust_classes: &[PyClassDef],
    python_classes: &[PyClassDef],
    config: Option<&Config>,
    check_stale_exceptions: bool,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut consumed_exceptions = config
        .map(|config| vec![false; config.validation.exceptions.len()])
        .unwrap_or_default();

    for rust_class in rust_classes {
        if rust_class.python_name.starts_with('_') {
            continue;
        }
        let mut class_diagnostics = Vec::new();
        // Find corresponding Python class - prefer module path match
        let python_class = find_matching_python_class(rust_class, python_classes);
        let manual_variant = find_manual_enum_variant(rust_class, rust_classes, python_classes);

        // Rust-only validations (don't require Python stub)
        class_diagnostics.extend(class_rules::validate_enum_classattr_redundancy(rust_class));
        class_diagnostics.extend(method_rules::validate_builder_receivers(rust_class));
        class_diagnostics.extend(class_rules::validate_conversion_fallbacks(rust_class));

        match python_class.or(manual_variant.as_ref()) {
            Some(py_class) => {
                // Validate class match
                class_diagnostics.extend(class_rules::validate_class_match(rust_class, py_class));

                // Validate constructor
                if !is_manual_enum_base(rust_class, py_class) {
                    class_diagnostics
                        .extend(method_rules::validate_constructor(rust_class, py_class));
                }

                // Validate methods
                class_diagnostics.extend(method_rules::validate_methods_with_config(
                    rust_class,
                    py_class,
                    config.map(|config| &config.bevy),
                ));

                // Validate properties
                class_diagnostics.extend(property_rules::validate_properties(
                    rust_class,
                    py_class,
                    config.map(|config| &config.bevy),
                ));

                // Validate bridge from_numpy stub parity
                class_diagnostics.extend(bridge_rules::validate_bridge_from_numpy(
                    rust_class, py_class,
                ));

                // Detect eligible but unexposed fields
                class_diagnostics.extend(bridge_rules::detect_unexposed_eligible_fields(
                    rust_class, py_class,
                ));

                // Style checks
                class_diagnostics.extend(style_rules::check_class_style(rust_class, rust_classes));
            }
            None => {
                // Class not found in Python stubs
                class_diagnostics.push(class_rules::missing_class_in_stub(rust_class));
            }
        }

        let path = class_path(rust_class);
        for diagnostic in class_diagnostics {
            if is_disabled_code(config, diagnostic.code.as_str()) {
                continue;
            }
            if consume_exception(
                config,
                &mut consumed_exceptions,
                &path,
                diagnostic.code.as_str(),
            ) {
                continue;
            }
            diagnostics.push(diagnostic);
        }
    }

    for py_class in python_classes {
        if !rust_classes
            .iter()
            .any(|r| r.python_name == py_class.python_name)
        {
            // Python-only classes are not errors; skip
        }
    }

    if let Some(config) = config.filter(|_| check_stale_exceptions) {
        for (exception, consumed) in config.validation.exceptions.iter().zip(consumed_exceptions) {
            if !consumed {
                diagnostics.push(
                    Diagnostic::error(
                        DiagnosticCode::E010,
                        format!(
                            "validation exception {} for '{}' is stale or invalid",
                            exception.code, exception.path
                        ),
                    )
                    .with_note(exception.reason.clone()),
                );
            }
        }
    }

    diagnostics
}

fn class_path(class: &PyClassDef) -> String {
    match &class.module_path {
        Some(module) => format!("{module}.{}", class.python_name),
        None => class.python_name.clone(),
    }
}

fn is_disabled_code(config: Option<&Config>, code: &str) -> bool {
    config.is_some_and(|config| {
        config
            .validation
            .disabled
            .iter()
            .any(|disabled| disabled.code == code && !disabled.reason.trim().is_empty())
    })
}

fn consume_exception(
    config: Option<&Config>,
    consumed: &mut [bool],
    path: &str,
    code: &str,
) -> bool {
    let Some(config) = config else {
        return false;
    };
    let Some((index, _)) =
        config
            .validation
            .exceptions
            .iter()
            .enumerate()
            .find(|(index, exception)| {
                !consumed[*index]
                    && !exception.reason.trim().is_empty()
                    && exception.path == path
                    && exception.code == code
            })
    else {
        return false;
    };
    consumed[index] = true;
    true
}

/// Find the matching Python class for a Rust class
/// Prefers matching by both name AND module path when possible
fn find_matching_python_class<'a>(
    rust_class: &PyClassDef,
    python_classes: &'a [PyClassDef],
) -> Option<&'a PyClassDef> {
    if let Some(rust_module) = &rust_class.module_path {
        // Try exact module match first
        if let Some(py_class) = python_classes.iter().find(|p| {
            p.python_name == rust_class.python_name
                && p.module_path.as_ref().is_some_and(|pm| pm == rust_module)
        }) {
            return Some(py_class);
        }

        // Try matching just the first component of the module path
        // e.g., rust "camera.scaling_mode" should match python "camera"
        let rust_module_base = rust_module.split('.').next().unwrap_or(rust_module);
        if let Some(py_class) = python_classes.iter().find(|p| {
            p.python_name == rust_class.python_name
                && p.module_path
                    .as_ref()
                    .is_some_and(|pm| pm == rust_module_base || pm.starts_with(rust_module_base))
        }) {
            return Some(py_class);
        }
    }

    // Fallback: match by name only
    python_classes
        .iter()
        .find(|p| p.python_name == rust_class.python_name)
}

fn is_manual_enum_base(rust_class: &PyClassDef, python_class: &PyClassDef) -> bool {
    python_class.is_enum
        && matches!(
            rust_class.macro_info,
            Some(MacroInfo::BevyEnum { manual: true, .. })
        )
}

fn find_manual_enum_variant(
    rust_class: &PyClassDef,
    rust_classes: &[PyClassDef],
    python_classes: &[PyClassDef],
) -> Option<PyClassDef> {
    let rust_parent_name = rust_class.extends.as_deref()?;
    let rust_parent = rust_classes.iter().find(|candidate| {
        candidate.rust_name == rust_parent_name
            && matches!(
                candidate.macro_info,
                Some(MacroInfo::BevyEnum { manual: true, .. })
            )
    })?;
    let python_parent = find_matching_python_class(rust_parent, python_classes)?;
    let python_variant = python_parent
        .enum_variants
        .iter()
        .find(|variant| variant.name == rust_class.python_name)?;

    Some(enum_variant_as_class(python_parent, python_variant))
}

fn enum_variant_as_class(parent: &PyClassDef, variant: &EnumVariantDef) -> PyClassDef {
    let fields: Vec<(String, String)> = match &variant.kind {
        EnumVariantKind::Unit | EnumVariantKind::EmptyTuple => Vec::new(),
        EnumVariantKind::Tuple(types) => types
            .iter()
            .enumerate()
            .map(|(index, field_type)| {
                let name = if types.len() == 1 {
                    "value".to_string()
                } else {
                    format!("_{index}")
                };
                (name, field_type.clone())
            })
            .collect(),
        EnumVariantKind::Struct(fields) => fields.clone(),
    };

    let parameters = fields
        .iter()
        .map(|(name, field_type)| ParameterDef {
            name: name.clone(),
            param_type: Some(field_type.clone()),
            ..Default::default()
        })
        .collect();
    let properties = fields
        .into_iter()
        .map(|(name, property_type)| PropertyDef {
            name,
            property_type: Some(property_type),
            has_getter: true,
            ..Default::default()
        })
        .collect();

    let constructor = variant.constructor.clone().unwrap_or_else(|| MethodDef {
        name: "__init__".to_string(),
        parameters,
        ..Default::default()
    });

    PyClassDef {
        python_name: variant.name.clone(),
        extends: Some(parent.python_name.clone()),
        module_path: parent.module_path.clone(),
        constructor: Some(constructor),
        properties,
        ..Default::default()
    }
}
