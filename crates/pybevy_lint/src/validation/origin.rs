//! Constructor origin resolution against the pinned Bevy declarations.
//!
//! The origin records what a Python constructor maps to upstream. It is
//! resolved from the pinned source declarations (struct fields, enum variant
//! shapes, associated constructor functions), never from defaults, arity, or
//! the spelling of the adapter's construction expression. A constructor whose
//! origin cannot be resolved reports `ConstructorOrigin::Unresolved` and the
//! policy check fails closed with E013 rather than silently passing.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use walkdir::WalkDir;

use super::method_rules;
use crate::{
    bevy_parser::types::{BevyCrate, BevyField, BevyItem, BevyItemKind, BevyVariantKind, SelfKind},
    comparison::pybevy_bevy_name,
    config::BevyConfig,
    model::{ConstructorOrigin, PyClassDef},
    output::{Diagnostic, DiagnosticCode},
};

pub fn audit_constructor_mappings(classes: &[PyClassDef], config: &BevyConfig) -> Vec<Diagnostic> {
    let mut names: Vec<_> = config
        .constructor_fields
        .keys()
        .chain(config.constructor_tuples.keys())
        .chain(config.constructor_adapters.keys())
        .chain(config.constructor_functions.keys())
        .collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .filter_map(|name| {
            let matches: Vec<_> = classes
                .iter()
                .filter(|class| constructor_mapping_key(class).as_deref() == Some(name.as_str()))
                .collect();
            if matches.len() == 1 && matches[0].constructor.is_some() {
                return None;
            }
            Some(
                Diagnostic::error(
                    DiagnosticCode::E012,
                    format!(
                        "constructor mapping '{name}' must match exactly one wrapper with a constructor"
                    ),
                )
                .with_note(
                    "mapping keys use '<public Python module>.<Rust wrapper>', such as 'pybevy.math.PyVec2'",
                ),
            )
        })
        .collect()
}

fn constructor_mapping_key(class: &PyClassDef) -> Option<String> {
    class
        .python_module_path
        .as_ref()
        .map(|module| format!("{module}.{}", class.rust_name))
}

/// Resolve the constructor origin for one PyBevy class against the parsed
/// pinned Bevy crates. Returns `None` when the class has no constructor or
/// the upstream type is not found in the parsed crates (the policy check
/// turns the unresolved case into E013).
pub fn resolve_constructor_origin(
    class: &PyClassDef,
    bevy_crates: &HashMap<String, BevyCrate>,
    config: &BevyConfig,
    bevy_path: Option<&Path>,
) -> Option<ConstructorOrigin> {
    let constructor = class.constructor.as_ref()?;
    resolve_method_origin(class, constructor, bevy_crates, config, bevy_path)
}

/// Resolve the origin for one constructor MethodDef against the pinned crates.
pub fn resolve_method_origin(
    class: &PyClassDef,
    constructor: &crate::model::MethodDef,
    bevy_crates: &HashMap<String, BevyCrate>,
    config: &BevyConfig,
    bevy_path: Option<&Path>,
) -> Option<ConstructorOrigin> {
    // The wrapper's module selects the owning crate (pybevy.camera ->
    // bevy_camera). This disambiguates same-name types that ship in several
    // crates (camera::Sphere vs math:Sphere) and keeps foreign matches from
    // hijacking the lookup.
    let preferred = match class.module_path.as_ref() {
        Some(module) => preferred_crate(config, Some(module.as_str())),
        None => None,
    };

    // Nested variant classes point at the parent enum through `extends` and
    // have no item of their own in the parsed crates; resolve them before
    // looking up the variant class's own bevy name.
    if let Some(parent_name) = class.extends.as_deref()
        && let Some((parent_crate, parent_item)) = find_item_preferred(
            bevy_crates,
            &variant_parent_bevy_name(class, parent_name, config),
            preferred.as_ref().cloned(),
        )
        && parent_item.kind == BevyItemKind::Enum
    {
        // Manual nested classes carry the Python variant name (`Perspective`),
        // not the rust-derived wrapper name (`ProjectionPerspective`).
        let variant_name = class.python_name.clone();
        if let Some(variant) = parent_item
            .variants
            .iter()
            .find(|variant| variant.name == variant_name)
        {
            return Some(resolve_variant_origin(
                variant,
                parent_crate,
                &parent_item.full_path,
                source_variant_field_order(
                    &parent_item.full_path,
                    parent_crate,
                    bevy_path,
                    &variant_name,
                )
                .unwrap_or_default(),
            ));
        }
    }

    let bevy_name = pybevy_bevy_name(class, config);
    // Resolve the item, falling back to the pinned source when cargo
    // public-api misses the type (private modules, generic templates).
    let found = find_or_scan(bevy_crates, &bevy_name, preferred, bevy_path, config)?;

    // Generated enum variant constructors are resolved from the enum's
    // variant shapes. The scanned form owns its item; keep a temporary so
    // the borrowed and owned paths share one classifier.
    match &found {
        FoundItem::Parsed {
            crate_name: crate_name2,
            item,
        } => resolve_with_item(
            class,
            constructor,
            crate_name2,
            item,
            bevy_crates,
            bevy_path,
            config,
        ),
        FoundItem::Scanned {
            crate_name: crate_name3,
            item,
        } => resolve_with_item(
            class,
            constructor,
            crate_name3.as_str(),
            item.as_ref(),
            bevy_crates,
            bevy_path,
            config,
        ),
    }
}

/// Generic templates split declarations: `CubicBezier<P>` holds the mapped
/// constructors while `CubicBezier` holds the public fields. Merge the
/// crate's `Name<...>` template methods into the base item for classification.
fn effective_item(
    item: &crate::bevy_parser::types::BevyItem,
    crate_name: &str,
    bevy_crates: &HashMap<String, BevyCrate>,
) -> crate::bevy_parser::types::BevyItem {
    let mut merged = item.clone();
    if !merged.methods.is_empty() {
        return merged;
    }
    let Some(crate_entry) = bevy_crates.get(crate_name) else {
        return merged;
    };
    for (key, template) in crate_entry.items.iter() {
        if key.starts_with(&format!("{}<", item.name)) && !template.methods.is_empty() {
            merged.methods = template.methods.clone();
            break;
        }
    }
    merged
}

