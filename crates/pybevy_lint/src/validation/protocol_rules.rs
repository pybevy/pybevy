use crate::{
    model::PyClassDef,
    output::{Diagnostic, DiagnosticCode},
};

fn is_checked_protocol(name: &str) -> bool {
    matches!(
        name,
        "__eq__"
            | "__ne__"
            | "__lt__"
            | "__le__"
            | "__gt__"
            | "__ge__"
            | "__add__"
            | "__radd__"
            | "__sub__"
            | "__rsub__"
            | "__mul__"
            | "__rmul__"
            | "__truediv__"
            | "__rtruediv__"
            | "__floordiv__"
            | "__rfloordiv__"
            | "__mod__"
            | "__rmod__"
            | "__pow__"
            | "__rpow__"
            | "__matmul__"
            | "__rmatmul__"
            | "__neg__"
            | "__pos__"
            | "__abs__"
            | "__len__"
            | "__getitem__"
            | "__setitem__"
            | "__contains__"
            | "__iter__"
            | "__next__"
    )
}

fn has_protocol(class: &PyClassDef, name: &str, classes: &[PyClassDef], native: bool) -> bool {
    let mut current = class;
    for _ in 0..=classes.len() {
        if current.methods.iter().any(|method| method.name == name) {
            return true;
        }
        if native
            && matches!(
                name,
                "__eq__" | "__ne__" | "__lt__" | "__le__" | "__gt__" | "__ge__"
            )
            && ((current.eq && matches!(name, "__eq__" | "__ne__"))
                || current
                    .methods
                    .iter()
                    .any(|method| method.name == "__richcmp__"))
        {
            return true;
        }
        if native {
            let delegate = match current.python_name.as_str() {
                "Query" => Some("QueryIter"),
                "Single" => Some("SingleQuery"),
                _ => None,
            };
            if let Some(delegate) =
                delegate.and_then(|name| classes.iter().find(|class| class.python_name == name))
            {
                current = delegate;
                continue;
            }
        }
        let Some(base) = current.extends.as_deref() else {
            break;
        };
        let base = base.split('[').next().unwrap_or(base);
        let Some(parent) = classes
            .iter()
            .find(|candidate| candidate.python_name == base || candidate.rust_name == base)
        else {
            break;
        };
        current = parent;
    }
    false
}

/// Check public protocols while accounting for inherited and PyO3-generated slots.
pub fn validate_protocols(
    rust: &PyClassDef,
    python: &PyClassDef,
    rust_classes: &[PyClassDef],
    python_classes: &[PyClassDef],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for method in &rust.methods {
        if is_checked_protocol(&method.name)
            && !has_protocol(python, &method.name, python_classes, false)
        {
            let mut diagnostic = Diagnostic::error(
                DiagnosticCode::E002,
                format!(
                    "method '{}::{}' not found in Python stub",
                    rust.python_name, method.name
                ),
            );
            if let Some(location) = &method.location {
                diagnostic = diagnostic.with_location(location.clone());
            }
            diagnostics.push(diagnostic);
        }
    }
    for method in &python.methods {
        if is_checked_protocol(&method.name)
            && !has_protocol(rust, &method.name, rust_classes, true)
        {
            let mut diagnostic = Diagnostic::warning(
                DiagnosticCode::E003,
                format!(
                    "method '{}::{}' in Python stub not found in Rust",
                    rust.python_name, method.name
                ),
            );
            if let Some(location) = &method.location {
                diagnostic = diagnostic.with_location(location.clone());
            }
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

/// Report a class whose `copy.copy` works but whose `copy.deepcopy` falls back to pickling.
pub fn validate_copy_pairing(
    class: &PyClassDef,
    classes: &[PyClassDef],
    native: bool,
) -> Vec<Diagnostic> {
    let Some(copy) = class
        .methods
        .iter()
        .find(|method| method.name == "__copy__")
    else {
        return Vec::new();
    };
    if has_protocol(class, "__deepcopy__", classes, native) {
        return Vec::new();
    }
    let side = if native { "pyclass" } else { "stub class" };
    let mut diagnostic = Diagnostic::warning(
        DiagnosticCode::W043,
        format!(
            "{side} '{}' defines __copy__ but no __deepcopy__",
            class.python_name
        ),
    )
    .with_note("copy.deepcopy then falls back to pickling and raises TypeError; add __deepcopy__(memo) returning the same copy");
    if let Some(location) = &copy.location {
        diagnostic = diagnostic.with_location(location.clone());
    }
    vec![diagnostic]
}
