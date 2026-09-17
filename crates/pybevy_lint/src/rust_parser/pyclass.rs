use std::path::Path;

use syn::{Attribute, Expr, Fields, ItemEnum, ItemStruct, Lit, Meta};

use super::{macros, signature, types::type_to_string};
use crate::model::{
    EnumVariantDef, EnumVariantKind, MacroInfo, MethodDef, ParameterDef, ParameterKind,
    PropertyDef, PyClassDef, SelfMutability, SourceLocation,
};

/// Parse a struct item for pyclass attributes
pub fn parse_struct(item: &ItemStruct, file_path: &Path) -> Option<PyClassDef> {
    let pyclass_attr = find_pyclass_attr(&item.attrs)?;
    let pyclass_args = parse_pyclass_args(pyclass_attr);

    let rust_name = item.ident.to_string();
    let python_name = pyclass_args
        .name
        .unwrap_or_else(|| strip_py_prefix(&rust_name));

    let location = SourceLocation {
        file: file_path.to_path_buf(),
        line: item.ident.span().start().line,
        column: item.ident.span().start().column,
    };

    let macro_info = macros::parse_macro_attrs(&item.attrs);
    let bridge_info = macros::parse_bridge_attrs(&item.attrs, location.clone());
    let storage_macro = macros::parse_storage_macro(&item.attrs);

    let properties = parse_struct_field_properties(&item.fields, file_path);

    Some(PyClassDef {
        python_name,
        rust_name,
        python_module_path: pyclass_args.module,
        extends: pyclass_args.extends,
        frozen: pyclass_args.frozen,
        eq: pyclass_args.eq,
        hash: pyclass_args.hash,
        eq_int: pyclass_args.eq_int,
        subclass: pyclass_args.subclass,
        from_py_object: pyclass_args.from_py_object,
        location: Some(location),
        macro_info,
        bridge_info,
        storage_macro,
        is_enum: false,
        properties,
        methods: Vec::new(),
        constructor: None,
        ..Default::default()
    })
}

/// Parse an enum item for pyclass attributes
pub fn parse_enum(item: &ItemEnum, file_path: &Path) -> Option<PyClassDef> {
    let pyclass_attr = find_pyclass_attr(&item.attrs)?;
    let pyclass_args = parse_pyclass_args(pyclass_attr);

    let rust_name = item.ident.to_string();
    let python_name = pyclass_args
        .name
        .unwrap_or_else(|| strip_py_prefix(&rust_name));

    let location = SourceLocation {
        file: file_path.to_path_buf(),
        line: item.ident.span().start().line,
        column: item.ident.span().start().column,
    };

    let macro_info = macros::parse_macro_attrs(&item.attrs);

    let mut bevy_tuple_variants = std::collections::HashSet::new();
    let enum_variants = item
        .variants
        .iter()
        .map(|v| {
            let rust_name = v.ident.to_string();
            let name = parse_variant_pyo3_name(&v.attrs).unwrap_or_else(|| rust_name.clone());
            if v.attrs.iter().any(|attr| {
                attr.path().is_ident("py_bevy")
                    && attr
                        .parse_args::<syn::Ident>()
                        .is_ok_and(|shape| shape == "tuple")
            }) {
                bevy_tuple_variants.insert(name.clone());
            }
            let kind = match &v.fields {
                Fields::Unit => EnumVariantKind::Unit,
                Fields::Unnamed(fields) => {
                    if fields.unnamed.is_empty() {
                        EnumVariantKind::EmptyTuple
                    } else {
                        let types = fields
                            .unnamed
                            .iter()
                            .map(|field| type_to_string(&field.ty))
                            .collect();
                        EnumVariantKind::Tuple(types)
                    }
                }
                Fields::Named(fields) => {
                    let field_types = fields
                        .named
                        .iter()
                        .map(|f| {
                            let name = f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
                            let ty = type_to_string(&f.ty);
                            (name, ty)
                        })
                        .collect();
                    EnumVariantKind::Struct(field_types)
                }
            };
            let shape_generated = matches!(
                &macro_info,
                Some(MacroInfo::BevyEnum { message: true, .. })
                    | Some(MacroInfo::BevyEnum {
                        component: true,
                        ..
                    })
                    | Some(MacroInfo::BevyEnum { resource: true, .. })
            );
            // Struct-backed emitters consume field metadata, not PyO3 variant attributes.
            let constructor = if shape_generated {
                generated_variant_constructor(v, bevy_tuple_variants.contains(&name))
            } else {
                parse_variant_constructor(&v.attrs, &v.ident)
                    .or_else(|| synthesized_variant_constructor(&v.fields, &v.ident))
            };
            EnumVariantDef {
                name,
                kind,
                constructor,
            }
        })
        .collect();

    let generated_component_base = matches!(
        macro_info,
        Some(MacroInfo::BevyEnum {
            component: true,
            ..
        })
    );
    let generated_resource_base =
        matches!(macro_info, Some(MacroInfo::BevyEnum { resource: true, .. }));

    Some(PyClassDef {
        python_name,
        rust_name,
        python_module_path: pyclass_args.module,
        extends: pyclass_args
            .extends
            .or_else(|| generated_component_base.then(|| "PyComponent".to_string()))
            .or_else(|| generated_resource_base.then(|| "PyResource".to_string())),
        frozen: pyclass_args.frozen,
        eq: pyclass_args.eq,
        hash: pyclass_args.hash,
        eq_int: pyclass_args.eq_int,
        subclass: pyclass_args.subclass,
        from_py_object: pyclass_args.from_py_object,
        location: Some(location),
        macro_info,
        is_enum: true,
        enum_variants,
        bevy_tuple_variants,
        ..Default::default()
    })
}

