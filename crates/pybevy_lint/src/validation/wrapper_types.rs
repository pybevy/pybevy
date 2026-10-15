use std::collections::{HashMap, HashSet};

use crate::{
    bevy_parser::types::{BevyCrate, BevyItem, BevyItemKind, BevyVariantKind},
    config::BevyConfig,
    model::{ConstructorOrigin, EnumVariantKind, PyClassDef, SourceLocation},
    output::{Diagnostic, DiagnosticCode},
};

const BUILTIN_NAMES: &[&str] = &[
    "tuple",
    "Tuple",
    "list",
    "List",
    "Sequence",
    "Iterable",
    "typing",
    "collections",
    "abc",
    "float",
    "int",
    "bool",
];

const STORAGE_TYPES: &[&str] = &[
    "ValueStorage",
    "FieldStorage",
    "ComponentStorage",
    "ResourceStorage",
    "AssetStorage",
];

/// Bevy short type name to the public PyBevy wrapper class names exposing it.
pub type WrapperIndex = HashMap<String, HashSet<String>>;

/// Short names of Bevy structs and enums that no PyBevy class exposes.
pub type UnwrappedBevyTypes = HashSet<String>;

/// Index public value wrappers (no ECS base class) by the Bevy type they wrap.
pub fn wrapper_index(rust_classes: &[PyClassDef], config: &BevyConfig) -> WrapperIndex {
    let mut index = WrapperIndex::new();
    for class in rust_classes {
        if class.python_name.starts_with('_')
            || class.extends.is_some()
            || !class
                .python_module_path
                .as_deref()
                .is_some_and(|module| module.starts_with("pybevy"))
        {
            continue;
        }
        let bevy_name = crate::comparison::pybevy_bevy_name(class, config);
        if bevy_name != class.python_name {
            continue;
        }
        index
            .entry(bevy_name)
            .or_default()
            .insert(class.python_name.clone());
    }
    index
}

/// Bevy structs and enums with no PyBevy class of the same Bevy or Python name.
pub fn unwrapped_bevy_types(
    rust_classes: &[PyClassDef],
    python_classes: &[PyClassDef],
    config: &BevyConfig,
    bevy_crates: &HashMap<String, BevyCrate>,
) -> UnwrappedBevyTypes {
    let mut wrapped: HashSet<String> = HashSet::new();
    for class in rust_classes {
        wrapped.insert(type_base(&crate::comparison::pybevy_bevy_name(class, config)).to_string());
        wrapped.insert(class.python_name.clone());
    }
    wrapped.extend(python_classes.iter().map(|class| class.python_name.clone()));
    bevy_crates
        .values()
        .flat_map(|bevy_crate| bevy_crate.items.values())
        .filter(|item| matches!(item.kind, BevyItemKind::Struct | BevyItemKind::Enum))
        .map(|item| type_base(&item.name).trim().to_string())
        .filter(|name| !wrapped.contains(name))
        .collect()
}

/// Upstream field or payload types for one class, keyed by Python-visible name.
#[derive(Debug, Default)]
pub struct UpstreamSlots {
    pub fields: HashMap<String, String>,
    pub variants: HashMap<String, BevyVariantKind>,
}

