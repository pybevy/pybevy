use super::{class_path, method_rules};
use crate::{
    config::Config,
    model::{MethodDef, PyClassDef},
    output::{Diagnostic, DiagnosticCode},
};

fn receiver(method: &MethodDef) -> Option<String> {
    let annotation: String = method
        .receiver_type
        .as_deref()?
        .trim()
        .trim_matches(['\'', '"'])
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let annotation = if method.is_class_method {
        annotation.strip_prefix("type[")?.strip_suffix(']')?
    } else {
        annotation.as_str()
    };
    annotation.contains('[').then(|| annotation.to_string())
}

fn mapping_owner(path: &str) -> &str {
    path.split('[').next().unwrap_or(path)
}

pub(super) fn audit_mappings(rust_classes: &[PyClassDef], config: &Config) -> Vec<Diagnostic> {
    config
        .validation
        .specialized_receivers
        .iter()
        .filter(|mapping| {
            !rust_classes
                .iter()
                .any(|class| class_path(class) == mapping_owner(&mapping.path))
        })
        .map(|mapping| {
            Diagnostic::error(
                DiagnosticCode::E010,
                format!(
                    "specialized receiver mapping '{}' has no native owner",
                    mapping.path
                ),
            )
        })
        .collect()
}

pub(super) fn validate_methods(
    rust: &PyClassDef,
    python: &PyClassDef,
    rust_classes: &[PyClassDef],
    config: Option<&Config>,
) -> Vec<Diagnostic> {
    let path = class_path(rust);
    let mappings: Vec<_> = config
        .into_iter()
        .flat_map(|config| &config.validation.specialized_receivers)
        .filter(|mapping| mapping_owner(&mapping.path) == path)
        .collect();
    if mappings.is_empty() {
        return method_rules::validate_methods_with_config(rust, python, config.map(|c| &c.bevy));
    }
    let mut common = python.clone();
    common.methods.retain(|method| receiver(method).is_none());
    common
        .static_methods
        .retain(|method| receiver(method).is_none());
    let mut diagnostics =
        method_rules::validate_methods_with_config(rust, &common, config.map(|c| &c.bevy));
    let module = rust.module_path.as_deref().unwrap_or("");
    let qualified = |name: &str| {
        if module.is_empty() {
            name.to_string()
        } else {
            format!("{module}.{name}")
        }
    };
    for mapping in &mappings {
        if mapping.reason.trim().is_empty()
            || !python
                .methods
                .iter()
                .chain(&python.static_methods)
                .any(|method| receiver(method).is_some_and(|name| qualified(&name) == mapping.path))
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::E010,
                format!(
                    "specialized receiver mapping '{}' is stale or lacks a reason",
                    mapping.path
                ),
            ));
        }
    }
    for method in python.methods.iter().chain(&python.static_methods) {
        let Some(name) = receiver(method) else {
            continue;
        };
        let receiver_path = qualified(&name);
        let candidates: Vec<_> = mappings
            .iter()
            .filter(|mapping| mapping.path == receiver_path)
            .collect();
        let targets: Vec<_> = match candidates.as_slice() {
            [mapping] => rust_classes
                .iter()
                .filter(|class| {
                    class.rust_name == mapping.rust_type
                        && class.module_path == rust.module_path
                        && class
                            .python_module_path
                            .as_deref()
                            .is_none_or(|declared| declared == format!("pybevy.{module}"))
                })
                .collect(),
            _ => Vec::new(),
        };
        let [target] = targets.as_slice() else {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::E010,
                format!(
                    "cannot resolve specialized receiver '{}' for '{}'",
                    receiver_path, method.name
                ),
            ));
            continue;
        };
        let native = if method.is_static || method.is_class_method {
            target
                .static_methods
                .iter()
                .find(|native| native.name == method.name)
        } else {
            target
                .methods
                .iter()
                .find(|native| native.name == method.name)
        };
        let Some(native) = native else {
            let mut diagnostic = Diagnostic::warning(
                DiagnosticCode::E003,
                format!(
                    "method '{}::{}' has no matching native receiver on '{}'",
                    name, method.name, target.rust_name
                ),
            );
            if let Some(location) = &method.location {
                diagnostic = diagnostic.with_location(location.clone());
            }
            diagnostics.push(diagnostic);
            continue;
        };
        let mut native = native.clone();
        if let Some(return_type) = &mut native.return_type {
            *return_type = return_type
                .split_inclusive(|c: char| !c.is_alphanumeric() && c != '_')
                .map(|part| {
                    let token = part.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
                    if token == target.rust_name {
                        format!("Self{}", &part[token.len()..])
                    } else {
                        part.to_string()
                    }
                })
                .collect();
        }
        // Reuse all ordinary method checks with only this receiver's native method.
        let mut native_class = PyClassDef {
            python_name: name.clone(),
            ..Default::default()
        };
        let mut stub_class = PyClassDef {
            python_name: name,
            ..Default::default()
        };
        native_class.methods.push(native);
        stub_class.methods.push(method.clone());
        diagnostics.extend(method_rules::validate_methods_with_config(
            &native_class,
            &stub_class,
            config.map(|c| &c.bevy),
        ));
    }
    diagnostics
}