/// Classify one wrapper constructor against a resolved upstream item.
fn resolve_with_item(
    class: &PyClassDef,
    constructor: &crate::model::MethodDef,
    crate_name: &str,
    item: &crate::bevy_parser::types::BevyItem,
    bevy_crates: &HashMap<String, BevyCrate>,
    bevy_path: Option<&Path>,
    config: &BevyConfig,
) -> Option<ConstructorOrigin> {
    let effective = effective_item(item, crate_name, bevy_crates);
    let item = &effective;
    let mapping_key = constructor_mapping_key(class);
    let field_mapping = mapping_key
        .as_ref()
        .and_then(|key| config.constructor_fields.get(key));
    let function_mapping = mapping_key
        .as_ref()
        .and_then(|key| config.constructor_functions.get(key));
    let tuple_mapping = mapping_key
        .as_ref()
        .and_then(|key| config.constructor_tuples.get(key));
    let adapter_mapping = mapping_key
        .as_ref()
        .and_then(|key| config.constructor_adapters.get(key));

    if (field_mapping.is_some()
        || function_mapping.is_some()
        || tuple_mapping.is_some()
        || adapter_mapping.is_some())
        && !unique_names(
            &constructor
                .parameters
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>(),
        )
    {
        return None;
    }

    let mapping_count = [
        field_mapping.is_some(),
        function_mapping.is_some(),
        tuple_mapping.is_some(),
        adapter_mapping.is_some(),
    ]
    .into_iter()
    .filter(|present| *present)
    .count();
    if mapping_count > 1 {
        return None;
    }

    if let Some(reason) = tuple_mapping {
        let is_tuple_struct = !item.fields.is_empty()
            && item
                .fields
                .iter()
                .all(|field| field.name.parse::<usize>().is_ok());
        if reason.trim().is_empty()
            || item.kind != BevyItemKind::Struct
            || !is_tuple_struct
            || constructor.parameters.is_empty()
        {
            return None;
        }
        return Some(ConstructorOrigin::TuplePayload {
            upstream_type: item.full_path.clone(),
        });
    }

    if let Some(reason) = adapter_mapping {
        if reason.trim().is_empty() {
            return None;
        }
        return Some(ConstructorOrigin::PythonAdapter {
            justification: reason.clone(),
        });
    }

    if let Some(mapping) = field_mapping {
        if mapping.reason().trim().is_empty() || item.kind != BevyItemKind::Struct {
            return None;
        }
        let mut field_order = declared_field_order(item, crate_name, bevy_path);
        if let Some(renames) = mapping.parameter_renames() {
            if renames.keys().any(|name| !field_order.contains(name)) {
                return None;
            }
            field_order = field_order
                .into_iter()
                .map(|name| renames.get(&name).unwrap_or(&name).clone())
                .collect();
        }
        if constructor.parameters.is_empty()
            || !unique_names(&field_order)
            || constructor
                .parameters
                .iter()
                .any(|parameter| !field_order.contains(&parameter.name))
        {
            return None;
        }
        return Some(ConstructorOrigin::FieldDerived {
            upstream_type: item.full_path.clone(),
            field_order,
        });
    }

    if let Some(mapping) = function_mapping {
        if mapping.reason.trim().is_empty() {
            return None;
        }
        let function = item.methods.iter().find(|method| {
            method.name == mapping.function && matches!(method.self_kind, SelfKind::None)
        })?;
        if !unique_names(&mapping.internal_parameters)
            || mapping.parameter_renames.keys().any(|name| {
                !function
                    .parameters
                    .iter()
                    .any(|parameter| parameter.name == *name)
            })
            || mapping.internal_parameters.iter().any(|name| {
                !function
                    .parameters
                    .iter()
                    .any(|parameter| parameter.name == *name)
                    || mapping.parameter_renames.contains_key(name)
            })
        {
            return None;
        }
        let first_internal = function
            .parameters
            .iter()
            .position(|parameter| mapping.internal_parameters.contains(&parameter.name));
        if first_internal.is_some_and(|start| {
            function.parameters[start..]
                .iter()
                .any(|parameter| !mapping.internal_parameters.contains(&parameter.name))
        }) {
            return None;
        }
        let function_params: Vec<String> = function
            .parameters
            .iter()
            .filter(|parameter| !mapping.internal_parameters.contains(&parameter.name))
            .map(|parameter| {
                mapping
                    .parameter_renames
                    .get(&parameter.name)
                    .unwrap_or(&parameter.name)
                    .clone()
            })
            .collect();
        let mut expected = function_params.clone();
        expected.extend(mapping.field_inputs.iter().cloned());
        if !unique_names(&expected) {
            return None;
        }
        if constructor.parameters.len() != expected.len()
            || constructor
                .parameters
                .iter()
                .any(|parameter| !expected.contains(&parameter.name))
        {
            return None;
        }
        if !mapping.field_inputs.is_empty() {
            let field_order = declared_field_order(item, crate_name, bevy_path);
            if item.kind != BevyItemKind::Struct
                || mapping
                    .field_inputs
                    .iter()
                    .any(|name| !field_order.contains(name))
            {
                return None;
            }
            return Some(ConstructorOrigin::Mixed {
                upstream_type: item.full_path.clone(),
                function: mapping.function.clone(),
                function_params,
                tail_fields: field_order
                    .into_iter()
                    .filter(|name| mapping.field_inputs.contains(name))
                    .collect(),
            });
        }
        return Some(ConstructorOrigin::BevyNew {
            upstream_type: item.full_path.clone(),
            function: mapping.function.clone(),
            function_params,
        });
    }

    if class.is_enum {
        return resolve_enum_constructor(class, constructor, item, crate_name);
    }

    match item.kind {
        // Declared pyenum bases (Falloff, WindowPosition, Color) keep the
        // enum-base contract: parameterless rejectors are ZeroArg, parameter-
        // ized bases are NonConstructible (E016).
        BevyItemKind::Enum
            if matches!(
                class.macro_info,
                Some(crate::model::MacroInfo::BevyEnum { .. })
            ) =>
        {
            resolve_enum_constructor(class, constructor, item, crate_name)
        }
        // A non-pyenum class mapping onto an upstream enum is a snapshot /
        // newtype mirror (pyvalue, pywrap, pycomponent over an enum payload):
        // parameterless mirrors are default-only; payload mirrors keep the
        // positional-or-keyword tuple contract.
        BevyItemKind::Enum => {
            if constructor.parameters.is_empty() {
                Some(ConstructorOrigin::ZeroArg {
                    upstream_type: item.full_path.clone(),
                })
            } else {
                Some(ConstructorOrigin::TuplePayload {
                    upstream_type: item.full_path.clone(),
                })
            }
        }
        BevyItemKind::Struct => {
            resolve_struct_constructor(constructor, item, crate_name, bevy_crates, bevy_path)
        }
        // Bevy traits (Component, Resource, Message, Plugin) are marker
        // surfaces; their Python wrappers are subclassing/rejection bases.
        BevyItemKind::Trait => {
            if is_varargs_rejection_base(constructor) {
                Some(ConstructorOrigin::PythonAdapter {
                    justification: "variadic *_args/**_kwargs trait-marker subclassing/rejection base; constructor raises"
                        .to_string(),
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

fn unique_names(names: &[String]) -> bool {
    names
        .iter()
        .enumerate()
        .all(|(index, name)| !name.trim().is_empty() && !names[..index].contains(name))
}

fn declared_field_order(
    item: &BevyItem,
    crate_name: &str,
    bevy_path: Option<&Path>,
) -> Vec<String> {
    source_field_order(&item.full_path, crate_name, bevy_path).unwrap_or_else(|| {
        item.fields
            .iter()
            .filter(|field| field.is_public)
            .map(|field| field.name.clone())
            .collect()
    })
}

/// True for `#[new] fn new(*_args, **_kwargs)` subclassing/rejection bases.
fn is_varargs_rejection_base(constructor: &crate::model::MethodDef) -> bool {
    constructor.parameters.iter().any(|p| {
        (p.name == "_args" || p.name == "args")
            && p.param_type
                .as_ref()
                .is_some_and(|t| t.contains("PyTuple") || t.contains("Tuple"))
    }) && constructor.parameters.iter().any(|p| {
        (p.name == "_kwargs" || p.name == "kwargs")
            && p.param_type
                .as_ref()
                .is_some_and(|t| t.contains("PyDict") || t.contains("Dict"))
    })
}

/// Resolve a struct constructor: a mapped `new()`/associated constructor
/// function wins over the struct's public fields (the design record's
/// classification rule; storage-macro declaration is the primary identity).
fn resolve_struct_constructor(
    constructor: &crate::model::MethodDef,
    item: &crate::bevy_parser::types::BevyItem,
    crate_name: &str,
    bevy_crates: &HashMap<String, BevyCrate>,
    bevy_path: Option<&Path>,
) -> Option<ConstructorOrigin> {
    let bevy_type = item.full_path.clone();

    // `#[new] fn new(*_args, **_kwargs)` is a Python subclassing/rejection
    // base (Message, Component, Resource, Plugin, Asset): the constructor
    // raises and the parameters are variadic plumbing, not upstream inputs.
    // Classify PythonAdapter so the kind/order policies stay silent and the
    // reviewed config exception (or the varargs nature itself) is the audit
    // record — never a fake BevyNew mapping.
    if is_varargs_rejection_base(constructor) {
        return Some(ConstructorOrigin::PythonAdapter {
            justification:
                "variadic *_args/**_kwargs subclassing/rejection base; constructor raises"
                    .to_string(),
        });
    }

    // Zero-argument constructors on default-only types.
    if constructor.parameters.is_empty() {
        return Some(ConstructorOrigin::ZeroArg {
            upstream_type: bevy_type,
        });
    }

    let names: Vec<String> = constructor
        .parameters
        .iter()
        .map(|parameter| parameter.name.clone())
        .collect();
    let public_fields: Vec<_> = item.fields.iter().filter(|field| field.is_public).collect();
    let mut field_names: Vec<String> = public_fields
        .iter()
        .map(|field| field.name.clone())
        .collect();
    // Public-api shells drop struct fields; derive them from the pinned
    // source when available so Mixed tail and FieldDerived checks can run
    // against the actual declaration order (P2#4).
    if field_names.is_empty()
        && let Some(order) = source_field_order(&item.full_path, crate_name, bevy_path)
    {
        field_names = order;
    }
    let is_tuple_struct = item
        .fields
        .iter()
        .any(|field| field.name.parse::<usize>().is_ok());
    let mapped_new = item.methods.iter().find(|method| {
        method.name == "new"
            && matches!(method.self_kind, SelfKind::None)
            && !method.parameters.is_empty()
    });
    let function_params: Option<Vec<String>> = mapped_new.map(|method| {
        method
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect()
    });

    // Tuple structs: a mapped `new` over the tuple stays positional (the
    // flattened mirror keeps positional-or-keyword callers); without one the
    // payload is a genuine positional tuple.
    if is_tuple_struct {
        return Some(ConstructorOrigin::BevyNew {
            upstream_type: bevy_type,
            function: "new".to_string(),
            function_params: function_params.unwrap_or_default(),
        });
    }

    // A field subset need not expose every input of an associated constructor.
    let field_order: Option<Vec<String>> =
        source_field_order(&item.full_path, crate_name, bevy_path);
    // Strict subset only: an exact match with the mapped `new` stays BevyNew
    // (ScatteringMedium::new, TextBounds::new mirror their upstream functions
    // with positional-or-keyword inputs).
    if !field_names.is_empty()
        && names.iter().all(|name| field_names.contains(name))
        && function_params
            .as_ref()
            .is_none_or(|params| params.len() > names.len())
    {
        return Some(ConstructorOrigin::FieldDerived {
            upstream_type: bevy_type,
            field_order: field_order.unwrap_or(field_names),
        });
    }

    // Associated constructor: the wrapper mirrors `new` when its parameter
    // names equal the mapped function's in order. A name-preserving prefix
    // (renames allowed: EditableText::new(initial_text) -> `text`)
    // plus keyword-only additional fields is Mixed.
    if let Some(function_params) = &function_params {
        if function_params == &names {
            return Some(ConstructorOrigin::BevyNew {
                upstream_type: bevy_type,
                function: "new".to_string(),
                function_params: function_params.clone(),
            });
        }
        // The mapped `new` is the positional prefix even when its parameter
        // names coincide with field names (ColorStop::new(color, point)):
        // renameable by position, followed by keyword-only field inputs. Tail
        // fields are the upstream field declaration order, NOT the wrapper's
        // current order (a reversed Torus must be flagged).
        let wrapper_tail: Vec<String> = names
            .iter()
            .skip(function_params.len())
            .map(|name| name.to_string())
            .collect();
        let tail: Vec<String> = field_order
            .as_ref()
            .map(|order| {
                order
                    .iter()
                    .filter(|name| {
                        let name_str = name.as_str();
                        wrapper_tail.iter().any(|w| w.as_str() == name_str)
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or(wrapper_tail);
        if function_params.len() < names.len()
            && !tail.is_empty()
            && tail.iter().all(|name| field_names.contains(name))
        {
            return Some(ConstructorOrigin::Mixed {
                upstream_type: bevy_type,
                function: "new".to_string(),
                function_params: function_params.clone(),
                tail_fields: tail,
            });
        }
    }

    // Field-derived: wrapper names equal the public fields exactly.
    if !field_names.is_empty() && names.iter().all(|name| field_names.contains(name)) {
        return Some(ConstructorOrigin::FieldDerived {
            upstream_type: bevy_type,
            field_order: field_order.unwrap_or(field_names),
        });
    }

    // Associated factory (`from_*`) with a flattened parameter list:
    // positional retained, flagged for individual review.
    if let Some(factory) = item.methods.iter().find(|method| {
        method.name.starts_with("from_")
            && matches!(method.self_kind, SelfKind::None)
            && !method.parameters.is_empty()
    }) {
        return Some(ConstructorOrigin::Factory {
            upstream_type: bevy_type,
            function: factory.name.clone(),
        });
    }

    // A mapped `new` whose parameters do not map cleanly onto the wrapper:
    // keep the mapped-function contract (defaults fill the remaining inputs).
    if let Some(function_params) = &function_params {
        return Some(ConstructorOrigin::BevyNew {
            upstream_type: bevy_type,
            function: "new".to_string(),
            function_params: function_params.clone(),
        });
    }

    // The struct declaration itself is not in the parsed crates (e.g. the
    // type lives in a directly wrapped non-Bevy package): unresolved.
    let _ = crate_name;
    let _ = bevy_crates;
    None
}

/// Resolve an enum constructor: the base is non-constructible (or
/// default-only); variant kinds follow the resolved Bevy variant shape.
fn resolve_enum_constructor(
    class: &PyClassDef,
    constructor: &crate::model::MethodDef,
    item: &crate::bevy_parser::types::BevyItem,
    _crate_name: &str,
) -> Option<ConstructorOrigin> {
    let bevy_type = item.full_path.clone();
    let _ = class;

    // A pyenum base reached with a `#[new]` is only reached when a
    // constructor exists. A parameterless one is a default constructor
    // (ZeroArg); a parameterized one is a regained initializer over a
    // non-constructible base (E016).
    if constructor.parameters.is_empty() {
        return Some(ConstructorOrigin::ZeroArg {
            upstream_type: bevy_type,
        });
    }

    Some(ConstructorOrigin::NonConstructible {
        upstream_type: bevy_type,
    })
}

/// Resolve one enum variant's constructor origin from its resolved Bevy
/// shape: named fields are keyword-only (FieldDerived-style contract);
/// tuple payloads stay positional-or-keyword; `#[py_bevy(tuple)]` named
/// adapters keep the positional payload contract.
fn resolve_variant_origin(
    variant: &crate::bevy_parser::types::BevyEnumVariant,
    _crate_name: &str,
    enum_path: &str,
    field_order: Vec<String>,
) -> ConstructorOrigin {
    match &variant.kind {
        BevyVariantKind::Unit => ConstructorOrigin::ZeroArg {
            upstream_type: enum_path.to_string(),
        },
        BevyVariantKind::Tuple(_) => ConstructorOrigin::TuplePayload {
            upstream_type: enum_path.to_string(),
        },
        BevyVariantKind::Struct(fields) => ConstructorOrigin::EnumVariant {
            upstream_type: enum_path.to_string(),
            variant: variant.name.clone(),
            field_order: if field_order.is_empty() {
                fields.iter().map(|(name, _)| name.clone()).collect()
            } else {
                field_order
            },
        },
    }
}

/// Bevy type name for a nested variant class's parent enum.
fn variant_parent_bevy_name(
    _class: &PyClassDef,
    parent_rust_name: &str,
    config: &BevyConfig,
) -> String {
    let parent = PyClassDef {
        rust_name: parent_rust_name.to_string(),
        ..Default::default()
    };
    pybevy_bevy_name(&parent, config)
}

/// Crate whose items this wrapper's module owns (pybevy.camera -> bevy_camera).
/// Used to disambiguate same-name items that ship in several crates.
/// Crate whose items this wrapper's module owns (pybevy.camera -> bevy_camera).
/// Used to disambiguate same-name items that ship in several crates.
fn preferred_crate(config: &BevyConfig, module_path: Option<&str>) -> Option<String> {
    let module = module_path?;
    let segments: Vec<&str> = module.split('.').collect();
    // Render the configured segment's mapped crate (pybevy.ecs -> "bevy_ecs").
    for (key, crate_name) in config.crate_mappings.iter() {
        let key_str = key.as_str();
        if segments.iter().any(|segment| segment == &key_str) {
            return Some(crate_name.clone());
        }
    }
    None
}

/// A resolved upstream item: either borrowed from the parsed crate map or
/// synthesized from the pinned source when cargo public-api misses the type.
enum FoundItem<'a> {
    Parsed {
        crate_name: &'a str,
        item: &'a crate::bevy_parser::types::BevyItem,
    },
    // Boxed: scanned items are owned and much larger than borrowed refs.
    Scanned {
        crate_name: String,
        item: Box<crate::bevy_parser::types::BevyItem>,
    },
}

/// Synthetic upstream items for wrappers whose audited declaration lives
/// outside the pinned registry (std::ops::Range<f32>, wgpu-types values).
/// These resolve so the reviewed FIELD contract is ENFORCED by the policy —
/// never masked behind an E013 exception.
fn synthetic_upstream(preferred: &str, short_name: &str) -> Option<BevyItem> {
    let module = preferred.strip_prefix("bevy_")?;
    let fields = match (module, short_name) {
        ("math", "Range") => vec!["start".to_string(), "end".to_string()],
        ("render", "Extent3d") => vec![
            "width".to_string(),
            "height".to_string(),
            "depth_or_array_layers".to_string(),
        ],
        ("render", "TextureUsages") => Vec::new(),
        _ => return None,
    };
    Some(bevy_item_with_fields(
        preferred,
        short_name,
        BevyItemKind::Struct,
        fields
            .into_iter()
            .map(|name| BevyField {
                name,
                field_type: "".to_string(),
                is_public: true,
            })
            .collect(),
    ))
}

/// Public helper for the audited non-registry upstream.
fn present_synthetic_item(crate_name: &str, short_name: &str) -> Option<BevyItem> {
    synthetic_upstream(crate_name, short_name)
}

/// Find the wrapper's upstream item, preferring its own crate, then the
/// global map, then a pinned-source scan of the owning crate.
fn find_or_scan<'a>(
    bevy_crates: &'a HashMap<String, BevyCrate>,
    bevy_name: &str,
    preferred: Option<String>,
    bevy_path: Option<&Path>,
    config: &BevyConfig,
) -> Option<FoundItem<'a>> {
    let short_name = bevy_name.rsplit("::").next().unwrap_or(bevy_name);
    if let Some(preferred) = preferred.as_ref() {
        let item = bevy_crates.get(preferred)?.items.get(short_name);
        // cargo public-api hides newtype payload fields; a field-less,
        // variant-less, method-less shell cannot be classified, so prefer the
        // source. A mapped `new()` is still authoritative even when fields
        // are missing (AxisSettings::new keeps its own parameter order).
        let is_empty_shell = item.is_some_and(|item| {
            item.fields.is_empty() && item.variants.is_empty() && item.methods.is_empty()
        });
        // cargo public-api sometimes emits an empty shell for newtypes /
        // reflected structs; the pinned source has the real declaration.
        if let Some(bevy_path) = bevy_path
            && is_empty_shell
            && let Some(scanned) = source_scan_item(preferred, short_name, bevy_path)
        {
            return Some(FoundItem::Scanned {
                crate_name: preferred.clone(),
                item: Box::new(scanned),
            });
        }
        if let Some(item) = item.filter(|item| !item.fields.is_empty() || !item.variants.is_empty())
            && let Some((crate_key, _)) = bevy_crates
                .iter()
                .find(|(name, _)| name.to_string() == *preferred)
        {
            return Some(FoundItem::Parsed {
                crate_name: crate_key.as_str(),
                item,
            });
        }
        // Parsed crates lack the name: scan the owning crate's pinned source.
        if let Some(bevy_path) = bevy_path
            && let Some(item) = source_scan_item(
                preferred,
                bevy_name.rsplit("::").next().unwrap_or(bevy_name),
                bevy_path,
            )
        {
            return Some(FoundItem::Scanned {
                crate_name: preferred.clone(),
                item: Box::new(item),
            });
        }
    }
    if let Some((crate_name2, item)) = find_item(bevy_crates, bevy_name) {
        // Methods do not restore declaration shape omitted by cargo public-api.
        let needs_source_shape = item.fields.is_empty() && item.variants.is_empty();
        if needs_source_shape
            && let Some(bevy_path) = bevy_path
            && let Some(scanned) = source_scan_item(crate_name2, short_name, bevy_path)
        {
            return Some(FoundItem::Scanned {
                crate_name: crate_name2.to_string(),
                item: Box::new(scanned),
            });
        }
        if needs_source_shape && let Some(item) = present_synthetic_item(crate_name2, short_name) {
            return Some(FoundItem::Scanned {
                crate_name: crate_name2.to_string(),
                item: Box::new(item),
            });
        }
        return Some(FoundItem::Parsed {
            crate_name: crate_name2,
            item,
        });
    }
    // Global miss: scan every parsed crate's pinned source (the wrapper's
    // module may live in a different crate than its upstream type, e.g.
    // pybevy.text::Text2d comes from bevy_sprite).
    if let Some(bevy_path) = bevy_path {
        // Scan the parsed crates plus their re-export source crates
        // (bevy_sprite_render, glam) where the upstream declaration lives.
        let mut scan_crates: Vec<String> = bevy_crates.keys().cloned().collect::<Vec<String>>();
        for sources in config.crate_type_sources.values() {
            for source in sources {
                if !scan_crates.contains(source) {
                    scan_crates.push(source.clone());
                }
            }
        }
        let short = bevy_name.rsplit("::").next().unwrap_or(bevy_name);
        for crate_name2 in scan_crates {
            if let Some(item) = source_scan_item(crate_name2.as_str(), short, bevy_path) {
                return Some(FoundItem::Scanned {
                    crate_name: crate_name2,
                    item: Box::new(item),
                });
            }
        }
        // Audited non-registry upstream (std::ops::Range<f32>, wgpu)
        if let Some(crate_name) = preferred
            && let Some(item) = synthetic_upstream(crate_name.as_str(), short)
        {
            return Some(FoundItem::Scanned {
                crate_name: crate_name.clone(),
                item: Box::new(item),
            });
        }
    }
    None
}

/// Find one item by short name, preferring the wrapper's own crate.
fn find_item_preferred<'a>(
    bevy_crates: &'a HashMap<String, BevyCrate>,
    bevy_name: &str,
    preferred: Option<String>,
) -> Option<(&'a str, &'a crate::bevy_parser::types::BevyItem)> {
    let short_name = bevy_name.rsplit("::").next().unwrap_or(bevy_name);
    if let Some(preferred) = preferred.as_ref()
        && let Some(item) = bevy_crates.get(preferred)?.items.get(short_name)
        && let Some((crate_key, _)) = bevy_crates
            .iter()
            .find(|(name, _)| name.to_string() == *preferred)
    {
        // Borrow the crate-name key from the map (the item's `module` field
        // is the full module path, not the owning crate).
        return Some((crate_key.as_str(), item));
    }
    find_item(bevy_crates, bevy_name)
}

/// Find one item by short name across the parsed crates.
/// Returns (crate name, item). Ambiguity resolves by configured crate order;
/// same-name types across crates must be disambiguated by the caller using
/// full-path identity.
fn find_item<'a>(
    bevy_crates: &'a HashMap<String, BevyCrate>,
    bevy_name: &str,
) -> Option<(&'a str, &'a crate::bevy_parser::types::BevyItem)> {
    let short_name = bevy_name.rsplit("::").next().unwrap_or(bevy_name);
    let mut matches: Vec<(&str, &crate::bevy_parser::types::BevyItem)> = Vec::new();
    for (crate_name, bevy_crate) in bevy_crates {
        if let Some(item) = bevy_crate.items.get(short_name) {
            matches.push((crate_name.as_str(), item));
        }
    }
    match matches.len() {
        1 => Some(matches[0]),
        0 => None,
        // Same short name in several crates: prefer the item whose short
        // name is unique in its crate's public API (registry-scoped
        // identity); when the type is genuinely re-exported next to a real
        // declaration (bevy_pbr::AssetId is a type alias), prefer the Enum
        // declaration for variant resolution.
        _ => {
            let unique: Vec<_> = matches
                .iter()
                .filter(|(_, item)| item.name == short_name)
                .collect();
            if unique.len() == 1 {
                return Some(*unique[0]);
            }
            let enums: Vec<_> = unique
                .iter()
                .filter(|(_, item)| item.kind == BevyItemKind::Enum)
                .collect();
            if enums.len() == 1 {
                return Some((enums[0].0, enums[0].1));
            }
            // Prefer the candidate declaring a mapped constructor or variants
            // (bevy_math::Sphere::new exists; bevy_camera::Sphere does not).
            let substantive: Vec<_> = unique
                .iter()
                .filter(|(_, item)| {
                    // Only a mapped constructor or variants disambiguate;
                    // accessor methods are noise (bevy_camera::Sphere has
                    // getters but no constructor).
                    !item.variants.is_empty()
                        || item.methods.iter().any(|method| {
                            method.name == "new"
                                && matches!(method.self_kind, SelfKind::None)
                                && !method.parameters.is_empty()
                        })
                })
                .collect();
            if substantive.len() == 1 {
                return Some((substantive[0].0, substantive[0].1));
            }
            None
        }
    }
}

/// MethodRules helper: validate one constructor's parameter roles against the
/// resolved origin. Public so the validation pipeline can call it with the
/// resolved origin.
pub fn check_constructor_policy(
    origin: &ConstructorOrigin,
    class: &PyClassDef,
) -> Vec<crate::output::Diagnostic> {
    method_rules::check_constructor_policy(origin, class)
}

/// Read the struct's field declaration order from the pinned source file.
/// `cargo public-api` output is alphabetically sorted, so declaration order
/// must come from the Rust source text. Returns None when the pinned source
/// is unavailable or the declaration file cannot be located.
fn source_field_order(
    full_path: &str,
    crate_name: &str,
    bevy_path: Option<&Path>,
) -> Option<Vec<String>> {
    let bevy_path = bevy_path?;
    let type_name = full_path.rsplit("::").next()?;
    let source = find_type_source_file(full_path, crate_name, bevy_path)?;
    read_any_field_order(&source, type_name)
}

/// Locate the pinned-source file that declares `full_path`'s type.
fn find_type_source_file(full_path: &str, crate_name: &str, bevy_path: &Path) -> Option<PathBuf> {
    let type_name = full_path.rsplit("::").next()?;
    // Drop the crate segment and the trailing type segment; the rest is the
    // module path.
    let mut module_segments: Vec<&str> = full_path
        .split("::")
        .skip(1)
        .filter(|segment| *segment != crate_name)
        .collect();
    if module_segments.last() == Some(&type_name) {
        module_segments.pop();
    }

    // The declaration may live in the type's own crate (the merged
    // bevy_sprite_render items carry "bevy_sprite_render::..." paths even
    // when wrapped from pybevy.mesh / pybevy.sprite).
    let source_crate = full_path
        .split("::")
        .next()
        .unwrap_or(crate_name)
        .to_string();
    let crate_src = bevy_path
        .join("crates")
        .join(source_crate.as_str())
        .join("src");
    let mut candidates: Vec<PathBuf> = Vec::new();
    let module_path = module_segments.join("/");
    candidates.push(crate_src.join(format!("{module_path}.rs")));
    candidates.push(crate_src.join(format!("{module_path}/mod.rs")));
    // Module segments may be re-exports; fall back to the leaf name file
    // beside each candidate parent directory.
    let leaf = module_segments.last().copied().unwrap_or(type_name);
    candidates.push(crate_src.join(format!("{leaf}.rs")));
    candidates.push(crate_src.join(format!("{leaf}/mod.rs")));
    for candidate in candidates {
        if candidate.exists() && text_declares_type(&candidate, type_name) {
            return Some(candidate);
        }
    }

    // Type-named files first, then any file whose source declares the type
    // (Bevy often names the file after the module, not the type — e.g.
    // bevy_ui's Node lives in ui_node.rs).
    let wanted = format!("{type_name}.rs");
    let mut fallback: Vec<PathBuf> = Vec::new();
    for entry in WalkDir::new(crate_src)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(&wanted)
        {
            return Some(path.to_path_buf());
        }
        fallback.push(path.to_path_buf());
    }
    fallback
        .into_iter()
        .find(|path| text_declares_type(path, type_name))
}

/// True when the file text contains an exact `pub struct/enum <Type>` or a
/// `#[derive(...)]`-adjacent enum declaration for the type.
fn text_declares_type(path: &Path, type_name: &str) -> bool {
    let Some(text) = std::fs::read_to_string(path).ok() else {
        return false;
    };
    let struct_marker = format!("pub struct {type_name}");
    let enum_marker = format!("pub enum {type_name}");
    has_exact_decl(&text, &struct_marker) || has_exact_decl(&text, &enum_marker)
}

/// Extract `pub struct <Type> { pub field: Type, ... }` field names in
/// declaration order, capturing private (#[reflect]) struct fields too — the
/// field names and declaration order are the upstream identity even when the
/// fields are not `pub`.
fn read_any_field_order(path: &Path, type_name: &str) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut start = None;
    let mut search_from = 0usize;
    let struct_marker = format!("pub struct {type_name}");
    while let Some(candidate) = text[search_from..].find(&struct_marker) {
        let absolute = search_from + candidate;
        let next = text[absolute + struct_marker.len()..].chars().next();
        let boundary = next.is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        if boundary {
            start = Some(absolute);
            break;
        }
        search_from = absolute + struct_marker.len();
    }
    let start = start?;
    let body_start = text[start..].find('{')? + start;
    let mut depth = 0usize;
    let body_end = text[body_start..]
        .char_indices()
        .find(|(_, c)| {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return true;
                    }
                }
                _ => {}
            }
            false
        })
        .map(|(index, _)| body_start + index)?;

    let body = &text[body_start + 1..body_end];
    let mut order = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.is_empty() || trimmed.starts_with("#[") {
            continue;
        }
        let candidate = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
        if candidate.starts_with("(") {
            continue;
        }
        let name: String = candidate
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() && candidate.find(":").is_some() {
            order.push(name);
        }
    }
    if order.is_empty() { None } else { Some(order) }
}