/// W018/W019: a stub publishes a builtin tuple/list/number where the Bevy
/// field or enum payload type has a PyBevy wrapper (W018) or is a Bevy struct
/// or enum that PyBevy does not wrap yet (W019).
pub fn check_wrapper_types(
    rust_class: &PyClassDef,
    python_class: &PyClassDef,
    origin: Option<&ConstructorOrigin>,
    upstream: &UpstreamSlots,
    index: &WrapperIndex,
    unwrapped: &UnwrappedBevyTypes,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let class_location = rust_class
        .constructor
        .as_ref()
        .and_then(|ctor| ctor.location.clone())
        .or_else(|| rust_class.location.clone());

    let constructor_fields: &[String] = match origin {
        Some(ConstructorOrigin::FieldDerived { field_order, .. }) => field_order,
        Some(ConstructorOrigin::Mixed { tail_fields, .. }) => tail_fields,
        _ => &[],
    };

    let mut names: Vec<&String> = upstream.fields.keys().collect();
    names.sort();
    for name in names {
        let bevy_type = &upstream.fields[name];
        let mut surfaces: Vec<(&str, &str)> = Vec::new();
        if constructor_fields.contains(name)
            && let Some(param) = python_class
                .constructor
                .as_ref()
                .and_then(|ctor| ctor.parameters.iter().find(|param| &param.name == name))
            && let Some(stub_type) = param.param_type.as_deref()
        {
            surfaces.push(("constructor parameter", stub_type));
        }
        if let Some(stub_type) = python_class
            .properties
            .iter()
            .find(|property| &property.name == name)
            .and_then(|property| property.property_type.as_deref())
        {
            surfaces.push(("property", stub_type));
        }
        let location = rust_class
            .properties
            .iter()
            .find(|property| &property.name == name)
            .and_then(|property| {
                property
                    .getter_location
                    .clone()
                    .or_else(|| property.setter_location.clone())
            })
            .or_else(|| class_location.clone());
        if let Some(diag) = report(
            &format!("'{}' field '{}'", rust_class.python_name, name),
            bevy_type,
            &surfaces,
            index,
            unwrapped,
            location,
        ) {
            diagnostics.push(diag);
        }
    }

    for variant in &python_class.enum_variants {
        let Some(bevy_kind) = upstream.variants.get(&variant.name) else {
            continue;
        };
        let stub_fields: Vec<(String, String)> = match &variant.kind {
            EnumVariantKind::Tuple(types) => types
                .iter()
                .map(|ty| ("value".to_string(), ty.clone()))
                .collect(),
            EnumVariantKind::Struct(fields) => fields.clone(),
            EnumVariantKind::Unit | EnumVariantKind::EmptyTuple => Vec::new(),
        };
        let positional = matches!(variant.kind, EnumVariantKind::Tuple(_))
            || rust_class.bevy_tuple_variants.contains(&variant.name);
        let ctor_params = variant
            .constructor
            .as_ref()
            .map(|ctor| ctor.parameters.as_slice())
            .unwrap_or_default();
        let mut field_names: Vec<String> = stub_fields.iter().map(|(n, _)| n.clone()).collect();
        for param in ctor_params {
            if !field_names.contains(&param.name) {
                field_names.push(param.name.clone());
            }
        }
        for (position, field_name) in field_names.iter().enumerate() {
            let bevy_type = match bevy_kind {
                BevyVariantKind::Struct(fields) => fields
                    .iter()
                    .find(|(name, _)| name == field_name)
                    .map(|(_, ty)| ty),
                BevyVariantKind::Tuple(types) if positional => types.get(position),
                BevyVariantKind::Tuple(_) => None,
                BevyVariantKind::Unit => None,
            };
            let Some(bevy_type) = bevy_type else {
                continue;
            };
            let mut surfaces: Vec<(&str, &str)> = Vec::new();
            if let Some((_, stub_type)) = stub_fields.iter().find(|(n, _)| n == field_name) {
                surfaces.push(("payload attribute", stub_type));
            }
            if let Some(stub_type) = ctor_params
                .iter()
                .find(|param| &param.name == field_name)
                .and_then(|param| param.param_type.as_deref())
            {
                surfaces.push(("constructor parameter", stub_type));
            }
            if let Some(diag) = report(
                &format!(
                    "'{}.{}' payload '{}'",
                    rust_class.python_name, variant.name, field_name
                ),
                bevy_type,
                &surfaces,
                index,
                unwrapped,
                class_location.clone(),
            ) {
                diagnostics.push(diag);
            }
        }
    }

    diagnostics
}

fn report(
    subject: &str,
    bevy_type: &str,
    surfaces: &[(&str, &str)],
    index: &WrapperIndex,
    unwrapped: &UnwrappedBevyTypes,
    location: Option<SourceLocation>,
) -> Option<Diagnostic> {
    let mut wrapped = (None, Vec::new());
    let mut missing = (None, Vec::new());
    for (surface, stub_type) in surfaces {
        let Some(shape) = builtin_shape(bevy_type, stub_type, index, unwrapped) else {
            continue;
        };
        let (name, offending) = match shape {
            BuiltinShape::Wrapped(name) => (name, &mut wrapped),
            BuiltinShape::Unwrapped(name) => (name, &mut missing),
        };
        offending.0 = Some(name);
        offending.1.push(format!("{surface} `{stub_type}`"));
    }
    let (code, message) = match (wrapped, missing) {
        ((Some(wrapper), offending), _) => (
            DiagnosticCode::W018,
            format!(
                "{subject} is stubbed as {} but the Bevy type `{}` has the PyBevy wrapper `{wrapper}`",
                offending.join(" and "),
                compact(bevy_type)
            ),
        ),
        (_, (Some(missing), offending)) => (
            DiagnosticCode::W019,
            format!(
                "{subject} is stubbed as {} but the Bevy type `{}` is built on `{missing}`, which has no PyBevy wrapper; wrap `{missing}` and use it in the stub",
                offending.join(" and "),
                compact(bevy_type)
            ),
        ),
        _ => return None,
    };
    let mut diag = Diagnostic::warning(code, message);
    if let Some(location) = location {
        diag = diag.with_location(location);
    }
    Some(diag)
}

enum BuiltinShape {
    Wrapped(String),
    Unwrapped(String),
}

/// The wrapper name when `stub_type` spells a builtin shape for a wrapped Bevy type.
pub fn builtin_for_wrapper(
    bevy_type: &str,
    stub_type: &str,
    index: &WrapperIndex,
) -> Option<String> {
    match builtin_shape(bevy_type, stub_type, index, &UnwrappedBevyTypes::new())? {
        BuiltinShape::Wrapped(name) => Some(name),
        BuiltinShape::Unwrapped(_) => None,
    }
}