/// Find the #[pyclass] attribute in a list of attributes
fn find_pyclass_attr(attrs: &[Attribute]) -> Option<&Attribute> {
    attrs.iter().find(|attr| attr.path().is_ident("pyclass"))
}

/// Parsed pyclass arguments
#[derive(Default)]
struct PyClassArgs {
    name: Option<String>,
    module: Option<String>,
    extends: Option<String>,
    frozen: bool,
    eq: bool,
    hash: bool,
    eq_int: bool,
    subclass: bool,
    from_py_object: bool,
}

/// Parse pyclass attribute arguments
fn parse_pyclass_args(attr: &Attribute) -> PyClassArgs {
    let mut args = PyClassArgs::default();

    let Ok(nested) =
        attr.parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
    else {
        return args;
    };

    for meta in nested {
        match meta {
            Meta::NameValue(nv) => {
                if nv.path.is_ident("name") {
                    if let Expr::Lit(expr_lit) = &nv.value
                        && let Lit::Str(lit_str) = &expr_lit.lit
                    {
                        args.name = Some(lit_str.value());
                    }
                } else if nv.path.is_ident("module") {
                    if let Expr::Lit(expr_lit) = &nv.value
                        && let Lit::Str(lit_str) = &expr_lit.lit
                    {
                        args.module = Some(lit_str.value());
                    }
                } else if nv.path.is_ident("extends") {
                    args.extends = Some(path_to_string(&nv.value));
                }
            }
            Meta::Path(path) => {
                if path.is_ident("frozen") {
                    args.frozen = true;
                } else if path.is_ident("eq") {
                    args.eq = true;
                } else if path.is_ident("hash") {
                    args.hash = true;
                } else if path.is_ident("eq_int") {
                    args.eq_int = true;
                } else if path.is_ident("subclass") {
                    args.subclass = true;
                } else if path.is_ident("from_py_object") {
                    args.from_py_object = true;
                }
            }
            _ => {}
        }
    }

    args
}