/// Enum variant payload field names in declaration order from the pinned
/// source. cargo public-api emits variant fields alphabetically; the true
/// order lives in the enum body's `Variant { field: Ty, ... }` block.
pub fn source_variant_field_order(
    full_path: &str,
    crate_name: &str,
    bevy_path: Option<&Path>,
    variant_name: &str,
) -> Option<Vec<String>> {
    let bevy_path = bevy_path?;
    let source = find_type_source_file(full_path, crate_name, bevy_path)?;
    let text = std::fs::read_to_string(&source).ok()?;
    let type_name = full_path.rsplit("::").next()?.split('<').next()?;
    let file = syn::parse_file(&text).ok()?;
    variant_fields_in_items(&file.items, type_name, variant_name)
}

fn variant_fields_in_items(
    items: &[syn::Item],
    type_name: &str,
    variant_name: &str,
) -> Option<Vec<String>> {
    items.iter().find_map(|item| match item {
        syn::Item::Enum(item) if item.ident == type_name => {
            let variant = item
                .variants
                .iter()
                .find(|variant| variant.ident == variant_name)?;
            let syn::Fields::Named(fields) = &variant.fields else {
                return None;
            };
            Some(
                fields
                    .named
                    .iter()
                    .filter_map(|field| {
                        field
                            .ident
                            .as_ref()
                            .map(|name| name.to_string().trim_start_matches("r#").to_string())
                    })
                    .collect(),
            )
        }
        syn::Item::Mod(module) => module
            .content
            .as_ref()
            .and_then(|(_, items)| variant_fields_in_items(items, type_name, variant_name)),
        _ => None,
    })
}

