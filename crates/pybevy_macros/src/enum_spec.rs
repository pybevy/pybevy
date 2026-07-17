use std::collections::HashSet;

use proc_macro2::TokenStream;
use syn::{Attribute, Field, Fields, Ident, ItemEnum, LitStr, Meta, Type};

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
    pub(crate) default: Option<TokenStream>,
    pub(crate) keyword_only: bool,
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
                let shape = match &variant.fields {
                    Fields::Unit => VariantShape::Unit,
                    Fields::Unnamed(fields) if fields.unnamed.is_empty() => {
                        VariantShape::EmptyTuple
                    }
                    Fields::Unnamed(fields) => VariantShape::Tuple(parse_fields(
                        fields.unnamed.iter(),
                        false,
                        fields.unnamed.len(),
                    )?),
                    Fields::Named(fields) => VariantShape::Struct(parse_fields(
                        fields.named.iter(),
                        true,
                        fields.named.len(),
                    )?),
                };

                Ok(VariantSpec {
                    rust_name: &variant.ident,
                    python_name,
                    shape,
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
        let keyword_only = unique_keyword_only(&field.attrs)?;
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
            default,
            keyword_only,
            declaration_index,
        });
    }

    Ok(specs)
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
        if attr.path().is_ident("pyo3")
            && let Meta::List(meta_list) = &attr.meta
        {
            let tokens = meta_list.tokens.to_string();
            if let Some(start) = tokens.find("name") {
                let rest = &tokens[start..];
                if let Some(eq_pos) = rest.find('=') {
                    let after_eq = rest[eq_pos + 1..].trim();
                    if let Some(first_quote) = after_eq.find('"') {
                        let after_first = &after_eq[first_quote + 1..];
                        if let Some(second_quote) = after_first.find('"') {
                            return Some(after_first[..second_quote].to_string());
                        }
                    }
                }
            }
        }
    }
    None
}