/// Convert an expression (usually a path) to a string
fn path_to_string(expr: &Expr) -> String {
    quote::quote!(#expr).to_string()
}

/// Strip "Py" prefix from Rust type name to get Python name
fn strip_py_prefix(name: &str) -> String {
    name.strip_prefix("Py").unwrap_or(name).to_string()
}

/// Parse struct fields for #[pyo3(get)] and #[pyo3(get, set)] attributes
/// These create automatic properties in PyO3
fn parse_struct_field_properties(fields: &Fields, file_path: &Path) -> Vec<PropertyDef> {
    let Fields::Named(named_fields) = fields else {
        return Vec::new();
    };

    let mut properties = Vec::new();

    for field in &named_fields.named {
        let Some(field_name) = &field.ident else {
            continue;
        };

        // Check for #[pyo3(get)] or #[pyo3(get, set)] attribute
        let (has_getter, has_setter, custom_name) = parse_pyo3_field_attr(&field.attrs);

        if has_getter || has_setter {
            let location = SourceLocation {
                file: file_path.to_path_buf(),
                line: field_name.span().start().line,
                column: field_name.span().start().column,
            };

            // Use custom name if provided, otherwise use field name
            let prop_name = custom_name.unwrap_or_else(|| field_name.to_string());

            properties.push(PropertyDef {
                name: prop_name,
                property_type: Some(type_to_string(&field.ty)),
                has_getter,
                has_setter,
                getter_mutability: SelfMutability::Ref, // Struct field getters use &self
                getter_location: Some(location.clone()),
                setter_location: if has_setter { Some(location) } else { None },
                getter_uses_borrow: false,
                getter_result_classification: Default::default(),
            });
        }
    }

    properties
}

/// Parse #[pyo3(...)] attribute on a struct field for get/set/name
/// Returns (has_getter, has_setter, custom_name)
fn parse_pyo3_field_attr(attrs: &[Attribute]) -> (bool, bool, Option<String>) {
    let mut has_getter = false;
    let mut has_setter = false;
    let mut custom_name = None;

    for attr in attrs {
        if !attr.path().is_ident("pyo3") {
            continue;
        }

        if let Ok(nested) = attr
            .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        {
            for meta in nested {
                match meta {
                    Meta::Path(path) => {
                        if path.is_ident("get") {
                            has_getter = true;
                        } else if path.is_ident("set") {
                            has_setter = true;
                        }
                    }
                    Meta::NameValue(nv) => {
                        if nv.path.is_ident("name")
                            && let syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Str(lit_str),
                                ..
                            }) = &nv.value
                        {
                            custom_name = Some(lit_str.value());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    (has_getter, has_setter, custom_name)
}

/// Parse #[pyo3(name = "...")] attribute on an enum variant
///
/// Returns the renamed name if present, None otherwise.
/// Extract parameter names in declaration order from `(p1, p2, *, p3)` text.
fn ordered_param_names(raw: &str) -> Vec<String> {
    raw.split(',')
        .filter_map(|seg| {
            let seg = seg.trim();
            if seg == "*" || seg.is_empty() {
                return None;
            }
            let mut name = seg.split(':').next().unwrap_or(seg).trim().to_string();
            if name.starts_with('*') || name.starts_with("**") {
                name = name.trim_start_matches('*').trim().to_string();
            }
            // `name = default` -> `name`
            if let Some(eq) = name.find('=') {
                name.truncate(eq);
                name = name.trim().to_string();
            }
            if name.is_empty() { None } else { Some(name) }
        })
        .collect()
}

/// Synthesize the pyo3 default constructor for an enum variant that has no
/// explicit `#[pyo3(constructor = ...)]`: pyo3 binds every field
/// positional-or-keyword. Attaching this makes the parsed Rust variant the
/// authority over stub claims even when the attribute is absent.
fn synthesized_variant_constructor(
    fields: &syn::Fields,
    variant_ident: &syn::Ident,
) -> Option<MethodDef> {
    let names: Vec<String> = match fields {
        Fields::Named(named) => named
            .named
            .iter()
            .map(|f| f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default())
            .filter(|n| !n.is_empty())
            .collect(),
        Fields::Unnamed(unnamed) if !unnamed.unnamed.is_empty() => {
            (0..unnamed.unnamed.len()).map(|i| i.to_string()).collect()
        }
        _ => return None,
    };
    if names.is_empty() {
        return None;
    }
    Some(MethodDef {
        name: "__new__".to_string(),
        rust_name: variant_ident.to_string(),
        parameters: names
            .into_iter()
            .map(|name| ParameterDef {
                name,
                param_type: Some("".to_string()),
                default_value: None,
                is_optional: false,
                kind: ParameterKind::PositionalOrKeyword,
            })
            .collect(),
        ..Default::default()
    })
}

fn generated_variant_constructor(variant: &syn::Variant, tuple: bool) -> Option<MethodDef> {
    if variant
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("py_unsupported"))
    {
        return None;
    }
    let mut constructor = synthesized_variant_constructor(&variant.fields, &variant.ident)?;
    for (parameter, field) in constructor.parameters.iter_mut().zip(variant.fields.iter()) {
        parameter.name = parameter.name.trim_start_matches("r#").to_string();
        parameter.kind = if tuple {
            ParameterKind::PositionalOrKeyword
        } else {
            ParameterKind::KeywordOnly
        };
        parameter.param_type = Some(type_to_string(&field.ty));
        for attr in &field.attrs {
            if attr.path().is_ident("py_default") {
                parameter.default_value = attr
                    .meta
                    .require_list()
                    .ok()
                    .map(|list| list.tokens.to_string());
            } else if attr.path().is_ident("py_field") {
                parameter.name = attr
                    .parse_args::<syn::Ident>()
                    .map(|name| name.to_string())
                    .or_else(|_| attr.parse_args::<syn::LitStr>().map(|name| name.value()))
                    .ok()?;
            } else if attr.path().is_ident("py_type") {
                parameter.param_type = attr
                    .parse_args::<syn::Type>()
                    .ok()
                    .as_ref()
                    .map(type_to_string);
            }
        }
    }
    Some(constructor)
}

/// Parse a generated variant's `#[pyo3(constructor = (...))]` attribute into
/// a MethodDef whose parameter kinds mirror the runtime's bound signature.
/// Returns None when the variant has no constructor attribute (pyo3 defaults
/// to positional-or-keyword for generated enum variants).
fn parse_variant_constructor(attrs: &[Attribute], variant_ident: &syn::Ident) -> Option<MethodDef> {
    for attr in attrs {
        let Some(info) = signature::parse_variant_constructor_attr(attr) else {
            continue;
        };
        // Parameter ORDER comes from the raw signature text (`*` sequences,
        // comma order); `kinds` is a HashMap with nondeterministic iteration.
        let names = ordered_param_names(&info.raw);
        let parameters: Vec<ParameterDef> = names
            .into_iter()
            .map(|name| ParameterDef {
                name: name.clone(),
                param_type: Some("".to_string()),
                default_value: info.defaults.get(&name).cloned(),
                is_optional: false,
                kind: info.kinds.get(&name).copied().unwrap_or_default(),
            })
            .collect();
        return Some(MethodDef {
            name: "__new__".to_string(),
            rust_name: variant_ident.to_string(),
            parameters,
            signature_str: Some(info.raw.clone()),
            ..Default::default()
        });
    }
    None
}

fn parse_variant_pyo3_name(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("pyo3") {
            continue;
        }

        if let Ok(nested) = attr
            .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        {
            for meta in nested {
                if let Meta::NameValue(nv) = meta
                    && nv.path.is_ident("name")
                    && let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(lit_str),
                        ..
                    }) = &nv.value
                {
                    return Some(lit_str.value());
                }
            }
        }
    }
    None
}
