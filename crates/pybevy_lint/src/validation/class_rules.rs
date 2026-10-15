use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    sync::LazyLock,
};

use regex::Regex;

use crate::{
    bevy_parser::types::{BevyItem, BevyItemKind},
    config::{BevyConfig, is_keyword_escape_of},
    model::{EnumVariantKind, MacroInfo, MethodDef, PyClassDef, SelfMutability, SourceLocation},
    output::{Diagnostic, DiagnosticCode, Suggestion},
    validation::origin::{source_field_types, source_struct_is_generic},
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

/// Case/underscore-insensitive comparison key for variant and method names.
fn normalize_variant_name(name: &str) -> String {
    name.to_lowercase().replace('_', "")
}

/// Field types that keep a strong Python reference alive. Field types are
/// syn pretty-printed with spaces around generics (`Py < PyArray >`), so
/// normalize before matching; identifiers are compared whole so that names
/// merely ending in `Py` (`NumPy<...>`) do not match.
fn holds_python_reference(field_type: &str) -> bool {
    let normalized = field_type.replace(' ', "");
    let mut rest = normalized.as_str();
    while !rest.is_empty() {
        let start = rest
            .find(|c: char| c.is_ascii_alphanumeric() || c == '_')
            .unwrap_or(rest.len());
        rest = &rest[start..];
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let (identifier, tail) = rest.split_at(end);
        let generic = tail.starts_with('<');
        if identifier == "PyObject"
            || (generic && matches!(identifier, "Py" | "PyRef" | "PyRefMut"))
        {
            return true;
        }
        rest = tail;
    }
    false
}

/// Whether a field type wraps its Python reference in an outer wrapper
/// (`Arc<...>`, `Mutex<...>`) before the first `Py` token.
fn wraps_python_reference(field_type: &str, wrappers: &[&str]) -> bool {
    let normalized = field_type.replace(' ', "");
    let prefix = normalized
        .find("Py<")
        .or_else(|| normalized.find("PyObject"))
        .map_or(normalized.as_str(), |index| &normalized[..index]);
    wrappers.iter().any(|wrapper| {
        prefix
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .any(|segment| segment == *wrapper)
    })
}

/// Flags GC-visible pyclasses that hold strong Python references in struct
/// fields but declare no `__traverse__`, so the GC cannot see the reference
/// and cyclic garbage survives. Reviewed exceptions (PyState/PyNextState
/// state machines, the GC bookkeeping classes) are config-scoped. Only the
/// presence of `__traverse__` is checked, not what it visits.
pub fn validate_traverse_presence(rust: &PyClassDef) -> Vec<Diagnostic> {
    let holding: Vec<(String, String)> = rust
        .struct_fields
        .iter()
        .filter(|(_, field_type)| holds_python_reference(field_type))
        .map(|(name, field_type)| (name.clone(), field_type.replace(' ', "")))
        .collect();
    if holding.is_empty() {
        return Vec::new();
    }
    let has_traverse = rust
        .methods
        .iter()
        .any(|method| method.name == "__traverse__");
    if has_traverse {
        return Vec::new();
    }

    let fields: Vec<String> = holding
        .iter()
        .map(|(name, field_type)| format!("{}: {}", name, field_type))
        .collect();
    let mut diag = Diagnostic::warning(
        DiagnosticCode::W032,
        format!(
            "pyclass '{}' holds Python references ({}) but declares no __traverse__",
            rust.python_name,
            fields.join(", ")
        ),
    )
    .with_note(
        "the GC cannot see references it is not shown; cyclic garbage through these fields survives",
    );
    if holding
        .iter()
        .any(|(_, field_type)| wraps_python_reference(field_type, &["Arc", "Rc"]))
    {
        diag = diag.with_note(
            "Arc clones share one Python incref: traverse only a reference this holder owns (its own clone_ref), never a handle another traversing owner also reports",
        );
    }
    if holding
        .iter()
        .any(|(_, field_type)| wraps_python_reference(field_type, &["Mutex", "RwLock", "RefCell"]))
    {
        diag = diag.with_note(
            "never take a blocking lock inside __traverse__: use try_lock/try_borrow and skip the traverse on contention, only where every locker is attached",
        );
    }
    diag = diag.with_suggestion(Suggestion::new(
        "add a __traverse__ pymethod that visits each held reference, or record a reviewed validation exception if the field is intentionally GC-invisible",
    ));
    if let Some(ref loc) = rust.location {
        diag = diag.with_location(loc.clone());
    }
    vec![diag]
}

/// Payload field names a variant's `__match_args__` must declare, in order.
/// Multi-field unnamed payloads have no convention-defined names.
fn variant_field_names(kind: &EnumVariantKind) -> Option<Vec<String>> {
    match kind {
        EnumVariantKind::Unit | EnumVariantKind::EmptyTuple => Some(Vec::new()),
        // A single unnamed tuple payload defaults to the field name `value`.
        EnumVariantKind::Tuple(fields) if fields.len() == 1 => Some(vec!["value".to_string()]),
        EnumVariantKind::Tuple(_) => None,
        EnumVariantKind::Struct(fields) => {
            Some(fields.iter().map(|(name, _)| name.clone()).collect())
        }
    }
}

/// W033: a stub's `__match_args__` must name the variant's own payload fields
/// in order (the Python pattern-matching contract), and for generated enums
/// those fields are the Rust payload's (manual adapters only get the
/// stub-internal check, since the Rust side carries no field graph).
pub fn validate_match_args_conformance(rust: &PyClassDef, python: &PyClassDef) -> Vec<Diagnostic> {
    if !rust.is_enum {
        return Vec::new();
    }
    let mut diagnostics = Vec::new();
    for py_variant in &python.enum_variants {
        let Some(declared) = &py_variant.match_args else {
            continue;
        };
        let Some(stub_fields) = variant_field_names(&py_variant.kind) else {
            continue;
        };
        if declared != &stub_fields {
            let mut diag = Diagnostic::warning(
                DiagnosticCode::W033,
                format!(
                    "variant '{}' declares __match_args__ ({}) that does not match its stub payload fields ({})",
                    py_variant.name,
                    declared.join(", "),
                    stub_fields.join(", ")
                ),
            );
            if let Some(ref loc) = python.location {
                diag = diag.with_location(loc.clone());
            }
            diagnostics.push(diag);
            continue;
        }
        if rust.manual_enum_verified {
            continue;
        }
        let Some(rust_variant) = rust
            .enum_variants
            .iter()
            .find(|variant| variant.name == py_variant.name)
        else {
            continue;
        };
        let Some(rust_fields) = variant_field_names(&rust_variant.kind) else {
            continue;
        };
        if declared != &rust_fields {
            let mut diag = Diagnostic::warning(
                DiagnosticCode::W033,
                format!(
                    "variant '{}' declares __match_args__ ({}) that does not match the Rust payload fields ({})",
                    py_variant.name,
                    declared.join(", "),
                    rust_fields.join(", ")
                ),
            )
            .with_note("the generated constructor derives __match_args__ from the payload fields");
            if let Some(ref loc) = python.location {
                diag = diag.with_location(loc.clone());
            }
            diagnostics.push(diag);
        }
    }
    diagnostics
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
    // Legacy plain `#[pyclass]` enums with a resolved upstream enum get the same
    // parity check; the upstream resolution is the guard against non-Bevy enums.
    let ordinary = ordinary_pyenum || rust.macro_info.is_none();
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

    if !ordinary
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

/// Flags wrappers that expose Python hashing while the resolved upstream type
/// does not implement `Hash`. The inverse parity of W015, which only sees the
/// upstream-has-Hash direction.
pub fn validate_extra_hash(rust: &PyClassDef, bevy: &BevyItem) -> Vec<Diagnostic> {
    let upstream_hashes = bevy
        .trait_impls
        .iter()
        .any(|trait_name| trait_name == "Hash" || trait_name.ends_with("::Hash"));

    if !rust.hash
        || !rust.eq
        || !rust.frozen
        || rust.eq_int
        || upstream_hashes
        || (bevy.kind != BevyItemKind::Enum && bevy.kind != BevyItemKind::Struct)
        || bevy.is_generic_base
    {
        return Vec::new();
    }

    let mut diagnostic = Diagnostic::warning(
        DiagnosticCode::W021,
        format!(
            "wrapper '{}' declares Python hashing but upstream type '{}' does not implement Hash",
            rust.python_name, bevy.full_path
        ),
    )
    .with_note("hashability must mirror Bevy in both directions; PyO3 hashing without an upstream Hash contract is a silent parity break")
    .with_suggestion(Suggestion::new(
        "remove `hash` from `#[pyclass(...)]` and the `Hash` derive, or record an exception with the reason",
    ));
    if let Some(location) = &rust.location {
        diagnostic = diagnostic.with_location(location.clone());
    }

    vec![diagnostic]
}

/// Flags nested PyBevy wrapper fields exposed through plain struct-field getters.
/// In borrowed (component) mode the getter hands Python a detached clone, so child
/// reads/writes silently miss the parent; storage-backed getters are the supported
/// pattern.
pub fn validate_nested_wrapper_getters(
    rust: &PyClassDef,
    wrapper_rust_names: &HashSet<String>,
) -> Vec<Diagnostic> {
    if rust.storage_macro.is_some() || rust.python_name.starts_with('_') {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    for property in rust
        .properties
        .iter()
        .filter(|property| property.has_getter)
    {
        if property.name.starts_with('_') {
            continue;
        }
        let Some((_, field_type)) = rust
            .struct_fields
            .iter()
            .find(|(name, _)| name == &property.name)
        else {
            continue;
        };
        let type_name = field_type
            .split('<')
            .next()
            .unwrap_or(field_type)
            .rsplit("::")
            .next()
            .unwrap_or(field_type)
            .to_string();
        // Handles and asset IDs are detached values by design; the concern is
        // nested live wrappers (vectors, colors, component values).
        if type_name.contains("Handle") || type_name.contains("AssetId") {
            continue;
        }
        if !wrapper_rust_names.contains(&type_name) {
            continue;
        }
        // Only component-mode classes can appear in borrowed storage, where the
        // plain getter detaches a clone. Events and owned value classes are
        // delivered by copy and do not lose child writes.
        let component_mode =
            rust.bridge_info.is_some() || rust.extends.as_deref() == Some("PyComponent");
        if !component_mode {
            continue;
        }

        let mut diagnostic = Diagnostic::warning(
            DiagnosticCode::W023,
            format!(
                "getter '{}' exposes nested wrapper field '{}' through a plain #[pyo3(get)] copy; borrowed views detach child writes",
                property.name, type_name
            ),
        )
        .with_note("use ValueStorage/FieldStorage with FromBorrowedStorage so the getter borrows the live field")
        .with_suggestion(Suggestion::new(
            "move the field into storage and generate the getter from the shared field-borrow helpers",
        ));
        if let Some(location) = &rust.location {
            diagnostic = diagnostic.with_location(location.clone());
        }
        diagnostics.push(diagnostic);
    }

    diagnostics
}

/// Flags non-Copy upstream associated constants exposed as class attributes.
/// A ClassVar/classattr hands every caller the same instance for a type whose
/// Bevy value is not `Copy`; the supported exposure is a factory staticmethod.
pub fn validate_constant_typing(
    rust: &PyClassDef,
    python: &PyClassDef,
    bevy: &BevyItem,
) -> Vec<Diagnostic> {
    if bevy.constants.is_empty() {
        return Vec::new();
    }
    let upstream_copy = bevy
        .trait_impls
        .iter()
        .any(|trait_name| trait_name == "Copy" || trait_name.ends_with("::Copy"));
    if upstream_copy {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    let item_name = bevy.name.clone();
    for (const_name, const_type) in &bevy.constants {
        // Scalar constants (e.g. `FogFalloff::THRESHOLD: f32`) and foreign-typed
        // constants (e.g. `Mesh::ATTRIBUTE_POSITION: MeshVertexAttribute`) are
        // not shared instances; only self-typed constants (`Bloom::NATURAL: Bloom`)
        // create the shared-instance hazard.
        let const_type_name = const_type.rsplit("::").next().unwrap_or(const_type);
        if const_type != "Self" && const_type_name != item_name {
            continue;
        }
        let exposed_as_class_attr = python
            .class_attrs
            .iter()
            .any(|attr| attr.name.eq_ignore_ascii_case(const_name))
            || rust
                .class_attrs
                .iter()
                .any(|attr| attr.name.eq_ignore_ascii_case(const_name));
        // A Rust factory yields a fresh instance per access, including through
        // the `pybevy/_constants.py` descriptors that back ClassVar stubs.
        let exposed_as_factory = python
            .static_methods
            .iter()
            .chain(&rust.static_methods)
            .any(|method| method.name.eq_ignore_ascii_case(const_name));
        if !exposed_as_class_attr || exposed_as_factory {
            continue;
        }

        let mut diagnostic = Diagnostic::warning(
            DiagnosticCode::W024,
            format!(
                "upstream constant '{}' is not Copy but is exposed as a class attribute on '{}'; callers share one instance",
                const_name, rust.python_name
            ),
        )
        .with_note("expose non-Copy Bevy constants as a factory staticmethod returning a fresh instance")
        .with_suggestion(Suggestion::new(
            "replace the ClassVar/classattr with a @staticmethod factory and update the stub",
        ));
        if let Some(location) = &rust.location {
            diagnostic = diagnostic.with_location(location.clone());
        }
        diagnostics.push(diagnostic);
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

/// Flags upstream tuple-payload enum variants that the wrapper exposes with
/// named fields in a shape the convention does not tolerate. Manual enum
/// adapters (`#[pyenum(T, manual)]`) are out of scope: the author manages the
/// mapping and the exposure shape is a reviewed decision, not a macro
/// contract. The tolerated shapes (single-field `value` naming and
/// `#[py_bevy(tuple)]` adapters) are honored through the shared comparison
/// matcher, which compares shape (arity/naming) there, not types: the
/// inventory and the adapter routinely disagree on type spelling (qualified
/// paths, `*Payload` newtypes, `Cow` vs owned) and payload types are
/// validated by the wrapper and constructor rules.
pub fn validate_tuple_variant_exposure(rust: &PyClassDef, bevy: &BevyItem) -> Vec<Diagnostic> {
    use crate::{bevy_parser::types::BevyVariantKind, comparison::enum_variant_kinds_match};

    if bevy.kind != BevyItemKind::Enum || rust.enum_variants.is_empty() {
        return Vec::new();
    }
    if matches!(
        &rust.macro_info,
        Some(MacroInfo::BevyEnum { manual: true, .. })
    ) {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    for bevy_variant in &bevy.variants {
        let BevyVariantKind::Tuple(bevy_fields) = &bevy_variant.kind else {
            continue;
        };
        let Some(py_variant) = rust
            .enum_variants
            .iter()
            .find(|variant| variant.name == bevy_variant.name)
        else {
            continue;
        };
        let named_bevy_tuple = rust.bevy_tuple_variants.contains(&py_variant.name);
        if enum_variant_kinds_match(&bevy_variant.kind, &py_variant.kind, named_bevy_tuple) {
            continue;
        }
        let mut diagnostic = Diagnostic::warning(
            DiagnosticCode::W026,
            format!(
                "Bevy variant '{}.{}' is a {}-field tuple payload but the wrapper exposes named fields",
                bevy.name,
                bevy_variant.name,
                bevy_fields.len()
            ),
        )
        .with_note("tuple payloads must stay positional; expose the payload positionally or mark the variant #[py_bevy(tuple)] when the named exposure was reviewed")
        .with_suggestion(Suggestion::new(
            "construct the nested variant class positionally to mirror the Bevy tuple payload",
        ));
        if let Some(location) = &rust.location {
            diagnostic = diagnostic.with_location(location.clone());
        }
        diagnostics.push(diagnostic);
    }

    diagnostics
}

/// W029: the Python registration module must match the canonical Bevy
/// placement. The wrapper's `module = "pybevy.X"` is compared against the
/// module mapped from the resolved Bevy crate via `[bevy.crate_mappings]`.
/// Rust ownership may stay in a lower-level crate to avoid dependency
/// cycles; the user-facing import path must not drift. Unmapped Bevy crates,
/// non-struct/enum items, excluded and false-match types, and
/// `[bevy.module_placement_overrides]` entries stay silent.
pub fn validate_module_placement(
    rust: &PyClassDef,
    bevy_crate: &str,
    bevy: &BevyItem,
    config: &BevyConfig,
) -> Vec<Diagnostic> {
    if !matches!(bevy.kind, BevyItemKind::Struct | BevyItemKind::Enum) {
        return Vec::new();
    }
    if bevy.is_generic_base
        || config.is_type_excluded_in_crate(&bevy.name, Some(bevy_crate))
        || config.is_false_match(&bevy.name, bevy_crate)
    {
        return Vec::new();
    }

    let Some(actual_full) = rust.python_module_path.as_deref() else {
        return Vec::new(); // W016 territory
    };
    let Some(actual) = actual_full.strip_prefix("pybevy.") else {
        return Vec::new();
    };

    let expected = config
        .crate_mappings
        .iter()
        .filter(|(_, crate_name)| crate_name.as_str() == bevy_crate)
        .map(|(module, _)| module.as_str())
        .min();
    let Some(expected) = expected else {
        return Vec::new(); // unmapped crate: no canonical module defined
    };
    if expected == actual {
        return Vec::new();
    }
    if config.is_module_placement_override(&bevy.name, bevy_crate, actual) {
        return Vec::new();
    }

    let mut diagnostic = Diagnostic::warning(
        DiagnosticCode::W029,
        format!(
            "wrapper '{}' is registered in Python module 'pybevy.{}', but Bevy type '{}' belongs to 'pybevy.{}'",
            rust.python_name, actual, bevy.full_path, expected
        ),
    )
    .with_note(
        "the canonical Bevy public item path fixes the Python module; Rust ownership may stay in a lower-level crate",
    )
    .with_suggestion(Suggestion::new(
        "move the wrapper's `module` and stub to the canonical module, or record [bevy.module_placement_overrides] with the reason when the placement is intentional",
    ));
    if let Some(location) = &rust.location {
        diagnostic = diagnostic.with_location(location.clone());
    }
    vec![diagnostic]
}

/// W031: every exposed variant of a resolved upstream enum must exist
/// upstream. A PyBevy variant name that is neither an upstream variant name
/// nor a configured rename target is a renamed/misspelled variant or an
/// unreviewed Python-only addition. Manual enum adapters (is_enum = false)
/// are out of scope: their variant classes materialize the upstream names.
pub fn validate_variant_name_parity(
    rust: &PyClassDef,
    bevy: &BevyItem,
    bevy_config: &BevyConfig,
) -> Vec<Diagnostic> {
    if bevy.kind != BevyItemKind::Enum || !rust.is_enum || rust.enum_variants.is_empty() {
        return Vec::new();
    }

    let upstream_names: HashSet<&str> = bevy
        .variants
        .iter()
        .map(|variant| variant.name.as_str())
        .collect();

    let mut diagnostics = Vec::new();
    for variant in &rust.enum_variants {
        if upstream_names.contains(variant.name.as_str())
            || bevy_config.is_rename_target(&bevy.name, &variant.name)
            || is_keyword_escape_of(&variant.name, |base| upstream_names.contains(base))
            || bevy_config.is_variant_intentionally_extra(&bevy.name, &variant.name)
        {
            continue;
        }
        let mut diagnostic = Diagnostic::warning(
            DiagnosticCode::W031,
            format!(
                "enum '{}' exposes PyBevy variant '{}' that is not present in the upstream Bevy enum",
                rust.python_name, variant.name
            ),
        )
        .with_note(format!(
            "upstream variants: {}",
            bevy
                .variants
                .iter()
                .map(|variant| variant.name.as_str())
                .take(12)
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .with_suggestion(Suggestion::new(
            "rename the variant to the upstream name, record the rename under [bevy.type_ignores] with the reason, or record a validation exception for a reviewed Python-only variant",
        ));
        if let Some(location) = &rust.location {
            diagnostic = diagnostic.with_location(location.clone());
        }
        diagnostics.push(diagnostic);
    }
    diagnostics
}

/// W031 struct-spelling arm: a struct wrapper that mirrors upstream enum
/// variants as classattrs/builder methods must spell them with the upstream
/// names, except for the `None` -> `None_` keyword escape, the
/// `__pybevy_variant_aliases__` table, and configured renames.
/// A candidate emulates a unit variant when it takes no payload arguments
/// (payload constructors like `CursorIcon.system(icon)` are reviewed
/// component-mode builders, not variant aliases).
fn is_zero_arg_candidate(method: &MethodDef) -> bool {
    method.parameters.iter().all(|param| param.name == "py")
}

fn variant_spelling_candidates(rust: &PyClassDef) -> Vec<String> {
    rust.class_attrs
        .iter()
        .map(|attr| attr.name.clone())
        .chain(
            rust.static_methods
                .iter()
                .filter(|method| is_zero_arg_candidate(method))
                .map(|method| method.name.clone()),
        )
        .chain(
            rust.methods
                .iter()
                .filter(|method| {
                    !method.name.starts_with('_')
                        && is_zero_arg_candidate(method)
                        && (method.is_static
                            || method.is_class_method
                            || method.self_mutability == SelfMutability::None)
                })
                .map(|method| method.name.clone()),
        )
        .collect()
}

/// Parses the `vec![("Upstream", "Alias"), ...]` body of the
/// `__pybevy_variant_aliases__` classattr, if present.
fn parse_variant_alias_table(rust: &PyClassDef) -> HashSet<(String, String)> {
    let Some(method) = rust
        .methods
        .iter()
        .find(|method| method.name == "__pybevy_variant_aliases__")
    else {
        return HashSet::new();
    };
    let Some(location) = &method.location else {
        return HashSet::new();
    };
    let Ok(source) = fs::read_to_string(&location.file) else {
        return HashSet::new();
    };
    let lines: Vec<&str> = source.lines().collect();
    let start = location.line - 1;
    if start >= lines.len() {
        return HashSet::new();
    }
    let end = lines
        .iter()
        .skip(location.line)
        .take(60)
        .position(|line| line.trim_end() == "}")
        .map(|offset| offset + location.line)
        .unwrap_or_else(|| location.line + 60)
        .min(lines.len());
    let region = lines[start..end].join("\n");
    let re = Regex::new(r#"\(\s*"([A-Za-z0-9_]+)"\s*,\s*"([A-Za-z0-9_]+)"\s*\)"#).ok();
    let Some(re) = re else {
        return HashSet::new();
    };
    re.captures_iter(&region)
        .map(|capture| (capture[1].to_string(), capture[2].to_string()))
        .collect()
}

pub fn validate_struct_variant_spelling(
    rust: &PyClassDef,
    bevy: &BevyItem,
    bevy_config: &BevyConfig,
) -> Vec<Diagnostic> {
    if bevy.kind != BevyItemKind::Enum
        || rust.is_enum
        || rust.macro_info.is_some()
        || bevy.variants.is_empty()
    {
        return Vec::new();
    }

    let candidates = variant_spelling_candidates(rust);
    let candidates: Vec<String> = candidates
        .into_iter()
        .filter(|name| !name.starts_with('_'))
        .collect();
    let aliases = parse_variant_alias_table(rust);

    let mut mismatches = Vec::new();
    for upstream in &bevy.variants {
        let upstream_key = normalize_variant_name(&upstream.name);
        let matched: Vec<&String> = candidates
            .iter()
            .filter(|candidate| normalize_variant_name(candidate) == upstream_key)
            .collect();
        if matched.is_empty() {
            continue;
        }
        let rename_target = bevy_config.get_rename(&bevy.name, &upstream.name);
        let explained = matched.iter().any(|candidate| {
            candidate.as_str() == upstream.name
                || aliases.contains(&(upstream.name.clone(), candidate.as_str().to_string()))
                || rename_target
                    .as_deref()
                    .is_some_and(|target| candidate.as_str() == target)
        });
        if !explained {
            mismatches.push((matched[0].clone(), upstream.name.clone()));
        }
    }

    if mismatches.is_empty() {
        return Vec::new();
    }

    let pairs: Vec<String> = mismatches
        .iter()
        .take(12)
        .map(|(exposed, upstream)| format!("'{exposed}' != '{upstream}'"))
        .collect();
    let mut diagnostic = Diagnostic::warning(
        DiagnosticCode::W031,
        format!(
            "struct '{}' exposes emulated variant names that do not match the upstream Bevy spelling: {}",
            rust.python_name,
            pairs.join(", ")
        ),
    )
    .with_note(
        "classattr/builder names for upstream variants use the upstream spelling, a Python keyword escape (None -> None_), a __pybevy_variant_aliases__ entry, or a configured rename",
    )
    .with_suggestion(Suggestion::new(
        "rename the classattrs/builders to the upstream variant spelling, or record the alias under __pybevy_variant_aliases__ or [bevy.type_ignores] with the reason",
    ));
    if let Some(location) = &rust.location {
        diagnostic = diagnostic.with_location(location.clone());
    }
    vec![diagnostic]
}

/// W035: a stub `Handle[T]` annotation (property or constructor parameter)
/// must name the same asset type as the upstream field it documents. Only
/// direct `Handle<T>` fields (optionally wrapped in `Option`/`NonSend`/`Arc`)
/// are comparable; other upstream shapes (lists, untyped handles, computed
/// values) are out of scope.
pub fn validate_handle_argument_parity(
    rust: &PyClassDef,
    python: &PyClassDef,
    bevy_item: &BevyItem,
    bevy_path: Option<&Path>,
) -> Vec<Diagnostic> {
    let Some(bevy_path) = bevy_path else {
        return Vec::new();
    };
    let Some(fields) = source_field_types(&bevy_item.full_path, Some(bevy_path)) else {
        return Vec::new();
    };
    // A generic struct's `Handle<M>` names a type parameter, not an asset.
    if source_struct_is_generic(&bevy_item.full_path, Some(bevy_path)) == Some(true) {
        return Vec::new();
    }
    // A single-field tuple struct (`Mesh3d(Handle<Mesh>)`) exposes its payload
    // under a domain name (`handle`), so any annotation documents field `0`.
    let tuple_payload = (fields.len() == 1).then(|| fields.get("0")).flatten();
    // The stub file may alias the asset class (e.g. `from pybevy.image import
    // Image as ImageAsset` to dodge a same-file name collision); resolve
    // aliases before comparing.
    let alias_map = python.location.as_ref().and_then(|location| {
        fs::read_to_string(&location.file).ok().map(|text| {
            stub_import_aliases(&text)
                .into_iter()
                .collect::<HashMap<String, String>>()
        })
    });
    let resolve = |argument: &str| -> String {
        alias_map
            .as_ref()
            .and_then(|map| map.get(argument))
            .cloned()
            .unwrap_or_else(|| argument.to_string())
    };
    let mut annotations: Vec<(String, String, Option<SourceLocation>)> = Vec::new();
    for property in &python.properties {
        if let Some(stub_type) = &property.property_type
            && let Some(argument) = handle_argument(stub_type).map(|argument| resolve(&argument))
        {
            annotations.push((property.name.clone(), argument.to_string(), None));
        }
    }
    if let Some(constructor) = &python.constructor {
        for parameter in &constructor.parameters {
            if let Some(stub_type) = &parameter.param_type
                && let Some(argument) =
                    handle_argument(stub_type).map(|argument| resolve(&argument))
            {
                let location = constructor.location.clone();
                annotations.push((parameter.name.clone(), argument.to_string(), location));
            }
        }
    }
    let mut diagnostics = Vec::new();
    for (name, argument, location) in annotations {
        let Some(upstream) = fields.get(&name).or(tuple_payload) else {
            continue;
        };
        let Some(upstream_argument) = handle_argument(upstream) else {
            continue;
        };
        if upstream_argument == argument {
            continue;
        }
        let mut diagnostic = Diagnostic::warning(
            DiagnosticCode::W035,
            format!(
                "stub handle argument for '{}.{}' is 'Handle[{}]', but the upstream field is 'Handle<{}>'",
                rust.python_name, name, argument, upstream_argument
            ),
        );
        if let Some(location) = location {
            diagnostic = diagnostic.with_location(location);
        }
        diagnostics.push(diagnostic);
    }
    diagnostics
}

/// Collect `from <module> import <name> as <alias>` pairs from stub source.
fn stub_import_aliases(text: &str) -> Vec<(String, String)> {
    let mut aliases = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(after_from) = line.strip_prefix("from ") else {
            continue;
        };
        let Some(import_index) = after_from.find(" import ") else {
            continue;
        };
        let items = &after_from[import_index + " import ".len()..];
        for item in items.split(',') {
            let item = item.trim();
            let Some((original, alias)) = item.split_once(" as ") else {
                continue;
            };
            let original = original.trim();
            let alias = alias.trim();
            if original
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && alias
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            {
                aliases.push((alias.to_string(), original.to_string()));
            }
        }
    }
    aliases
}

/// Extract the type argument of a `Handle[T]`/`Handle<T>` annotation. Wrappers
/// around a single `Handle` (`Option<...>`, `NonSend<...>`, `Arc<...>`) are
/// peeled; anything else (lists, untyped handles, computed values) is not a
/// comparable handle annotation. Stubs spell the argument with brackets
/// (`Handle[Image]`), upstream sources with angle brackets; the result uses
/// brackets and drops module qualifiers so both spellings compare equal.
fn handle_argument(annotation: &str) -> Option<String> {
    let mut text = annotation.replace(' ', "");
    loop {
        let peeled = ["Option<", "NonSend<", "Arc<"].iter().find_map(|wrapper| {
            text.strip_prefix(wrapper)
                .and_then(|rest| rest.strip_suffix('>'))
        });
        match peeled {
            Some(inner) => text = inner.to_string(),
            None => break,
        }
    }
    let text = text.replace('<', "[").replace('>', "]");
    let start = text.find("Handle[")?;
    let qualifier = &text[..start];
    if !qualifier
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '.')
        || !(qualifier.is_empty() || qualifier.ends_with("::") || qualifier.ends_with('.'))
    {
        return None;
    }
    let inner = &text[start + "Handle[".len()..];
    let mut depth = 0usize;
    let end = inner.char_indices().find_map(|(index, ch)| match ch {
        '[' => {
            depth += 1;
            None
        }
        ']' if depth == 0 => Some(index),
        ']' => {
            depth -= 1;
            None
        }
        _ => None,
    })?;
    let argument = HANDLE_QUALIFIER_RE.replace_all(&inner[..end], "");
    (!argument.is_empty()).then(|| argument.into_owned())
}

/// Module qualifiers (`bevy_image::`, `pybevy.image.`) in a type spelling.
static HANDLE_QUALIFIER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:[A-Za-z_][A-Za-z0-9_]*(?:::|\.))+").expect("static regex is valid")
});

/// A stub type that accepts `None`: a top-level `None` union member (in any
/// position) or an `Optional[...]` spelling. `None` nested inside a
/// subscript (`Callable[[], None]`, `tuple[int, None]`) does not count.
pub fn stub_none_accepting(stub_type: &str) -> bool {
    let text = stub_type.replace(' ', "");
    let mut depth = 0usize;
    let mut start = 0;
    let mut members = Vec::new();
    for (index, ch) in text.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            '|' if depth == 0 => {
                members.push(&text[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    members.push(&text[start..]);
    members
        .iter()
        .any(|member| *member == "None" || member.starts_with("Optional["))
}

/// Rust types whose Python value is dynamic (`PyAny`, `PyObject`): whether
/// they yield or accept `None` is not visible in the type.
pub fn is_dynamic_python_type(rust_type: &str) -> bool {
    let text = rust_type.replace(' ', "");
    text.contains("PyAny") || text.contains("PyObject")
}

/// Whether a Rust getter return type can yield `None`. Getters may wrap the
/// value in `PyResult`, so one `PyResult<...>` layer is peeled before
/// checking for an `Option`. `None` for a dynamic Python return type.
fn rust_return_optionality(rust_type: &str) -> Option<bool> {
    let mut text = rust_type.replace(' ', "");
    if let Some(inner) = text.strip_prefix("PyResult<") {
        text = inner.strip_suffix('>').unwrap_or(inner).to_string();
    }
    if text.starts_with("Option<") {
        return Some(true);
    }
    (!is_dynamic_python_type(&text)).then_some(false)
}

/// W036 property arm: the stub's `None` acceptance for a readable property
/// must match what the getter can return. The Rust getter's own return type
/// is the source of truth (a hand-written getter may legitimately hide an
/// upstream `Option`, e.g. `AssetPath.source` normalizes the upstream
/// source-id enum and returns `None` for the default source); the upstream
/// field's `Option` status is the fallback when the getter type is not
/// visible. Computed properties whose name is not an upstream field and
/// whose Rust type is unknown are out of scope.
pub fn validate_property_optionality_parity(
    rust: &PyClassDef,
    python: &PyClassDef,
    bevy_item: &BevyItem,
    bevy_path: Option<&Path>,
) -> Vec<Diagnostic> {
    let fields =
        bevy_path.and_then(|bevy_path| source_field_types(&bevy_item.full_path, Some(bevy_path)));
    let mut diagnostics = Vec::new();
    for property in &python.properties {
        if !property.has_getter {
            continue;
        }
        let Some(stub_type) = &property.property_type else {
            continue;
        };
        let rust_optional = rust
            .properties
            .iter()
            .find(|rust_property| rust_property.name == property.name)
            .and_then(|rust_property| rust_property.property_type.as_deref())
            .map(rust_return_optionality);
        // A dynamic getter return type decides nothing, not even by fallback.
        let rust_optional = match rust_optional {
            Some(None) => continue,
            Some(Some(optional)) => Some(optional),
            None => None,
        };
        let upstream_optional = fields
            .as_ref()
            .and_then(|fields| fields.get(&property.name))
            .map(|upstream| upstream.replace(' ', "").starts_with("Option<"));
        let optional = match (rust_optional, upstream_optional) {
            (Some(rust_optional), _) => rust_optional,
            (None, Some(upstream_optional)) => upstream_optional,
            (None, None) => continue,
        };
        let stub_optional = stub_none_accepting(stub_type);
        if optional == stub_optional {
            continue;
        }
        let expected = if optional {
            "accepts None (the getter returns Option<...>)"
        } else {
            "must not accept None (the getter never returns None)"
        };
        let mut diagnostic = Diagnostic::warning(
            DiagnosticCode::W036,
            format!(
                "property '{}.{}' is typed '{}', but it {}",
                rust.python_name, property.name, stub_type, expected
            ),
        );
        if let Some(location) = &property.getter_location {
            diagnostic = diagnostic.with_location(location.clone());
        }
        diagnostics.push(diagnostic);
    }
    diagnostics
}
