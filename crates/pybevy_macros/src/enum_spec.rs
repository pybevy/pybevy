use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, Expr, Field, Fields, Ident, ItemEnum, Lit, LitStr, Meta, Type,
    punctuated::Punctuated, token::Comma,
};

/// Interpreter-neutral structural description of an enum wrapper.
pub(crate) struct EnumSpec<'a> {
    pub(crate) wrapper_name: &'a Ident,
    pub(crate) inner_type: &'a Type,
    pub(crate) variants: Vec<VariantSpec<'a>>,
}

pub(crate) struct VariantSpec<'a> {
    pub(crate) rust_name: &'a Ident,
    pub(crate) python_name: String,
    pub(crate) shape: VariantShape<'a>,
    pub(crate) bevy_shape: BevyVariantShape,
    /// Keep this Bevy variant visible to parity audits without exposing a
    /// constructible Python nested class.
    pub(crate) unsupported: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BevyVariantShape {
    Unit,
    Tuple,
    Struct,
}

pub(crate) enum VariantShape<'a> {
    Unit,
    EmptyTuple,
    Tuple(Vec<FieldSpec<'a>>),
    Struct(Vec<FieldSpec<'a>>),
}

#[allow(dead_code)] // The RP2 emitter consumes the full field contract after branch integration.
pub(crate) struct FieldSpec<'a> {
    pub(crate) rust_name: Option<&'a Ident>,
    pub(crate) python_name: String,
    pub(crate) rust_type: &'a Type,
    /// Python-facing wrapper type used at the adapter boundary.
    ///
    /// When absent, the stored Rust type is exposed directly. Emitters own the
    /// interpreter-specific conversion in either direction.
    pub(crate) python_type: Option<Type>,
    /// The wrapper-to-inner conversion is fallible (`TryFrom`) rather than `From`.
    pub(crate) try_into: bool,
    /// Returned wrapped values require exact nested-variant materialization.
    pub(crate) materialize: bool,
    pub(crate) default: Option<TokenStream>,
    pub(crate) keyword_only: bool,
    /// Generate a Python setter for a struct-backed mutable storage base.
    pub(crate) writable: bool,
    /// Return a storage-backed Python wrapper borrowing this component field.
    pub(crate) borrowed: bool,
    pub(crate) declaration_index: usize,
}

impl<'a> EnumSpec<'a> {
    pub(crate) fn parse(item: &'a ItemEnum, inner_type: &'a Type) -> syn::Result<Self> {
        if item.variants.is_empty() {
            return Err(syn::Error::new_spanned(
                item,
                "enum wrappers must declare at least one variant",
            ));
        }

        let variants = item
            .variants
            .iter()
            .map(|variant| {
                let python_name = pyo3_variant_name(&variant.attrs)
                    .unwrap_or_else(|| normalized_ident(&variant.ident));
                let (shape, default_bevy_shape) = match &variant.fields {
                    Fields::Unit => (VariantShape::Unit, BevyVariantShape::Unit),
                    Fields::Unnamed(fields) if fields.unnamed.is_empty() => {
                        (VariantShape::EmptyTuple, BevyVariantShape::Unit)
                    }
                    Fields::Unnamed(fields) => (
                        VariantShape::Tuple(parse_fields(
                            fields.unnamed.iter(),
                            false,
                            fields.unnamed.len(),
                        )?),
                        BevyVariantShape::Tuple,
                    ),
                    Fields::Named(fields) => (
                        VariantShape::Struct(parse_fields(
                            fields.named.iter(),
                            true,
                            fields.named.len(),
                        )?),
                        BevyVariantShape::Struct,
                    ),
                };
                let bevy_shape = explicit_bevy_shape(&variant.attrs)?.unwrap_or(default_bevy_shape);
                let unsupported = unique_marker(&variant.attrs, "py_unsupported")?;

                if bevy_shape == BevyVariantShape::Tuple
                    && !matches!(shape, VariantShape::Tuple(_) | VariantShape::Struct(_))
                {
                    return Err(syn::Error::new_spanned(
                        variant,
                        "#[py_bevy(tuple)] requires a payload variant",
                    ));
                }

                Ok(VariantSpec {
                    rust_name: &variant.ident,
                    python_name,
                    shape,
                    bevy_shape,
                    unsupported,
                })
            })
            .collect::<syn::Result<Vec<_>>>()?;

        Ok(Self {
            wrapper_name: &item.ident,
            inner_type,
            variants,
        })
    }