/// Build an item shell for a source-scanned declaration.
fn empty_bevy_item(crate_name: &str, short_name: &str, kind: BevyItemKind) -> BevyItem {
    BevyItem::new(format!("{}::{short_name}", crate_name), kind)
}

fn bevy_item_with_fields(
    crate_name: &str,
    short_name: &str,
    kind: BevyItemKind,
    fields: Vec<BevyField>,
) -> BevyItem {
    let mut item = empty_bevy_item(crate_name, short_name, kind);
    item.fields = fields;
    item
}

/// Fall back to the pinned source when cargo public-api misses a type
/// (private modules, newtype tuple structs, generic templates). Synthesizes
/// the item with declaration-order fields so classification can proceed.
fn source_scan_item(crate_name: &str, short_name: &str, bevy_path: &Path) -> Option<BevyItem> {
    let crate_src = bevy_path.join("crates").join(crate_name).join("src");
    let struct_marker = format!("pub struct {short_name}");
    let enum_marker = format!("pub enum {short_name}");
    for entry in WalkDir::new(crate_src)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        // Enum declaration: no fields (variants are not needed for the
        // upstream identity here).
        if has_exact_decl(&text, &enum_marker) {
            return Some(empty_bevy_item(crate_name, short_name, BevyItemKind::Enum));
        }
        if !has_exact_decl(&text, &struct_marker) {
            continue;
        }
        // Newtype `pub struct X(pub T, ...)`: a positional tuple payload.
        if let Some(after) = marker_after(&text, &struct_marker) {
            // Skip an optional `<...>` generic suffix before the payload.
            let after_generics = skip_generics(after);
            if after_generics.starts_with("(") {
                let mut item = bevy_item_with_fields(
                    crate_name,
                    short_name,
                    BevyItemKind::Struct,
                    vec![BevyField {
                        name: "0".to_string(),
                        field_type: "".to_string(),
                        is_public: true,
                    }],
                );
                if let Some(method) = read_impl_new(path, short_name) {
                    item.methods.push(method);
                }
                return Some(item);
            }
            // Braced struct: use the source declaration order. Private
            // #[reflect] structs expose their fields via getters, so capture
            // non-pub fields too (e.g. bevy_asset::AssetPath).
            if let Some(names) = read_any_field_order(path, short_name) {
                let mut item = bevy_item_with_fields(
                    crate_name,
                    short_name,
                    BevyItemKind::Struct,
                    names
                        .iter()
                        .map(|name| BevyField {
                            name: name.clone(),
                            field_type: "".to_string(),
                            is_public: true,
                        })
                        .collect(),
                );
                // A source-declared `new()` is authoritative for the
                // BevyNew/Mixed classification (AxisSettings::new, Timer...);
                // public-api shells drop methods, so reattach them here.
                if let Some(method) = read_impl_new(path, short_name) {
                    item.methods.push(method);
                }
                return Some(item);
            }
        }
    }
    None
}

