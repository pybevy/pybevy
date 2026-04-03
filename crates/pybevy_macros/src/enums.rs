use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, ItemEnum, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

/// Arguments for pyenum attribute macro
struct BevyEnumArgs {
    /// The Bevy enum type to convert to/from
    bevy_type: Type,
    /// If true, map empty tuple variants Variant() to Bevy's unit variants Variant
    empty_tuple: bool,
}

impl Parse for BevyEnumArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let bevy_type: Type = input.parse()?;

        let mut empty_tuple = false;
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let option: Ident = input.parse()?;
            match option.to_string().as_str() {
                "empty_tuple" => empty_tuple = true,
                other => {
                    return Err(syn::Error::new_spanned(
                        option,
                        format!("unknown option '{}', expected: empty_tuple", other),
                    ));
                }
            }
        }

        Ok(BevyEnumArgs {
            bevy_type,
            empty_tuple,
        })
    }
}

/// Generates boilerplate implementations for PyBevy enum wrapper types.
///
/// This macro generates:
/// - `impl From<BevyType> for PyType` (by matching variant names)
/// - `impl From<PyType> for BevyType` (by matching variant names)
/// - `#[pymethods]` with `__repr__`
///
/// **Important**: This macro only works for enums with unit variants (no data).
/// For enums with tuple/struct variants, implement conversions manually.
///
/// # Usage
///
/// ```rust
/// #[pyenum(BevyCursorGrabMode)]
/// #[pyclass(name = "CursorGrabMode", eq)]
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// pub enum PyCursorGrabMode {
///     None,
///     Confined,
///     Locked,
/// }
/// ```
///
/// The macro assumes variant names match between the Py and Bevy types.
/// Extract the pyo3 name from variant attributes if present
/// Looks for #[pyo3(name = "...")] attribute
fn get_pyo3_variant_name(variant: &syn::Variant) -> Option<String> {
    for attr in &variant.attrs {
        if attr.path().is_ident("pyo3")
            && let syn::Meta::List(meta_list) = &attr.meta
        {
            let tokens = meta_list.tokens.to_string();
            // Parse "name = \"SomeName\""
            if let Some(start) = tokens.find("name") {
                let rest = &tokens[start..];
                if let Some(eq_pos) = rest.find('=') {
                    let after_eq = rest[eq_pos + 1..].trim();
                    // Extract string between quotes
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

/// Variant kind for pyenum processing
enum VariantKind {
    /// Unit variant: `Variant`
    Unit,
    /// Empty tuple variant: `Variant()` - maps to Bevy's unit variant
    EmptyTuple,
    /// Single-field tuple variant: `Variant(T)` - passes through the value
    DataTuple,
}

pub fn pyenum(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as BevyEnumArgs);
    let input = parse_macro_input!(item as ItemEnum);

    let py_type = &input.ident;
    let bevy_type = &args.bevy_type;

    // Collect variant info (name, optional pyo3 rename, and variant kind)
    struct VariantInfo<'a> {
        ident: &'a Ident,
        repr_name: String,
        kind: VariantKind,
    }

    let mut variants: Vec<VariantInfo> = Vec::new();
    for variant in &input.variants {
        let repr_name = get_pyo3_variant_name(variant).unwrap_or_else(|| variant.ident.to_string());

        let kind = match &variant.fields {
            syn::Fields::Unit => Some(VariantKind::Unit),
            syn::Fields::Unnamed(f) if f.unnamed.is_empty() && args.empty_tuple => {
                Some(VariantKind::EmptyTuple)
            }
            syn::Fields::Unnamed(f) if f.unnamed.len() == 1 => {
                // Auto-detect single-field tuple variants like Other(u16)
                Some(VariantKind::DataTuple)
            }
            _ => None,
        };

        match kind {
            Some(k) => {
                variants.push(VariantInfo {
                    ident: &variant.ident,
                    repr_name,
                    kind: k,
                });
            }
            None => {
                let msg = if args.empty_tuple {
                    "pyenum only supports unit, empty tuple, and single-field tuple variants"
                } else {
                    "pyenum only supports unit and single-field tuple variants (use empty_tuple for Variant() style)"
                };
                return syn::Error::new_spanned(variant, msg)
                    .to_compile_error()
                    .into();
            }
        }
    }

    // Generate From<BevyType> for PyType
    let from_bevy_arms = variants.iter().map(|v| {
        let name = v.ident;
        match v.kind {
            VariantKind::Unit => quote! { #bevy_type::#name => #py_type::#name },
            VariantKind::EmptyTuple => quote! { #bevy_type::#name => #py_type::#name() },
            VariantKind::DataTuple => quote! { #bevy_type::#name(v) => #py_type::#name(v) },
        }
    });

    // Generate From<PyType> for BevyType
    let from_py_arms = variants.iter().map(|v| {
        let name = v.ident;
        match v.kind {
            VariantKind::Unit => quote! { #py_type::#name => #bevy_type::#name },
            VariantKind::EmptyTuple => quote! { #py_type::#name() => #bevy_type::#name },
            VariantKind::DataTuple => quote! { #py_type::#name(v) => #bevy_type::#name(v) },
        }
    });

    // Extract the type name for __repr__ (strip "Py" prefix if present)
    let type_name = py_type.to_string();
    let type_repr_name = type_name.strip_prefix("Py").unwrap_or(&type_name);

    // Generate __repr__ match arms (using pyo3 name if present)
    let repr_arms = variants.iter().map(|v| {
        let ident = v.ident;
        match v.kind {
            VariantKind::Unit => {
                let repr_str = format!("{}.{}", type_repr_name, v.repr_name);
                quote! { #py_type::#ident => #repr_str.to_string() }
            }
            VariantKind::EmptyTuple => {
                let repr_str = format!("{}.{}", type_repr_name, v.repr_name);
                quote! { #py_type::#ident() => #repr_str.to_string() }
            }
            VariantKind::DataTuple => {
                let repr_prefix = format!("{}.{}(", type_repr_name, v.repr_name);
                quote! { #py_type::#ident(v) => format!("{}{})", #repr_prefix, v) }
            }
        }
    });

    // Always generate __repr__ (uses multiple-pymethods feature)
    let pymethods_block = quote! {
        #[pymethods]
        impl #py_type {
            pub fn __repr__(&self) -> String {
                match self {
                    #(#repr_arms,)*
                }
            }
        }
    };

    let expanded = quote! {
        #input

        impl From<#bevy_type> for #py_type {
            fn from(value: #bevy_type) -> Self {
                match value {
                    #(#from_bevy_arms,)*
                }
            }
        }

        impl From<#py_type> for #bevy_type {
            fn from(value: #py_type) -> Self {
                match value {
                    #(#from_py_arms,)*
                }
            }
        }

        #pymethods_block
    };

    TokenStream::from(expanded)
}
