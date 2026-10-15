use std::collections::{HashMap, HashSet};

use crate::{
    bevy_parser::types::BevyVariantKind,
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

/// Bevy short type name to the public PyBevy wrapper class names exposing it.
pub type WrapperIndex = HashMap<String, HashSet<String>>;

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

/// Upstream field or payload types for one class, keyed by Python-visible name.
#[derive(Debug, Default)]
pub struct UpstreamSlots {
    pub fields: HashMap<String, String>,
    pub variants: HashMap<String, BevyVariantKind>,
}

/// W018: a stub publishes a builtin tuple/list/number where the Bevy field or
/// enum payload type has a PyBevy wrapper.
pub fn check_wrapper_types(
    rust_class: &PyClassDef,
    python_class: &PyClassDef,
    origin: Option<&ConstructorOrigin>,
    upstream: &UpstreamSlots,
    index: &WrapperIndex,
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
    location: Option<SourceLocation>,
) -> Option<Diagnostic> {
    let mut wrapper = None;
    let mut offending: Vec<String> = Vec::new();
    for (surface, stub_type) in surfaces {
        if let Some(name) = builtin_for_wrapper(bevy_type, stub_type, index) {
            wrapper = Some(name);
            offending.push(format!("{surface} `{stub_type}`"));
        }
    }
    let wrapper = wrapper?;
    let mut diag = Diagnostic::warning(
        DiagnosticCode::W018,
        format!(
            "{subject} is stubbed as {} but the Bevy type `{}` has the PyBevy wrapper `{wrapper}`",
            offending.join(" and "),
            compact(bevy_type)
        ),
    );
    if let Some(location) = location {
        diag = diag.with_location(location);
    }
    Some(diag)
}

/// The wrapper name when `stub_type` spells a builtin shape for a wrapped Bevy type.
pub fn builtin_for_wrapper(
    bevy_type: &str,
    stub_type: &str,
    index: &WrapperIndex,
) -> Option<String> {
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
        return builtin_for_wrapper(element, stub_element, index);
    }
    let base = type_base(&bevy_inner);
    let wrappers = index.get(base)?;
    if base == "Range" && generic_args(&bevy_inner, "Range") != Some("f32") {
        return None;
    }
    let identifiers: Vec<&str> = stub_inner
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|part| !part.is_empty() && !part.chars().all(|c| c.is_ascii_digit()))
        .collect();
    if identifiers.is_empty()
        || identifiers.iter().any(|part| wrappers.contains(*part))
        || !identifiers.iter().all(|part| BUILTIN_NAMES.contains(part))
    {
        return None;
    }
    let mut names: Vec<&String> = wrappers.iter().collect();
    names.sort();
    Some(names[0].clone())
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
