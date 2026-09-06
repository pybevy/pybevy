use syn::{Attribute, Expr, Lit, Meta, Token};

use crate::model::{BridgeStorageKind, ComponentBridgeInfo, MacroInfo, SourceLocation};

/// Parse custom PyBevy macro attributes (pyenum).
pub fn parse_macro_attrs(attrs: &[Attribute]) -> Option<MacroInfo> {
    attrs
        .iter()
        .find(|attr| attr.path().is_ident("pyenum"))
        .and_then(parse_pyenum)
}

/// Name of the storage macro on a struct, whether or not it declares a bridge.
pub fn parse_storage_macro(attrs: &[Attribute]) -> Option<String> {
    const STORAGE_MACROS: [&str; 7] = [
        "pycomponent",
        "pyresource",
        "pyasset",
        "pywrap",
        "pyfield",
        "pyvalue",
        "pyhandle",
    ];
    attrs.iter().find_map(|attr| {
        let name = attr.path().get_ident()?.to_string();
        STORAGE_MACROS.contains(&name.as_str()).then_some(name)
    })
}

/// Parse bridge metadata from storage macro attributes like
/// `#[pycomponent(..., bridge, view_fields = [...], no_insert)]`.
pub fn parse_bridge_attrs(
    attrs: &[Attribute],
    location: SourceLocation,
) -> Option<ComponentBridgeInfo> {
    attrs
        .iter()
        .find_map(|attr| parse_bridge_attr(attr, &location))
}

fn parse_bridge_attr(attr: &Attribute, location: &SourceLocation) -> Option<ComponentBridgeInfo> {
    let attr_name = attr.path().get_ident()?.to_string();
    let storage_kind = match attr_name.as_str() {
        "pycomponent" => BridgeStorageKind::Component,
        "pyresource" => BridgeStorageKind::Resource,
        "pyasset" => BridgeStorageKind::Asset,
        "pywrap" => BridgeStorageKind::Newtype,
        _ => return None,
    };
    if !matches!(
        storage_kind,
        BridgeStorageKind::Component
            | BridgeStorageKind::Resource
            | BridgeStorageKind::Asset
            | BridgeStorageKind::Newtype
    ) {
        return None;
    }

    let tokens = attr.meta.require_list().ok()?.tokens.clone();
    let top_level_args = split_top_level_tokens(tokens);

    let mut has_bridge = false;
    let mut no_insert = false;
    let mut no_reflect = false;
    let mut view_fields = Vec::new();
    let mut batch_only_fields = Vec::new();
    let mut view_only_fields = Vec::new();

    let bevy_type = top_level_args
        .first()
        .map(ToString::to_string)
        .unwrap_or_default();

    for arg in top_level_args {
        let arg_str = arg.to_string();
        let trimmed = arg_str.trim();

        if trimmed == "bridge" {
            has_bridge = true;
            continue;
        }

        if trimmed == "no_insert" {
            no_insert = true;
            continue;
        }

        if trimmed == "no_reflect" {
            no_reflect = true;
            continue;
        }

        if let Some(fields) = parse_named_field_list(trimmed, "view_fields") {
            view_fields = fields;
            continue;
        }

        if let Some(fields) = parse_named_field_list(trimmed, "batch_only_fields") {
            batch_only_fields = fields;
            continue;
        }

        if let Some(fields) = parse_named_field_list(trimmed, "view_only_fields") {
            view_only_fields = fields;
        }
    }

    has_bridge.then_some(ComponentBridgeInfo {
        bevy_type,
        storage_kind,
        view_fields,
        batch_only_fields,
        view_only_fields,
        no_insert,
        no_reflect,
        location: Some(location.clone()),
    })
}

fn split_top_level_tokens(tokens: proc_macro2::TokenStream) -> Vec<proc_macro2::TokenStream> {
    let mut parts = Vec::new();
    let mut current = proc_macro2::TokenStream::new();

    for token in tokens {
        if matches!(&token, proc_macro2::TokenTree::Punct(p) if p.as_char() == ',') {
            if !current.is_empty() {
                parts.push(current);
                current = proc_macro2::TokenStream::new();
            }
            continue;
        }

        current.extend(std::iter::once(token));
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

fn parse_named_field_list(arg: &str, name: &str) -> Option<Vec<String>> {
    let (lhs, rhs) = arg.split_once('=')?;
    if lhs.trim() != name {
        return None;
    }

    let rhs = rhs.trim();
    let inner = rhs.strip_prefix('[')?.strip_suffix(']')?;
    Some(parse_field_entries(inner))
}

fn parse_field_entries(inner: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    for (index, character) in inner.char_indices() {
        match character {
            '[' | '(' | '{' | '<' => depth += 1,
            ']' | ')' | '}' | '>' if depth > 0 => depth -= 1,
            ',' if depth == 0 => {
                entries.push(&inner[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    entries.push(&inner[start..]);

    entries
        .into_iter()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(parse_field_name)
        .collect()
}

fn parse_field_name(entry: &str) -> String {
    let entry = entry
        .split_once("=>")
        .map_or(entry, |(field, _constraints)| field)
        .trim();

    if let Some((_, alias)) = entry.rsplit_once(" as ") {
        return alias.trim().to_string();
    }

    if let Some((name, _)) = entry.split_once(':') {
        return name.trim().to_string();
    }

    entry.trim().to_string()
}

/// Parse #[pyenum(Type, options...)]
fn parse_pyenum(attr: &Attribute) -> Option<MacroInfo> {
    let list = attr.meta.require_list().ok()?;
    let tokens: proc_macro2::TokenStream = list.tokens.clone();

    let mut bevy_type = String::new();
    let mut empty_tuple = false;
    let mut manual = false;
    let mut message = false;
    let mut component = false;
    let mut resource = false;
    let mut in_type = true;

    for token in tokens {
        match token {
            proc_macro2::TokenTree::Punct(p) if p.as_char() == ',' => {
                in_type = false;
            }
            proc_macro2::TokenTree::Ident(ident) if !in_type => match ident.to_string().as_str() {
                "empty_tuple" => empty_tuple = true,
                "manual" => manual = true,
                "message" => message = true,
                "component" => component = true,
                "resource" => resource = true,
                _ => {}
            },
            _ if in_type => {
                bevy_type.push_str(&token.to_string());
            }
            _ => {}
        }
    }

    let bevy_type = bevy_type.trim().to_string();

    Some(MacroInfo::BevyEnum {
        bevy_type,
        empty_tuple,
        manual,
        message,
        component,
        resource,
    })
}

/// Parse a pyo3 attribute to extract pyo3-specific options
pub fn parse_pyo3_attr(attr: &Attribute) -> Option<Pyo3Args> {
    if !attr.path().is_ident("pyo3") {
        return None;
    }

    let mut args = Pyo3Args::default();

    let Ok(nested) =
        attr.parse_args_with(syn::punctuated::Punctuated::<Meta, Token![,]>::parse_terminated)
    else {
        return Some(args);
    };

    for meta in nested {
        if let Meta::NameValue(nv) = meta {
            if nv.path.is_ident("name") {
                if let Expr::Lit(expr_lit) = &nv.value
                    && let Lit::Str(lit_str) = &expr_lit.lit
                {
                    args.name = Some(lit_str.value());
                }
            } else if nv.path.is_ident("signature") {
                args.signature = Some(quote::quote!(#nv.value).to_string());
            }
        }
    }

    Some(args)
}

/// Parsed pyo3 attribute arguments
#[derive(Default)]
pub struct Pyo3Args {
    pub name: Option<String>,
    pub signature: Option<String>,
}
