use crate::{
    config::BevyConfig,
    model::{
        ConstructorOrigin, EnumVariantKind, MethodDef, ParameterKind, PyClassDef, SelfMutability,
    },
    output::{Diagnostic, DiagnosticCode, Suggestion},
    python_parser::types::{resolve_self, types_compatible},
    rust_parser::types::normalize_rust_type,
};

/// Check if two parameter names are equivalent
/// Treats `_foo` and `foo` as equivalent (Rust uses underscore to silence unused warnings)
fn param_names_match(rust_name: &str, py_name: &str) -> bool {
    if rust_name == py_name {
        return true;
    }

    // Strip leading underscore from Rust name and compare
    let rust_stripped = rust_name.strip_prefix('_').unwrap_or(rust_name);
    let py_stripped = py_name.strip_prefix('_').unwrap_or(py_name);

    rust_stripped == py_stripped || rust_stripped == py_name || rust_name == py_stripped
}

/// Check if a constructor uses *args, **kwargs pattern for subclassing
/// These have parameters like `_args: &Bound<PyTuple>` and `_kwargs: Option<&Bound<PyDict>>`
fn is_varargs_subclass_constructor(ctor: &crate::model::MethodDef) -> bool {
    // Check if signature string contains *args or **kwargs pattern
    if let Some(ref sig) = ctor.signature_str
        && (sig.contains("* _args") || sig.contains("*_args") || sig.contains("** _kwargs"))
    {
        return true;
    }

    // Also check parameter names - if they're just _args and _kwargs with tuple/dict types
    let has_args = ctor.parameters.iter().any(|p| {
        (p.name == "_args" || p.name == "args")
            && p.param_type
                .as_ref()
                .is_some_and(|t| t.contains("PyTuple") || t.contains("Tuple"))
    });

    let has_kwargs = ctor.parameters.iter().any(|p| {
        (p.name == "_kwargs" || p.name == "kwargs")
            && p.param_type
                .as_ref()
                .is_some_and(|t| t.contains("PyDict") || t.contains("Dict"))
    });

    has_args && has_kwargs
}

/// Check if Rust parameter is a tuple type (for *args matching)
fn is_rust_tuple_param(param: &crate::model::ParameterDef) -> bool {
    param
        .param_type
        .as_ref()
        .is_some_and(|t| t.contains("PyTuple") || t.contains("Tuple"))
}