/// Read the parameter list of `impl <Type> { ... pub fn new(..) .. }` in the
/// pinned source. Public-api shells drop methods, but a source-declared `new`
/// is authoritative for BevyNew/Mixed classification.
fn read_impl_new(path: &Path, type_name: &str) -> Option<crate::bevy_parser::types::BevyMethod> {
    let text = std::fs::read_to_string(path).ok()?;
    let impl_marker = format!("impl {type_name}");
    let mut search_from = 0usize;
    while let Some(cand) = text[search_from..].find(&impl_marker) {
        let absolute = search_from + cand;
        let next = text[absolute + impl_marker.len()..].chars().next();
        let boundary = next.is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        if !boundary {
            search_from = absolute + impl_marker.len();
            continue;
        }
        // Find the impl block's opening brace and matching close.
        let Some(body_start) = text[absolute..].find('{') else {
            search_from = absolute + impl_marker.len();
            continue;
        };
        let body_start = absolute + body_start;
        let mut depth = 0usize;
        let Some(body_end_rel) = text[body_start..]
            .char_indices()
            .find(|(_, c)| {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return true;
                        }
                    }
                    _ => {}
                }
                false
            })
            .map(|(index, _)| index)
        else {
            search_from = absolute + impl_marker.len();
            continue;
        };
        let body = &text[body_start + 1..body_start + body_end_rel];
        let Some((new_offset, _)) = body.lines().enumerate().find(|(_, line)| {
            let t = line.trim();
            t.contains("fn new(") && t.starts_with("pub ")
                || t.starts_with("const")
                || t.starts_with("fn")
        }) else {
            search_from = absolute + impl_marker.len();
            continue;
        };
        let line_start = body
            .lines()
            .take(new_offset)
            .fold(0usize, |acc, l| acc + l.len() + 1);
        let Some(open_rel) = body[line_start..].find('(') else {
            search_from = absolute + impl_marker.len();
            continue;
        };
        let open_abs = line_start + open_rel;
        let mut pdepth = 0usize;
        let Some(close_rel) = body[open_abs..]
            .char_indices()
            .find(|(_, c)| {
                match c {
                    '(' => pdepth += 1,
                    ')' => {
                        pdepth -= 1;
                        if pdepth == 0 {
                            return true;
                        }
                    }
                    _ => {}
                }
                false
            })
            .map(|(index, _)| index)
        else {
            break;
        };
        let params_decl = &body[open_abs + 1..open_abs + close_rel];
        let params: Vec<crate::bevy_parser::types::BevyParameter> =
            split_source_params(params_decl)
                .into_iter()
                .filter_map(|seg| {
                    let seg = seg.trim();
                    let (name, _) = seg.split_once(':')?;
                    let name = name
                        .trim()
                        .strip_prefix("mut ")
                        .unwrap_or(name.trim())
                        .trim();
                    if name.is_empty() || name.contains("impl ") || name == "self" {
                        return None;
                    }
                    Some(crate::bevy_parser::types::BevyParameter {
                        name: name.to_string(),
                        param_type: "".to_string(),
                    })
                })
                .collect();
        if !params.is_empty() {
            return Some(crate::bevy_parser::types::BevyMethod {
                name: "new".to_string(),
                signature: "pub fn new(...)".to_string(),
                parameters: params,
                return_type: Some("Self".to_string()),
                self_kind: crate::bevy_parser::types::SelfKind::None,
                is_const: false,
                is_unsafe: false,
                is_async: false,
                from_trait: None,
            });
        }
        break;
    }
    None
}

