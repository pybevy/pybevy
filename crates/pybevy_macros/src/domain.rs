//! `domains = [field => [rules]]`: parsing and the inherent items it generates,
//! shared by every wrapper macro that stores a Bevy value.

use quote::quote;
use syn::{
    Expr, Ident, Token, Type,
    parse::{Parse, ParseStream},
};

pub(crate) const DOMAIN_RULES: &str = "finite, not_nan, finite_or_pos_inf, non_negative, positive, range(lo, hi), range_inclusive(lo, hi), at_least(n)";

/// One rule of a `domains` entry, lowered to `pybevy_core::domain::FieldDomain`.
#[derive(Clone)]
pub(crate) enum DomainRuleKind {
    Finite,
    NotNan,
    FiniteOrPosInf,
    NonNegative,
    Positive,
    Range(Box<Expr>, Box<Expr>),
    RangeInclusive(Box<Expr>, Box<Expr>),
    AtLeast(Box<Expr>),
}

#[derive(Clone)]
pub(crate) struct DomainRule {
    pub(crate) kind: DomainRuleKind,
    /// Explicit `view = checked` or `view = unchecked`.
    view: Option<bool>,
}

impl Parse for DomainRule {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let name = ident.to_string();
        if !matches!(
            name.as_str(),
            "finite"
                | "not_nan"
                | "finite_or_pos_inf"
                | "non_negative"
                | "positive"
                | "range"
                | "range_inclusive"
                | "at_least"
        ) {
            return Err(syn::Error::new_spanned(
                &ident,
                format!("unknown domain rule, expected one of: {DOMAIN_RULES}"),
            ));
        }
        let mut args: Vec<Expr> = Vec::new();
        let mut view = None;
        if input.peek(syn::token::Paren) {
            let content;
            syn::parenthesized!(content in input);
            while !content.is_empty() {
                if content.peek(Ident) && content.peek2(Token![=]) {
                    let key: Ident = content.parse()?;
                    if key != "view" {
                        return Err(syn::Error::new_spanned(
                            key,
                            "expected `view = checked` or `view = unchecked`",
                        ));
                    }
                    content.parse::<Token![=]>()?;
                    let value: Ident = content.parse()?;
                    view = Some(match value.to_string().as_str() {
                        "checked" => true,
                        "unchecked" => false,
                        _ => {
                            return Err(syn::Error::new_spanned(
                                value,
                                "expected `checked` or `unchecked`",
                            ));
                        }
                    });
                } else {
                    args.push(content.parse()?);
                }
                if !content.is_empty() {
                    content.parse::<Token![,]>()?;
                }
            }
        }
        let bound = |expr: &Expr| Box::new(expr.clone());
        let kind = match (name.as_str(), args.as_slice()) {
            ("finite", []) => DomainRuleKind::Finite,
            ("not_nan", []) => DomainRuleKind::NotNan,
            ("finite_or_pos_inf", []) => DomainRuleKind::FiniteOrPosInf,
            ("non_negative", []) => DomainRuleKind::NonNegative,
            ("positive", []) => DomainRuleKind::Positive,
            ("range", [lo, hi]) => DomainRuleKind::Range(bound(lo), bound(hi)),
            ("range_inclusive", [lo, hi]) => DomainRuleKind::RangeInclusive(bound(lo), bound(hi)),
            ("at_least", [n]) => DomainRuleKind::AtLeast(bound(n)),
            _ => {
                return Err(syn::Error::new_spanned(
                    &ident,
                    format!("wrong arguments for `{name}`, expected one of: {DOMAIN_RULES}"),
                ));
            }
        };
        Ok(Self { kind, view })
    }
}