/// The unwrapped Bevy type when `stub_type` spells a builtin shape for it.
pub fn builtin_for_unwrapped(
    bevy_type: &str,
    stub_type: &str,
    unwrapped: &UnwrappedBevyTypes,
) -> Option<String> {
    match builtin_shape(bevy_type, stub_type, &WrapperIndex::new(), unwrapped)? {
        BuiltinShape::Unwrapped(name) => Some(name),
        BuiltinShape::Wrapped(_) => None,
    }
}

fn builtin_shape(
    bevy_type: &str,
    stub_type: &str,
    index: &WrapperIndex,
    unwrapped: &UnwrappedBevyTypes,
) -> Option<BuiltinShape> {
    let bevy_type = compact(bevy_type);
    let (bevy_inner, stub_inner) = match generic_args(&bevy_type, "Option") {
        Some(inner) => (inner.to_string(), strip_none(stub_type)),
        None => (bevy_type.clone(), stub_type.trim().to_string()),
    };
    if let Some(element) = generic_args(&bevy_inner, "Vec")
        && let Some(stub_element) = ["list", "Sequence", "List"]
            .iter()
            .find_map(|base| python_subscript(&stub_inner, base))
    {
        return builtin_shape(element, stub_element, index, unwrapped);
    }
    let base = type_base(&bevy_inner);
    let identifiers: Vec<&str> = stub_inner
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|part| !part.is_empty() && !part.chars().all(|c| c.is_ascii_digit()))
        .collect();
    if identifiers.is_empty() || !identifiers.iter().all(|part| BUILTIN_NAMES.contains(part)) {
        return None;
    }
    if let Some(wrappers) = index.get(base) {
        if base == "Range" && generic_args(&bevy_inner, "Range") != Some("f32") {
            return None;
        }
        let mut names: Vec<&String> = wrappers.iter().collect();
        names.sort();
        return Some(BuiltinShape::Wrapped(names[0].clone()));
    }
    unwrapped
        .contains(base)
        .then(|| BuiltinShape::Unwrapped(base.to_string()))
}

/// W020: a public value class copies a Bevy struct's fields instead of holding
/// storage or using a generated wrapper.
pub fn check_hand_mirrored_value(
    rust_class: &PyClassDef,
    bevy_item: &BevyItem,
) -> Option<Diagnostic> {
    if rust_class.is_enum
        || rust_class.wrapper_macro.is_some()
        || rust_class.macro_info.is_some()
        || rust_class.python_name.starts_with('_')
        || !matches!(bevy_item.kind, BevyItemKind::Struct)
        || type_base(&bevy_item.name) != rust_class.python_name
        || !rust_class
            .python_module_path
            .as_deref()
            .is_some_and(|module| module.starts_with("pybevy"))
    {
        return None;
    }
    let bevy_name = type_base(&bevy_item.name);
    let holds_value = rust_class.struct_fields.iter().any(|(_, ty)| {
        rust_identifiers(ty).any(|part| STORAGE_TYPES.contains(&part) || part == bevy_name)
    });
    if holds_value {
        return None;
    }
    let mirrored: Vec<&str> = rust_class
        .struct_fields
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| {
            bevy_item
                .fields
                .iter()
                .any(|field| field.is_public && field.name == *name)
        })
        .collect();
    if mirrored.is_empty() {
        return None;
    }
    let mut diag = Diagnostic::warning(
        DiagnosticCode::W020,
        format!(
            "'{}' copies the fields {} of the Bevy struct `{}` instead of holding it in storage; use pyvalue (Copy) or pyfield (Clone) with the Bevy type",
            rust_class.python_name,
            mirrored
                .iter()
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>()
                .join(", "),
            bevy_item.full_path
        ),
    );
    if let Some(location) = rust_class.location.clone() {
        diag = diag.with_location(location);
    }
    Some(diag)
}

fn rust_identifiers(ty: &str) -> impl Iterator<Item = &str> {
    ty.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|part| !part.is_empty())
}

fn compact(ty: &str) -> String {
    ty.chars().filter(|c| !c.is_whitespace()).collect()
}

fn type_base(ty: &str) -> &str {
    let head = ty.split('<').next().unwrap_or(ty);
    head.rsplit("::").next().unwrap_or(head)
}

/// Inner arguments of `Base<...>` (any path prefix), for a compacted Rust type.
fn generic_args<'a>(ty: &'a str, base: &str) -> Option<&'a str> {
    let open = ty.find('<')?;
    if type_base(&ty[..open]) != base || !ty.ends_with('>') {
        return None;
    }
    Some(&ty[open + 1..ty.len() - 1])
}

fn python_subscript<'a>(ty: &'a str, base: &str) -> Option<&'a str> {
    let rest = ty.trim().strip_prefix(base)?.strip_prefix('[')?;
    rest.strip_suffix(']')
}

fn strip_none(stub_type: &str) -> String {
    let stub_type = stub_type.trim();
    if let Some(inner) = python_subscript(stub_type, "Optional") {
        return inner.trim().to_string();
    }
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in stub_type.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            '|' if depth == 0 => {
                parts.push(stub_type[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(stub_type[start..].trim());
    parts.retain(|part| *part != "None");
    parts.join(" | ")
}