fn split_source_params(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut angle = 0usize;
    let mut paren = 0usize;
    let mut bracket = 0usize;
    let mut brace = 0usize;
    let mut previous = None;
    for (index, character) in input.char_indices() {
        match character {
            '<' => angle += 1,
            '>' if previous != Some('-') => angle = angle.saturating_sub(1),
            '(' => paren += 1,
            ')' => paren = paren.saturating_sub(1),
            '[' => bracket += 1,
            ']' => bracket = bracket.saturating_sub(1),
            '{' => brace += 1,
            '}' => brace = brace.saturating_sub(1),
            ',' if angle == 0 && paren == 0 && bracket == 0 && brace == 0 => {
                parts.push(&input[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
        previous = Some(character);
    }
    parts.push(&input[start..]);
    parts
}

/// Skip a `<...>` generic suffix immediately after a declaration name.
fn skip_generics(text: &str) -> &str {
    if !text.starts_with('<') {
        return text;
    }
    let mut depth = 0usize;
    for (index, c) in text.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => {
                depth = depth.saturating_sub(1);
                if depth == 0 && index < text.len() {
                    return &text[index + 1..];
                }
            }
            _ => {}
        }
    }
    text
}

/// True when `text` contains an exact `pub struct/enum <Name>` declaration.
fn has_exact_decl(text: &str, marker: &str) -> bool {
    marker_after(text, marker).is_some()
}

/// Text immediately after an exact `pub struct/enum <Name>` marker.
fn marker_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let mut search_from = 0usize;
    while let Some(candidate) = text[search_from..].find(marker) {
        let absolute = search_from + candidate;
        let next = text[absolute + marker.len()..].chars().next();
        if next.is_none_or(|c| !(c.is_alphanumeric() || c == '_')) {
            return Some(&text[absolute + marker.len()..]);
        }
        search_from = absolute + marker.len();
    }
    None
}