impl DomainRule {
    pub(crate) fn tokens(&self) -> proc_macro2::TokenStream {
        match &self.kind {
            DomainRuleKind::Finite => quote! { pybevy_core::domain::FieldDomain::Finite },
            DomainRuleKind::NotNan => quote! { pybevy_core::domain::FieldDomain::NotNan },
            DomainRuleKind::FiniteOrPosInf => {
                quote! { pybevy_core::domain::FieldDomain::FiniteOrPosInf }
            }
            DomainRuleKind::NonNegative => quote! { pybevy_core::domain::FieldDomain::NonNegative },
            DomainRuleKind::Positive => quote! { pybevy_core::domain::FieldDomain::Positive },
            DomainRuleKind::Range(lo, hi) => quote! {
                pybevy_core::domain::FieldDomain::Range { lo: (#lo) as f64, hi: (#hi) as f64 }
            },
            DomainRuleKind::RangeInclusive(lo, hi) => quote! {
                pybevy_core::domain::FieldDomain::RangeInclusive { lo: (#lo) as f64, hi: (#hi) as f64 }
            },
            DomainRuleKind::AtLeast(n) => quote! {
                pybevy_core::domain::FieldDomain::AtLeast((#n) as i64)
            },
        }
    }

    /// Rules whose violations follow NumPy float semantics on View by default.
    fn is_float_only(&self) -> bool {
        matches!(
            self.kind,
            DomainRuleKind::Finite | DomainRuleKind::NotNan | DomainRuleKind::FiniteOrPosInf
        )
    }
}

/// A `domains = [field => [rules]]` entry.
#[derive(Clone)]
pub(crate) struct DomainEntry {
    pub(crate) rust_accessor: proc_macro2::TokenStream,
    pub(crate) python_name: Ident,
    pub(crate) rules: Vec<DomainRule>,
    pub(crate) view_checked: bool,
}

impl Parse for DomainEntry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let (rust_accessor, _offset_path, python_name) = parse_field_path(input)?;
        input.parse::<Token![=>]>()?;
        let content;
        syn::bracketed!(content in input);
        let rules: Vec<DomainRule> = content
            .parse_terminated(DomainRule::parse, Token![,])?
            .into_iter()
            .collect();
        if rules.is_empty() {
            return Err(syn::Error::new_spanned(
                &python_name,
                "a domains entry needs at least one rule",
            ));
        }
        let explicit: Vec<bool> = rules.iter().filter_map(|rule| rule.view).collect();
        let view_checked = match explicit.as_slice() {
            [] => rules.iter().any(|rule| !rule.is_float_only()),
            [first, rest @ ..] if rest.iter().all(|view| view == first) => *first,
            _ => {
                return Err(syn::Error::new_spanned(
                    &python_name,
                    "conflicting `view` settings in one domains entry",
                ));
            }
        };
        Ok(Self {
            rust_accessor,
            python_name,
            rules,
            view_checked,
        })
    }
}

impl DomainEntry {
    pub(crate) fn const_ident(&self) -> Ident {
        quote::format_ident!("{}_DOMAIN", self.python_name.to_string().to_uppercase())
    }

    pub(crate) fn check_ident(&self) -> Ident {
        quote::format_ident!("check_{}", self.python_name)
    }

    pub(crate) fn validate_ident(&self) -> Ident {
        quote::format_ident!("validate_{}", self.python_name)
    }
}

/// Parse a field path: `intensity`, or a tuple index with an alias such as
/// `0 as value` or `0.x as x`. Returns the accessor, the `offset_of!` path and
/// the Python name.
pub(crate) fn parse_field_path(
    input: ParseStream,
) -> syn::Result<(proc_macro2::TokenStream, proc_macro2::TokenStream, Ident)> {
    if input.peek(syn::LitInt) {
        let idx: syn::LitInt = input.parse()?;
        let idx_val = idx.base10_parse::<usize>()?;
        let idx_lit = syn::Index::from(idx_val);

        let mut accessor_tokens = quote! { .#idx_lit };
        let mut offset_tokens = quote! { #idx_lit };

        while input.peek(Token![.]) {
            input.parse::<Token![.]>()?;
            let sub: Ident = input.parse()?;
            accessor_tokens = quote! { #accessor_tokens.#sub };
            offset_tokens = quote! { #offset_tokens.#sub };
        }

        input.parse::<Token![as]>()?;
        let python_name: Ident = input.parse()?;
        Ok((accessor_tokens, offset_tokens, python_name))
    } else {
        let ident: Ident = input.parse()?;
        Ok((quote! { .#ident }, quote! { #ident }, ident))
    }
}

/// Inherent items generated from `domains`: per-field rule constants and
/// checks, and `validate_domains` over the whole native value.
pub(crate) fn domain_impl_tokens(
    bevy_type: &Type,
    py_type: &Ident,
    owner: &str,
    domains: &[DomainEntry],
) -> proc_macro2::TokenStream {
    if domains.is_empty() {
        return quote! {};
    }
    let items = domains.iter().map(|entry| {
        let const_ident = entry.const_ident();
        let check_ident = entry.check_ident();
        let validate_ident = entry.validate_ident();
        let name = entry.python_name.to_string();
        let rules = entry.rules.iter().map(DomainRule::tokens);
        quote! {
            pub const #const_ident: &'static [pybevy_core::domain::FieldDomain] = &[#(#rules),*];

            pub fn #check_ident<V: pybevy_core::domain::DomainValues>(
                value: V,
            ) -> Result<V, pybevy_core::domain::DomainViolation> {
                pybevy_core::domain::check(#owner, #name, value, Self::#const_ident)
            }

            pub fn #validate_ident<V: pybevy_core::domain::DomainValues>(
                value: &V,
            ) -> Result<(), pybevy_core::StorageError> {
                Self::#check_ident(*value).map(drop).map_err(Into::into)
            }
        }
    });
    let whole = domains.iter().map(|entry| {
        let check_ident = entry.check_ident();
        let accessor = &entry.rust_accessor;
        quote! { Self::#check_ident(value #accessor)?; }
    });
    quote! {
        impl #py_type {
            #(#items)*

            /// Check every declared field domain of a whole native value.
            pub fn validate_domains(
                value: &#bevy_type,
            ) -> Result<(), pybevy_core::domain::DomainViolation> {
                #(#whole)*
                Ok(())
            }
        }
    }
}

/// The Python-facing owner name: the bridge name, or the wrapper name without
/// its `Py` prefix.
pub(crate) fn owner_name(py_type: &Ident, bridge_name_override: Option<&str>) -> String {
    bridge_name_override.map(String::from).unwrap_or_else(|| {
        let name = py_type.to_string();
        name.strip_prefix("Py").unwrap_or(&name).to_string()
    })
}

/// Parse `= [field => [rules], ...]` after a `domains` key into `domains`,
/// rejecting a field named twice.
pub(crate) fn parse_domains(input: ParseStream, domains: &mut Vec<DomainEntry>) -> syn::Result<()> {
    input.parse::<Token![=]>()?;
    let content;
    syn::bracketed!(content in input);
    let entries = content.parse_terminated(DomainEntry::parse, Token![,])?;
    for entry in entries {
        if domains
            .iter()
            .any(|existing| existing.python_name == entry.python_name)
        {
            return Err(syn::Error::new_spanned(
                &entry.python_name,
                "duplicate domains entry",
            ));
        }
        domains.push(entry);
    }
    Ok(())
}

/// Arguments of `#[pyvalue(...)]` and `#[pyfield(...)]`: an optional explicit
/// Bevy type, then optional `domains = [...]`.
pub(crate) struct StorageTypeArgs {
    pub(crate) bevy_type: Option<Type>,
    pub(crate) domains: Vec<DomainEntry>,
}

impl Parse for StorageTypeArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut bevy_type = None;
        let mut domains = Vec::new();
        while !input.is_empty() {
            if input.peek(Ident) && input.peek2(Token![=]) {
                let key: Ident = input.parse()?;
                if key != "domains" {
                    return Err(syn::Error::new_spanned(
                        key,
                        "unknown option, expected `domains = [...]`",
                    ));
                }
                parse_domains(input, &mut domains)?;
            } else if bevy_type.is_none() && domains.is_empty() {
                bevy_type = Some(input.parse()?);
            } else {
                return Err(input.error("expected `domains = [...]`"));
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Self { bevy_type, domains })
    }
}