/// Check one constructor's parameter roles against its resolved audited
/// origin. The policy per origin:
/// - BevyNew/Mixed: all mapped-function inputs positional-or-keyword in the
///   mapped function's order; convenience defaults form a trailing suffix.
/// - FieldDerived/EnumVariant: every supplied field keyword-only in upstream
///   field declaration order.
/// - TuplePayload: positional-or-keyword preserved (no order policy).
/// - ZeroArg: no parameters beyond self.
/// - NonConstructible: must not have a public initializer (E016).
/// - Factory/PythonAdapter: reviewed exceptions, no positional policy.
pub fn check_constructor_policy(origin: &ConstructorOrigin, class: &PyClassDef) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let Some(constructor) = class.constructor.as_ref() else {
        return diagnostics;
    };

    match origin {
        ConstructorOrigin::Unresolved => {
            // Fail closed: the caller (validate_all_impl) reports E013 with
            // the pinned-source context when the origin stays unresolved.
            diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::E013,
                    format!(
                        "constructor '{}' has no resolved audited origin; the pinned upstream declaration must be available before policy checks can pass",
                        class.python_name
                    ),
                )
                .with_note(
                    "unresolved origins never silently pass; resolve the pinned source or record a reviewed PythonAdapter exception",
                ),
            );
        }
        ConstructorOrigin::BevyNew {
            upstream_type,
            function,
            function_params,
        } => {
            for (index, param) in constructor.parameters.iter().enumerate() {
                if param.kind == ParameterKind::KeywordOnly {
                    let mut diag = Diagnostic::error(
                        DiagnosticCode::E014,
                        format!(
                            "constructor '{}' parameter {} ('{}') is keyword-only but the mapped upstream constructor {}::{} takes it positional-or-keyword",
                            class.python_name,
                            index + 1,
                            param.name,
                            upstream_type,
                            function
                        ),
                    );
                    if let Some(loc) = &constructor.location {
                        diag = diag.with_location(loc.clone());
                    }
                    diagnostics.push(diag);
                }
            }
            check_function_order(class, constructor, function_params, &mut diagnostics);
            check_default_suffix(
                class,
                constructor,
                constructor.parameters.len(),
                &mut diagnostics,
            );
        }
        ConstructorOrigin::FieldDerived {
            upstream_type,
            field_order,
        } => {
            for (index, param) in constructor.parameters.iter().enumerate() {
                if param.kind != ParameterKind::KeywordOnly {
                    let mut diag = Diagnostic::error(
                        DiagnosticCode::E014,
                        format!(
                            "field-derived constructor '{}' parameter {} ('{}') is positional but the upstream fields of {} are keyword-only in declaration order",
                            class.python_name,
                            index + 1,
                            param.name,
                            upstream_type
                        ),
                    );
                    if let Some(loc) = &constructor.location {
                        diag = diag.with_location(loc.clone());
                    }
                    diagnostics.push(diag);
                }
            }
            check_field_order(class, constructor, field_order, &mut diagnostics);
        }
        ConstructorOrigin::EnumVariant {
            upstream_type,
            variant,
            field_order,
        } => {
            for (index, param) in constructor.parameters.iter().enumerate() {
                if param.kind != ParameterKind::KeywordOnly {
                    let mut diag = Diagnostic::error(
                        DiagnosticCode::E014,
                        format!(
                            "named enum variant constructor '{}::{}' parameter {} ('{}') is positional but the variant's fields are keyword-only in declaration order",
                            class.python_name,
                            variant,
                            index + 1,
                            param.name
                        ),
                    )
                    .with_note(format!("upstream enum: {}", upstream_type));
                    if let Some(loc) = &constructor.location {
                        diag = diag.with_location(loc.clone());
                    }
                    diagnostics.push(diag);
                }
            }
            check_field_order(class, constructor, field_order, &mut diagnostics);
        }
        ConstructorOrigin::Mixed {
            upstream_type,
            function,
            function_params,
            tail_fields,
        } => {
            let tail_start = constructor
                .parameters
                .len()
                .saturating_sub(tail_fields.len());
            // Conditionally check the native prefix order only against the
            // constructor's own prefix, not the full parameter list.
            for (index, param) in constructor.parameters.iter().enumerate() {
                if index < tail_start {
                    if param.kind == ParameterKind::KeywordOnly {
                        let mut diag = Diagnostic::error(
                            DiagnosticCode::E014,
                            format!(
                                "mixed constructor '{}' native input {} ('{}') is keyword-only but the mapped upstream constructor {}::{} takes it positional-or-keyword",
                                class.python_name,
                                index + 1,
                                param.name,
                                upstream_type,
                                function
                            ),
                        );
                        if let Some(loc) = &constructor.location {
                            diag = diag.with_location(loc.clone());
                        }
                        diagnostics.push(diag);
                    }
                } else if param.kind != ParameterKind::KeywordOnly {
                    let mut diag = Diagnostic::error(
                        DiagnosticCode::E014,
                        format!(
                            "mixed constructor '{}' additional field input {} ('{}') is positional but additional inputs are keyword-only in field declaration order",
                            class.python_name,
                            index + 1,
                            param.name
                        ),
                    );
                    if let Some(loc) = &constructor.location {
                        diag = diag.with_location(loc.clone());
                    }
                    diagnostics.push(diag);
                }
            }
            if tail_start > 0 {
                check_prefix_order(
                    class,
                    constructor,
                    tail_start,
                    function_params,
                    &mut diagnostics,
                );
            }
            check_field_order(class, constructor, tail_fields, &mut diagnostics);
            check_default_suffix(class, constructor, tail_start, &mut diagnostics);
        }
        ConstructorOrigin::TuplePayload { .. }
        | ConstructorOrigin::Factory { .. }
        | ConstructorOrigin::PythonAdapter { .. } => {
            // Genuine tuple payloads keep positional-or-keyword construction;
            // factories and reviewed adapters carry their own exceptions.
        }
        ConstructorOrigin::ZeroArg { .. } => {
            if !constructor.parameters.is_empty() {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::E014,
                    format!(
                        "zero-argument constructor '{}' declares {} parameter(s)",
                        class.python_name,
                        constructor.parameters.len()
                    ),
                ));
            }
        }
        ConstructorOrigin::NonConstructible { upstream_type } => {
            let mut diag = Diagnostic::error(
                DiagnosticCode::E016,
                format!(
                    "non-constructible wrapper '{}' declares a public initializer; the upstream type {} is an explicitly non-constructible snapshot or enum base",
                    class.python_name, upstream_type
                ),
            );
            if let Some(loc) = &constructor.location {
                diag = diag.with_location(loc.clone());
            }
            diagnostics.push(diag);
        }
    }

    diagnostics
}