    #[allow(dead_code)] // The RP2 emitter uses this classification.
    pub(crate) fn is_complex(&self) -> bool {
        self.variants
            .iter()
            .any(|variant| !matches!(variant.shape, VariantShape::Unit))
    }
}

/// Constructor parameter declaration for one adapter field.
pub(crate) fn parameter(field: &FieldSpec<'_>) -> TokenStream {
    let name = parameter_name(field);
    let ty = field.python_type.as_ref().unwrap_or(field.rust_type);
    quote! { #name: #ty }
}

/// Public Python name of one adapter field as an identifier.
pub(crate) fn parameter_name(field: &FieldSpec<'_>) -> Ident {
    format_ident!("{}", field.python_name)
}

/// `#[pyo3(signature = ...)]` for a variant constructor, honoring defaults and
/// keyword-only fields; empty when the plain signature suffices.
pub(crate) fn constructor_signature(fields: &[FieldSpec<'_>]) -> TokenStream {
    if fields
        .iter()
        .all(|field| field.default.is_none() && !field.keyword_only)
    {
        return TokenStream::new();
    }
    let mut parts = Vec::new();
    let mut emitted_star = false;
    for field in fields {
        if field.keyword_only && !emitted_star {
            parts.push(quote! { * });
            emitted_star = true;
        }
        let name = parameter_name(field);
        if let Some(default) = &field.default {
            parts.push(quote! { #name = #default });
        } else {
            parts.push(quote! { #name });
        }
    }
    quote! { #[pyo3(signature = (#(#parts),*))] }
}

fn explicit_bevy_shape(attrs: &[Attribute]) -> syn::Result<Option<BevyVariantShape>> {
    let attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("py_bevy"))
        .collect::<Vec<_>>();
    match attrs.as_slice() {
        [] => Ok(None),
        [attr] => {
            let shape = attr.parse_args::<Ident>()?;
            match shape.to_string().as_str() {
                "tuple" => Ok(Some(BevyVariantShape::Tuple)),
                _ => Err(syn::Error::new_spanned(
                    shape,
                    "unknown Bevy enum shape; expected `tuple`",
                )),
            }
        }
        _ => Err(syn::Error::new_spanned(
            attrs[1],
            "enum variants may declare #[py_bevy(...)] only once",
        )),
    }
}

fn parse_fields<'a>(
    fields: impl Iterator<Item = &'a Field>,
    named: bool,
    field_count: usize,
) -> syn::Result<Vec<FieldSpec<'a>>> {
    let mut specs = Vec::with_capacity(field_count);
    let mut python_names = HashSet::with_capacity(field_count);
    let mut saw_positional_default = false;
    let mut saw_keyword_only = false;

    for (declaration_index, field) in fields.enumerate() {
        let explicit_name = unique_field_name(&field.attrs)?;
        let python_name = if let Some(name) = explicit_name {
            name
        } else if let Some(rust_name) = &field.ident {
            normalized_ident(rust_name)
        } else if field_count == 1 {
            "value".to_string()
        } else {
            return Err(syn::Error::new_spanned(
                field,
                "multi-field tuple variants require #[py_field(name)] on every field",
            ));
        };

        if !python_names.insert(python_name.clone()) {
            return Err(syn::Error::new_spanned(
                field,
                format!("duplicate Python enum field name '{python_name}'"),
            ));
        }

        let default = unique_default(&field.attrs)?;
        let python_type = unique_python_type(&field.attrs)?;
        let try_into = unique_marker(&field.attrs, "py_try_into")?;
        let materialize = unique_marker(&field.attrs, "py_materialize")?;
        if python_type.is_none() && (try_into || materialize) {
            return Err(syn::Error::new_spanned(
                field,
                "#[py_try_into] and #[py_materialize] require #[py_type(PyWrapper)]",
            ));
        }
        let keyword_only = unique_keyword_only(&field.attrs)?;
        let writable = unique_marker(&field.attrs, "py_set")?;
        let borrowed = unique_marker(&field.attrs, "py_borrow")?;
        if borrowed && python_type.is_none() {
            return Err(syn::Error::new_spanned(
                field,
                "#[py_borrow] requires #[py_type(PyWrapper)]",
            ));
        }
        if saw_keyword_only && !keyword_only {
            return Err(syn::Error::new_spanned(
                field,
                "positional enum fields cannot follow a keyword-only field",
            ));
        }
        saw_keyword_only |= keyword_only;

        if !keyword_only {
            if saw_positional_default && default.is_none() {
                return Err(syn::Error::new_spanned(
                    field,
                    "required positional enum fields cannot follow defaulted fields; mark the field #[py_kw_only]",
                ));
            }
            saw_positional_default |= default.is_some();
        }

        specs.push(FieldSpec {
            rust_name: if named {
                Some(field.ident.as_ref().expect("named field"))
            } else {
                None
            },
            python_name,
            rust_type: &field.ty,
            python_type,
            try_into,
            materialize,
            default,
            keyword_only,
            writable,
            borrowed,
            declaration_index,
        });
    }

    Ok(specs)
}

fn unique_marker(attrs: &[Attribute], name: &str) -> syn::Result<bool> {
    let attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident(name))
        .collect::<Vec<_>>();
    match attrs.as_slice() {
        [] => Ok(false),
        [attr] if matches!(attr.meta, Meta::Path(_)) => Ok(true),
        [attr] => Err(syn::Error::new_spanned(
            attr,
            format!("#[{name}] does not take arguments"),
        )),
        _ => Err(syn::Error::new_spanned(
            attrs[1],
            format!("enum fields may declare #[{name}] only once"),
        )),
    }
}

fn unique_python_type(attrs: &[Attribute]) -> syn::Result<Option<Type>> {
    let attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("py_type"))
        .collect::<Vec<_>>();
    match attrs.as_slice() {
        [] => Ok(None),
        [attr] => attr.parse_args::<Type>().map(Some),
        _ => Err(syn::Error::new_spanned(
            attrs[1],
            "enum fields may declare #[py_type(...)] only once",
        )),
    }
}

fn unique_field_name(attrs: &[Attribute]) -> syn::Result<Option<String>> {
    let names = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("py_field"))
        .map(parse_name_argument)
        .collect::<syn::Result<Vec<_>>>()?;
    match names.as_slice() {
        [] => Ok(None),
        [name] => Ok(Some(name.clone())),
        _ => Err(syn::Error::new_spanned(
            attrs
                .iter()
                .find(|attr| attr.path().is_ident("py_field"))
                .expect("at least one field name"),
            "enum fields may declare #[py_field(...)] only once",
        )),
    }
}

fn parse_name_argument(attr: &Attribute) -> syn::Result<String> {
    if let Ok(name) = attr.parse_args::<Ident>() {
        return Ok(normalized_ident(&name));
    }
    attr.parse_args::<LitStr>().map(|name| name.value())
}

fn unique_default(attrs: &[Attribute]) -> syn::Result<Option<TokenStream>> {
    let defaults = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("py_default"))
        .map(|attr| match &attr.meta {
            Meta::List(list) if !list.tokens.is_empty() => Ok(list.tokens.clone()),
            _ => Err(syn::Error::new_spanned(
                attr,
                "#[py_default(...)] requires an explicit default expression",
            )),
        })
        .collect::<syn::Result<Vec<_>>>()?;
    match defaults.as_slice() {
        [] => Ok(None),
        [default] => Ok(Some(default.clone())),
        _ => Err(syn::Error::new_spanned(
            attrs
                .iter()
                .find(|attr| attr.path().is_ident("py_default"))
                .expect("at least one default"),
            "enum fields may declare #[py_default(...)] only once",
        )),
    }
}

fn unique_keyword_only(attrs: &[Attribute]) -> syn::Result<bool> {
    let attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("py_kw_only"))
        .collect::<Vec<_>>();
    match attrs.as_slice() {
        [] => Ok(false),
        [attr] if matches!(attr.meta, Meta::Path(_)) => Ok(true),
        [attr] => Err(syn::Error::new_spanned(
            attr,
            "#[py_kw_only] does not take arguments",
        )),
        _ => Err(syn::Error::new_spanned(
            attrs[0],
            "enum fields may declare #[py_kw_only] only once",
        )),
    }
}

fn normalized_ident(ident: &Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").unwrap_or(&name).to_string()
}

fn pyo3_variant_name(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("pyo3") {
            continue;
        }
        let Ok(metas) = attr.parse_args_with(Punctuated::<Meta, Comma>::parse_terminated) else {
            continue;
        };
        for meta in metas {
            if let Meta::NameValue(name_value) = meta
                && name_value.path.is_ident("name")
                && let Expr::Lit(expr) = name_value.value
                && let Lit::Str(value) = expr.lit
            {
                return Some(value.value());
            }
        }
    }
    None
}
