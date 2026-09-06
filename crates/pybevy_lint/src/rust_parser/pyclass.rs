use std::path::Path;

use syn::{Attribute, Expr, Fields, ItemEnum, ItemStruct, Lit, Meta};

use super::{macros, types::type_to_string};
use crate::model::{
    EnumVariantDef, EnumVariantKind, MacroInfo, PropertyDef, PyClassDef, SelfMutability,
    SourceLocation,
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
        extends: pyclass_args.extends,
        frozen: pyclass_args.frozen,
        eq: pyclass_args.eq,
        subclass: pyclass_args.subclass,
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
            EnumVariantDef {
                name,
                kind,
                constructor: None,
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
        extends: pyclass_args
            .extends
            .or_else(|| generated_component_base.then(|| "PyComponent".to_string()))
            .or_else(|| generated_resource_base.then(|| "PyResource".to_string())),
        frozen: pyclass_args.frozen,
        eq: pyclass_args.eq,
        subclass: pyclass_args.subclass,
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
    extends: Option<String>,
    frozen: bool,
    eq: bool,
    subclass: bool,
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
                } else if nv.path.is_ident("extends") {
                    args.extends = Some(path_to_string(&nv.value));
                }
            }
            Meta::Path(path) => {
                if path.is_ident("frozen") {
                    args.frozen = true;
                } else if path.is_ident("eq") {
                    args.eq = true;
                } else if path.is_ident("subclass") {
                    args.subclass = true;
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