/// Convenience defaults on native constructor inputs must form a trailing
/// suffix: a default preceding a required native input is E015.
fn check_default_suffix(
    class: &PyClassDef,
    constructor: &crate::model::MethodDef,
    native_input_count: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen_default = false;
    for (index, param) in constructor
        .parameters
        .iter()
        .take(native_input_count)
        .enumerate()
    {
        if param.default_value.is_some() {
            seen_default = true;
        } else if seen_default {
            let mut diag = Diagnostic::error(
                DiagnosticCode::E015,
                format!(
                    "constructor '{}' convenience default precedes required native input {} ('{}'); remove the default instead of making the input keyword-only or reordering",
                    class.python_name,
                    index + 1,
                    param.name
                ),
            );
            if let Some(loc) = &constructor.location {
                diag = diag.with_location(loc.clone());
            }
            diagnostics.push(diag);
            return;
        }
    }
}

/// Supplied params must follow the mapped function's parameter order.
fn check_function_order(
    class: &PyClassDef,
    constructor: &crate::model::MethodDef,
    function_params: &[String],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let names: Vec<&str> = constructor
        .parameters
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    let positions: Vec<Option<usize>> = names
        .iter()
        .map(|name| function_params.iter().position(|param| param == name))
        .collect();

    let mut last_seen: Option<usize> = None;
    for (index, position) in positions.iter().enumerate() {
        let Some(pos) = position else {
            continue;
        };
        if let Some(seen) = last_seen
            && pos < &seen
        {
            let mut diag = Diagnostic::error(
                DiagnosticCode::E005,
                format!(
                    "constructor '{}' parameter {} ('{}') follows upstream parameter '{}' but precedes it in the mapped constructor order",
                    class.python_name,
                    index + 1,
                    names[index],
                    function_params[seen]
                ),
            )
            .with_note(format!(
                "upstream constructor order: {}",
                function_params.join(", ")
            ));
            if let Some(loc) = &constructor.location {
                diag = diag.with_location(loc.clone());
            }
            diagnostics.push(diag);
            return;
        }
        last_seen = Some(*pos);
    }
}

/// Check only the positional-prefix segment of a mixed constructor against
/// the mapped function's parameter order.
fn check_prefix_order(
    class: &PyClassDef,
    constructor: &crate::model::MethodDef,
    prefix_len: usize,
    function_params: &[String],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let names: Vec<&str> = constructor
        .parameters
        .iter()
        .take(prefix_len)
        .map(|p| p.name.as_str())
        .collect();
    let positions: Vec<Option<usize>> = names
        .iter()
        .map(|name| function_params.iter().position(|param| param == name))
        .collect();

    let mut last_seen: Option<usize> = None;
    for (index, position) in positions.iter().enumerate() {
        let Some(pos) = position else {
            continue;
        };
        if let Some(seen) = last_seen
            && pos < &seen
        {
            let mut diag = Diagnostic::error(
                DiagnosticCode::E005,
                format!(
                    "mixed constructor '{}' native input {} ('{}') follows upstream parameter '{}' but precedes it in the mapped constructor order",
                    class.python_name,
                    index + 1,
                    names[index],
                    function_params[seen]
                ),
            )
            .with_note(format!(
                "upstream constructor order: {}",
                function_params.join(", ")
            ));
            if let Some(loc) = &constructor.location {
                diag = diag.with_location(loc.clone());
            }
            diagnostics.push(diag);
            return;
        }
        last_seen = Some(*pos);
    }
}

