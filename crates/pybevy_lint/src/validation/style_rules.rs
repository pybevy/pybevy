use crate::{
    model::{GetterResultClassification, PyClassDef, SelfMutability},
    output::{Diagnostic, DiagnosticCode, Suggestion},
};

/// Check style rules for a Rust class
/// `all_classes` is used to look up return types and check if they're enums/frozen
pub fn check_class_style(rust: &PyClassDef, all_classes: &[PyClassDef]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for prop in &rust.properties {
        if prop.has_getter && prop.getter_mutability == SelfMutability::RefMut {
            let mut diag = Diagnostic::warning(
                DiagnosticCode::W002,
                format!(
                    "getter '{}::{}' uses &mut self instead of &self",
                    rust.python_name, prop.name
                ),
            );

            if let Some(ref loc) = prop.getter_location {
                diag = diag.with_location(loc.clone());
            }

            diag = diag
                .with_note("Getters should be immutable (&self) for better performance and safety");
            diag = diag.with_suggestion(Suggestion::new("Change to &self"));

            diagnostics.push(diag);
        }
    }

    // W001/W004 are enforced directly at parse time rather than raised as diagnostics.
    // Storage-derived mutable results need an explicit source relationship; a getter-only
    // property can still silently mutate a copy via `obj.field.x = value`.
    if uses_storage_wrapper(rust) {
        for prop in &rust.properties {
            let classification = if prop.getter_uses_borrow {
                GetterResultClassification::Live
            } else {
                prop.getter_result_classification
            };
            let Some(return_type) = prop.property_type.as_deref() else {
                continue;
            };
            if !prop.has_getter
                || classification != GetterResultClassification::Unclassified
                || is_immutable_result_type(return_type, all_classes)
            {
                continue;
            }

            let mut diag = Diagnostic::warning(
                DiagnosticCode::W006,
                format!(
                    "getter '{}::{}' returns unclassified mutable result '{}'",
                    rust.python_name, prop.name, return_type
                ),
            );
            if let Some(ref loc) = prop.getter_location {
                diag = diag.with_location(loc.clone());
            }
            diag = diag.with_note(
                "Nested mutations may target a detached copy. Return live resolver storage, \
                 an enforced read-only snapshot, a computed_owned() value, or an owned \
                 collection whose elements are mechanically immutable.",
            );
            diag = diag.with_suggestion(Suggestion::new(
                "Classify the getter result and pin its mutation semantics with a public test",
            ));
            diagnostics.push(diag);
        }

        // Asset inspection methods have the same problem as properties when
        // they derive mutable results from `self`.
        if uses_pyasset(rust) {
            for method in &rust.methods {
                let Some(return_type) = method.return_type.as_deref() else {
                    continue;
                };
                if !method.result_derived_from_self
                    || method.result_classification != GetterResultClassification::Unclassified
                    || is_immutable_result_type(return_type, all_classes)
                {
                    continue;
                }
                let mut diag = Diagnostic::warning(
                    DiagnosticCode::W006,
                    format!(
                        "asset inspection method '{}::{}' returns unclassified mutable result '{}'",
                        rust.python_name, method.name, return_type
                    ),
                );
                if let Some(ref loc) = method.location {
                    diag = diag.with_location(loc.clone());
                }
                diag = diag.with_note(
                    "Inspection results derived from an asset need the same live, read-only, \
                     computed-owned, or immutable-collection contract as properties.",
                );
                diagnostics.push(diag);
            }
        }
    }

    diagnostics
}

/// Check if a class uses a storage macro wrapper (Component/Resource/Field/Value/Asset storage)
fn uses_storage_wrapper(class: &PyClassDef) -> bool {
    matches!(
        class.storage_macro.as_deref(),
        Some("pycomponent" | "pyresource" | "pyfield" | "pyvalue" | "pyasset")
    )
}

fn uses_pyasset(class: &PyClassDef) -> bool {
    class.storage_macro.as_deref() == Some("pyasset")
}

/// Check if a type is a struct that should potentially use borrow_field pattern
/// Returns true if the type looks like a complex struct type that may have mutable fields
/// This is conservative - warns on any non-primitive type that isn't borrowed
fn is_immutable_result_type(ty: &str, all_classes: &[PyClassDef]) -> bool {
    let compact: String = ty
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    is_immutable_compact(&compact, all_classes)
}

fn is_immutable_compact(ty: &str, all_classes: &[PyClassDef]) -> bool {
    if let Some(inner) = strip_outer(ty, "PyResult") {
        return is_immutable_compact(inner, all_classes);
    }
    if let Some(inner) = strip_outer(ty, "Option") {
        return is_immutable_compact(inner, all_classes);
    }
    if let Some(inner) = strip_outer(ty, "Py") {
        return is_immutable_compact(inner, all_classes);
    }
    if let Some(inner) = strip_outer(ty, "Bound") {
        let parts = split_top_level(inner);
        return parts
            .last()
            .is_some_and(|part| is_immutable_compact(part, all_classes));
    }
    let ty = ty.trim_start_matches('&').trim_start_matches("mut");

    let primitives = [
        "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64",
        "u128", "usize", "bool", "String", "str", "char", "()", "Duration", "None",
    ];
    if primitives.contains(&ty) {
        return true;
    }
    if ty.starts_with("Handle<")
        || ty == "PyHandle"
        || ty.starts_with("AssetId<")
        || ty == "UntypedAssetId"
        || ty == "Entity"
        || ty == "PyEntity"
    {
        return true;
    }
    if let Some(inner) = strip_outer(ty, "Vec") {
        return is_immutable_compact(inner, all_classes);
    }
    if let Some(inner) = strip_outer(ty, "HashMap").or_else(|| strip_outer(ty, "BTreeMap")) {
        let parts = split_top_level(inner);
        return parts.len() == 2
            && parts
                .iter()
                .all(|part| is_immutable_compact(part, all_classes));
    }
    if ty.starts_with('(') && ty.ends_with(')') {
        return split_top_level(&ty[1..ty.len() - 1])
            .iter()
            .all(|part| is_immutable_compact(part, all_classes));
    }
    if ty.starts_with('[') && ty.ends_with(']') {
        let element = ty[1..ty.len() - 1].split(';').next().unwrap_or_default();
        return is_immutable_compact(element, all_classes);
    }
    if ty == "Self" {
        return true;
    }
    if matches!(ty, "PyBytes" | "PyString" | "PyFrozenSet" | "PyNone") {
        return true;
    }
    let lookup = ty.rsplit("::").next().unwrap_or(ty);
    if let Some(class_def) = all_classes.iter().find(|class| class.rust_name == lookup) {
        return class_def.frozen
            && !class_def
                .properties
                .iter()
                .any(|property| property.has_setter)
            && !matches!(
                class_def.storage_macro.as_deref(),
                Some("pycomponent" | "pyresource" | "pyfield" | "pyvalue" | "pyasset")
            );
    }
    false
}

fn strip_outer<'a>(ty: &'a str, wrapper: &str) -> Option<&'a str> {
    ty.strip_prefix(wrapper)
        .and_then(|rest| rest.strip_prefix('<'))
        .and_then(|rest| rest.strip_suffix('>'))
}

fn split_top_level(input: &str) -> Vec<&str> {
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut result = Vec::new();
    for (index, character) in input.char_indices() {
        match character {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                result.push(&input[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    if start < input.len() {
        result.push(&input[start..]);
    }
    result
}