/// Supplied field inputs must follow the upstream field declaration order.
fn check_field_order(
    class: &PyClassDef,
    constructor: &crate::model::MethodDef,
    field_order: &[String],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let names: Vec<&str> = constructor
        .parameters
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    let positions: Vec<Option<usize>> = names
        .iter()
        .map(|name| field_order.iter().position(|field| field == name))
        .collect();

    let mut last_seen: Option<usize> = None;
    for (index, position) in positions.iter().enumerate() {
        let Some(pos) = position else {
            continue;
        };
        if let Some(seen) = last_seen
            && pos < &seen
        {
            let mut diag = Diagnostic::error(
                DiagnosticCode::E005,
                format!(
                    "constructor '{}' parameter {} ('{}') follows upstream field '{}' but precedes it in the upstream declaration order",
                    class.python_name,
                    index + 1,
                    names[index],
                    field_order[seen]
                ),
            )
            .with_note(format!(
                "upstream field order: {}",
                field_order.join(", ")
            ));
            if let Some(loc) = &constructor.location {
                diag = diag.with_location(loc.clone());
            }
            diagnostics.push(diag);
            return;
        }
        last_seen = Some(*pos);
    }
}

/// Validate constructor matches between Rust and Python
pub fn validate_constructor(rust: &PyClassDef, python: &PyClassDef) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    match (&rust.constructor, &python.constructor) {
        (Some(rust_ctor), Some(py_ctor)) => {
            // Check parameter count
            // Special case: Python *args (has_varargs) can match a single Rust tuple parameter
            let params_match = if py_ctor.has_varargs
                && py_ctor.parameters.is_empty()
                && rust_ctor.parameters.len() == 1
                && is_rust_tuple_param(&rust_ctor.parameters[0])
            {
                // *args in Python matches single tuple param in Rust
                true
            } else {
                rust_ctor.parameters.len() == py_ctor.parameters.len()
            };

            if !params_match {
                let mut diag = Diagnostic::error(
                    DiagnosticCode::E008,
                    format!(
                        "constructor parameter count mismatch for '{}': Rust has {} params, Python has {}{}",
                        rust.python_name,
                        rust_ctor.parameters.len(),
                        py_ctor.parameters.len(),
                        if py_ctor.has_varargs {
                            " (plus *args)"
                        } else {
                            ""
                        }
                    ),
                );

                if let Some(ref loc) = rust_ctor.location {
                    diag = diag.with_location(loc.clone());
                }

                diagnostics.push(diag);
            } else if !py_ctor.has_varargs {
                // Only check parameter names if not using *args pattern
                // Check parameter names and order
                for (i, (rust_param, py_param)) in rust_ctor
                    .parameters
                    .iter()
                    .zip(py_ctor.parameters.iter())
                    .enumerate()
                {
                    if !param_names_match(&rust_param.name, &py_param.name) {
                        let mut diag = Diagnostic::error(
                            DiagnosticCode::E004,
                            format!(
                                "constructor parameter {} name mismatch for '{}': Rust='{}' vs Python='{}'",
                                i + 1,
                                rust.python_name,
                                rust_param.name,
                                py_param.name
                            ),
                        );

                        if let Some(ref loc) = rust_ctor.location {
                            diag = diag.with_location(loc.clone());
                        }

                        diag = diag
                            .with_note("Parameter names must match for keyword arguments to work");

                        diagnostics.push(diag);
                    }
                }
            }
        }
        (Some(rust_ctor), None) => {
            // Skip varargs subclass constructors (*args, **kwargs pattern)
            // These are base classes meant to be subclassed, and the Python stub
            // intentionally omits __init__ since users create subclasses
            if rust.subclass && is_varargs_subclass_constructor(rust_ctor) {
                // This is expected - base class with varargs for subclassing
            } else {
                let mut diag = Diagnostic::error(
                    DiagnosticCode::E008,
                    format!(
                        "class '{}' has constructor in Rust but not in Python stub",
                        rust.python_name
                    ),
                );

                if let Some(ref loc) = rust_ctor.location {
                    diag = diag.with_location(loc.clone());
                }

                // Generate constructor stub
                let stub = generate_constructor_stub(rust_ctor);
                diag = diag.with_suggestion(
                    Suggestion::new("Add __init__ to stub").with_replacement(stub),
                );

                diagnostics.push(diag);
            }
        }
        (None, Some(_)) => {
            // Python-only constructor: allowed (default or implied constructor)
        }
        (None, None) => {
            // Neither has constructor - OK
        }
    }

    diagnostics
}

/// Check if a method is a context manager protocol method
fn is_context_manager_method(name: &str) -> bool {
    name == "__enter__" || name == "__exit__"
}

/// Type checkers treat protocol-method arguments as positional-only even when
/// the stub gives them descriptive names, so the names are not part of the keyword contract.
fn is_positional_protocol_method(name: &str) -> bool {
    name.starts_with("__") && name.ends_with("__")
}

/// Validate methods match between Rust and Python
pub fn validate_methods(rust: &PyClassDef, python: &PyClassDef) -> Vec<Diagnostic> {
    validate_methods_with_config(rust, python, None)
}

/// Validate methods, including reviewed methods installed by a root adapter.
pub fn validate_methods_with_config(
    rust: &PyClassDef,
    python: &PyClassDef,
    config: Option<&BevyConfig>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Check each Rust method exists in Python
    for rust_method in &rust.methods {
        let py_method = python.methods.iter().find(|m| m.name == rust_method.name);

        match py_method {
            Some(py_method) => {
                diagnostics.extend(validate_method_return(rust, python, rust_method, py_method));
                // Skip parameter validation for context manager methods
                // - __enter__ in Python has no params, Rust needs `py: Python`
                // - __exit__ has standard Python names that differ from Rust convention
                if is_context_manager_method(&rust_method.name) {
                    continue;
                }

                // Check if parameter counts match
                // Special case: Python *args matches a single Rust tuple parameter
                // e.g., Python: `def add_systems(self, stage, *systems)` (1 param + varargs)
                //       Rust: `fn add_systems(&self, schedule, systems: &Bound<PyTuple>)` (2 params)
                // Also handles keyword-only args after *args:
                // e.g., Python: `def spawn_batch(self, *args, count=None)` (1 param + varargs)
                //       Rust: `fn spawn_batch(&self, args: Tuple, count: Option)` (2 params)
                let params_match = if py_method.has_varargs {
                    // With varargs, Python params + 1 (for the tuple) should equal Rust params
                    // Check that Rust has at least one tuple parameter (not necessarily last,
                    // since keyword-only args in Python come after *args)
                    let rust_has_tuple_param =
                        rust_method.parameters.iter().any(is_rust_tuple_param);

                    rust_has_tuple_param
                        && py_method.parameters.len() + 1 == rust_method.parameters.len()
                } else {
                    rust_method.parameters.len() == py_method.parameters.len()
                };

                // Validate parameter names match
                if !params_match {
                    let mut diag = Diagnostic::warning(
                        DiagnosticCode::E008,
                        format!(
                            "method '{}::{}' parameter count mismatch: Rust has {} params, Python has {}{}",
                            rust.python_name,
                            rust_method.name,
                            rust_method.parameters.len(),
                            py_method.parameters.len(),
                            if py_method.has_varargs {
                                " (plus *args)"
                            } else {
                                ""
                            }
                        ),
                    );

                    if let Some(ref loc) = rust_method.location {
                        diag = diag.with_location(loc.clone());
                    }

                    diagnostics.push(diag);
                } else if !py_method.has_varargs
                    && !is_positional_protocol_method(&rust_method.name)
                {
                    // Only check keyword-capable parameter names. Protocol
                    // method arguments are positional-only to type checkers.
                    // Check parameter names
                    for (i, (rust_param, py_param)) in rust_method
                        .parameters
                        .iter()
                        .zip(py_method.parameters.iter())
                        .enumerate()
                    {
                        if !param_names_match(&rust_param.name, &py_param.name) {
                            let mut diag = Diagnostic::warning(
                                DiagnosticCode::E004,
                                format!(
                                    "method '{}::{}' parameter {} name mismatch: Rust='{}' vs Python='{}'",
                                    rust.python_name,
                                    rust_method.name,
                                    i + 1,
                                    rust_param.name,
                                    py_param.name
                                ),
                            );

                            if let Some(ref loc) = rust_method.location {
                                diag = diag.with_location(loc.clone());
                            }

                            diagnostics.push(diag);
                        }
                    }
                }
            }
            None => {
                // Private and special methods are intentionally absent from
                // the public stub contract.
                if rust_method.name.starts_with('_') {
                    continue;
                }

                let mut diag = Diagnostic::error(
                    DiagnosticCode::E002,
                    format!(
                        "method '{}::{}' not found in Python stub",
                        rust.python_name, rust_method.name
                    ),
                );

                if let Some(ref loc) = rust_method.location {
                    diag = diag.with_location(loc.clone());
                }

                let stub = generate_method_stub(rust_method);
                diag = diag
                    .with_suggestion(Suggestion::new("Add method to stub").with_replacement(stub));

                diagnostics.push(diag);
            }
        }
    }

    // Check static methods
    for rust_method in &rust.static_methods {
        let py_method = python
            .static_methods
            .iter()
            .find(|m| m.name == rust_method.name);

        if let Some(py_method) = py_method {
            diagnostics.extend(validate_method_return(rust, python, rust_method, py_method));
        }

        // Also check if it exists as a ClassVar (PyO3 #[staticmethod] constants
        // are often stubbed as ClassVar[T] since they're accessed without parens)
        let is_class_attr = python
            .class_attrs
            .iter()
            .any(|a| a.name == rust_method.name);

        if py_method.is_none() && !is_class_attr {
            // Private and special methods are intentionally absent from the
            // public stub contract.
            if rust_method.name.starts_with('_') {
                continue;
            }

            let mut diag = Diagnostic::error(
                DiagnosticCode::E002,
                format!(
                    "static method '{}::{}' not found in Python stub",
                    rust.python_name, rust_method.name
                ),
            );

            if let Some(ref loc) = rust_method.location {
                diag = diag.with_location(loc.clone());
            }

            let stub = generate_static_method_stub(rust_method);
            diag = diag.with_suggestion(
                Suggestion::new("Add static method to stub").with_replacement(stub),
            );

            diagnostics.push(diag);
        }
    }

    // Check for extra methods in Python stub (not in Rust)
    for py_method in &python.methods {
        // Check if method exists in Rust
        let in_rust_methods = rust.methods.iter().any(|m| m.name == py_method.name);

        // Check if method matches a tuple enum variant (PyO3 generates callable constructors for these)
        // e.g., `enum Volume { Linear(f32) }` generates `Volume.Linear(0.5)` in Python
        let is_tuple_variant_constructor = rust.is_enum
            && rust
                .enum_variants
                .iter()
                .any(|v| matches!(v.kind, EnumVariantKind::Tuple(_)) && v.name == py_method.name);

        // Check if this is a known runtime delegate pattern
        // Query stub documents PyQueryIter methods - PyQuery is just a type annotation marker,
        // but PyQueryIter (the runtime object injected into systems) has the actual methods
        let is_runtime_delegate = rust.python_name == "Query"
            && ["get", "get_mut", "single", "is_empty", "iter_many"]
                .contains(&py_method.name.as_str());

        // Native plugin wrappers receive build() from the pyplugin bridge,
        // which the parser cannot see; the stub documents the Plugin
        // override point.
        let is_plugin_build =
            py_method.name == "build" && python.extends.as_deref() == Some("Plugin");
        let is_runtime_injected = config.is_some_and(|config| {
            config.is_runtime_injected_method(&rust.python_name, &py_method.name)
        });

        if !in_rust_methods
            && !is_tuple_variant_constructor
            && !is_runtime_delegate
            && !is_plugin_build
            && !is_runtime_injected
        {
            // Skip dunder methods and some common Python-specific methods
            if py_method.name.starts_with("__") && py_method.name.ends_with("__") {
                continue;
            }

            let mut diag = Diagnostic::warning(
                DiagnosticCode::E003,
                format!(
                    "method '{}::{}' in Python stub not found in Rust",
                    rust.python_name, py_method.name
                ),
            );

            // Use method location if available, otherwise use class location
            if let Some(ref loc) = py_method.location {
                diag = diag.with_location(loc.clone());
            } else if let Some(ref loc) = python.location {
                diag = diag.with_location(loc.clone());
            }

            diag = diag.with_note("Method may be inherited or generated by a macro");

            diagnostics.push(diag);
        }
    }

    for py_method in python.methods.iter().chain(&python.static_methods) {
        let native = rust
            .methods
            .iter()
            .chain(&rust.static_methods)
            .find(|method| method.name == py_method.name);
        if let Some(native) = native {
            for diagnostic in validate_method_return(rust, python, native, py_method) {
                if !diagnostics
                    .iter()
                    .any(|existing| existing.message == diagnostic.message)
                {
                    diagnostics.push(diagnostic);
                }
            }
        }
    }
    diagnostics
}

fn is_inplace_operator(name: &str) -> bool {
    matches!(
        name,
        "__iadd__"
            | "__isub__"
            | "__imul__"
            | "__imatmul__"
            | "__itruediv__"
            | "__ifloordiv__"
            | "__imod__"
            | "__ipow__"
            | "__ilshift__"
            | "__irshift__"
            | "__iand__"
            | "__ixor__"
            | "__ior__"
    )
}

fn validate_method_return(
    class: &PyClassDef,
    stub_class: &PyClassDef,
    rust: &MethodDef,
    python: &MethodDef,
) -> Vec<Diagnostic> {
    let Some(python_return) = python.return_type.as_deref() else {
        return Vec::new();
    };
    let rust_return = rust.return_type.as_deref().unwrap_or("()");
    let mut native = normalize_rust_type(&resolve_self(rust_return, &class.python_name));
    if rust.name == "__next__" || rust.name == "__anext__" {
        native = native
            .strip_suffix(" | None")
            .unwrap_or(&native)
            .to_string();
    }
    if is_inplace_operator(&rust.name) && native == "None" {
        native = class.python_name.clone();
    }
    let variant = python_return
        .strip_prefix(&format!("{}.", class.python_name))
        .map(|name| name.split('[').next().unwrap_or(name));
    if native == class.python_name
        && variant.is_some_and(|name| {
            class
                .enum_variants
                .iter()
                .chain(&stub_class.enum_variants)
                .any(|variant| variant.name == name)
                || stub_class.nested_types.iter().any(|nested| nested == name)
        })
    {
        return Vec::new();
    }
    let stub = resolve_self(python_return, &class.python_name);
    if types_compatible(&native, &stub) {
        return Vec::new();
    }
    let mut diagnostic = Diagnostic::warning(
        DiagnosticCode::E006,
        format!(
            "method '{}::{}' return type mismatch: Rust='{}' vs Python='{}'",
            class.python_name, rust.name, rust_return, python_return
        ),
    );
    if let Some(location) = &rust.location {
        diagnostic = diagnostic.with_location(location.clone());
    }
    vec![diagnostic]
}

/// Generate a constructor stub
fn generate_constructor_stub(ctor: &crate::model::MethodDef) -> String {
    let mut stub = String::from("def __init__(\n    self,\n");

    for param in &ctor.parameters {
        let ty = param
            .param_type
            .as_ref()
            .map(|t| crate::rust_parser::types::normalize_rust_type(t))
            .unwrap_or_else(|| "Any".to_string());

        if let Some(ref default) = param.default_value {
            stub.push_str(&format!("    {}: {} = {},\n", param.name, ty, default));
        } else if param.is_optional {
            stub.push_str(&format!("    {}: {} = None,\n", param.name, ty));
        } else {
            stub.push_str(&format!("    {}: {},\n", param.name, ty));
        }
    }

    stub.push_str(") -> None: ...");
    stub
}

/// Generate a method stub
fn generate_method_stub(method: &crate::model::MethodDef) -> String {
    let mut stub = format!("def {}(\n    self,\n", method.name);

    for param in &method.parameters {
        let ty = param
            .param_type
            .as_ref()
            .map(|t| crate::rust_parser::types::normalize_rust_type(t))
            .unwrap_or_else(|| "Any".to_string());

        if let Some(ref default) = param.default_value {
            stub.push_str(&format!("    {}: {} = {},\n", param.name, ty, default));
        } else {
            stub.push_str(&format!("    {}: {},\n", param.name, ty));
        }
    }

    let ret = method
        .return_type
        .as_ref()
        .map(|t| crate::rust_parser::types::normalize_rust_type(t))
        .unwrap_or_else(|| "None".to_string());

    stub.push_str(&format!(") -> {}: ...", ret));
    stub
}

/// Generate a static method stub
fn generate_static_method_stub(method: &crate::model::MethodDef) -> String {
    let mut stub = String::from("@staticmethod\n");
    stub.push_str(&format!("def {}(\n", method.name));

    for param in &method.parameters {
        let ty = param
            .param_type
            .as_ref()
            .map(|t| crate::rust_parser::types::normalize_rust_type(t))
            .unwrap_or_else(|| "Any".to_string());

        if let Some(ref default) = param.default_value {
            stub.push_str(&format!("    {}: {} = {},\n", param.name, ty, default));
        } else {
            stub.push_str(&format!("    {}: {},\n", param.name, ty));
        }
    }

    let ret = method
        .return_type
        .as_ref()
        .map(|t| crate::rust_parser::types::normalize_rust_type(t))
        .unwrap_or_else(|| "None".to_string());

    stub.push_str(&format!(") -> {}: ...", ret));
    stub
}

/// Bevy's `with_*` builders take `mut self` by value, so the caller never observes a mutation.
/// Taking a mutable receiver instead writes through to the caller's value (and the ECS when
/// the receiver is a borrowed component).
pub fn validate_builder_receivers(rust: &PyClassDef) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for method in &rust.methods {
        if !is_value_builder_name(&method.name) || !returns_self(method) {
            continue;
        }

        let receiver = match method.self_mutability {
            SelfMutability::RefMut => "&mut self",
            SelfMutability::Owned => "slf: Py<Self>",
            SelfMutability::Ref | SelfMutability::None => continue,
        };

        let mut diag = Diagnostic::warning(
            DiagnosticCode::W010,
            format!(
                "builder '{}::{}' returns Self but takes `{}`, so it mutates the caller's value",
                rust.python_name, method.name, receiver
            ),
        );

        if let Some(ref loc) = method.location {
            diag = diag.with_location(loc.clone());
        }

        diagnostics.push(
            diag.with_note(
                "bevy takes `mut self` by value here; the caller's own value is never modified",
            )
            .with_note("on a component from Query[Mut[T]] this writes straight into the ECS")
            .with_suggestion(
                Suggestion::new("take `&self`, copy or clone the value, and return the copy")
                    .with_replacement(format!(
                        "pub fn {}(&self, py: Python<'_>, ...) -> PyResult<Py<Self>>",
                        method.rust_name
                    )),
            ),
        );
    }

    diagnostics
}

/// `with_x`, `with_` (bevy's `with`, renamed around the Python keyword),
/// `without`/`without_x`, and past-participle builders such as `transformed_by`.
fn is_value_builder_name(name: &str) -> bool {
    name.starts_with("with_")
        || name == "without"
        || name.starts_with("without_")
        || name.ends_with("ed_by")
}

/// Whether the return type names `Self`, covering `PyResult<Py<Self>>`,
/// `PyResult<PyRefMut<'py, Self>>` and bare `Self`.
fn returns_self(method: &crate::model::MethodDef) -> bool {
    method.return_type.as_ref().is_some_and(|ret| {
        ret.split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|t| t == "Self")
    })
}
