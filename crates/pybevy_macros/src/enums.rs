use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, Item, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

use crate::enum_spec::{EnumSpec, VariantShape};

/// Arguments for pyenum attribute macro
struct BevyEnumArgs {
    /// The Bevy enum type to convert to/from
    bevy_type: Type,
    /// If true, map empty tuple variants Variant() to Bevy's unit variants Variant
    empty_tuple: bool,
    /// Only declare the Bevy enum relationship; the adapter supplies its own implementation.
    manual: bool,
}

impl Parse for BevyEnumArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let bevy_type: Type = input.parse()?;

        let mut empty_tuple = false;
        let mut manual = false;
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let option: Ident = input.parse()?;
            match option.to_string().as_str() {
                "empty_tuple" => empty_tuple = true,
                "manual" => manual = true,
                other => {
                    return Err(syn::Error::new_spanned(
                        option,
                        format!(
                            "unknown option '{}', expected: empty_tuple or manual",
                            other
                        ),
                    ));
                }
            }
        }

        Ok(BevyEnumArgs {
            bevy_type,
            empty_tuple,
            manual,
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
/// ```rust,ignore
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
/// Variant kind for pyenum processing
enum VariantKind {
    /// Unit variant: `Variant`
    Unit,
    /// Empty tuple variant: `Variant()` - maps to Bevy's unit variant
    EmptyTuple,
    /// Single-field tuple variant: `Variant(T)` - passes through the value
    DataTuple {
        /// Field is `Option<T>`; repr prints the inner value or `None`
        is_option: bool,
    },
    /// Single named Python field mapped to a single-field Bevy tuple variant.
    DataStruct {
        field: Ident,
        /// Field is `Option<T>`; repr prints the inner value or `None`
        is_option: bool,
    },
}

pub fn pyenum(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as BevyEnumArgs);
    let input = parse_macro_input!(item as Item);

    // Complex enum adapters may need handwritten conversions, nested Python variant
    // registration, or storage that the generated implementation cannot express. In
    // that case the attribute remains as machine-readable contract metadata for
    // pybevy_lint and deliberately leaves the item unchanged.
    if args.manual {
        return quote!(#input).into();
    }

    let Item::Enum(input) = input else {
        return syn::Error::new_spanned(
            input,
            "pyenum requires an enum; use `manual` for a handwritten enum adapter",
        )
        .to_compile_error()
        .into();
    };

    let py_type = &input.ident;
    let bevy_type = &args.bevy_type;
    let spec = match EnumSpec::parse(&input, bevy_type) {
        Ok(spec) => spec,
        Err(error) => return error.to_compile_error().into(),
    };
    debug_assert_eq!(spec.wrapper_name, py_type);
    let spec_inner_type = spec.inner_type;
    debug_assert_eq!(
        quote!(#bevy_type).to_string(),
        quote!(#spec_inner_type).to_string()
    );

    // Collect variant info (name, optional pyo3 rename, and variant kind)
    struct VariantInfo<'a> {
        ident: &'a Ident,
        repr_name: String,
        kind: VariantKind,
    }

    let mut variants: Vec<VariantInfo> = Vec::new();
    for variant in &spec.variants {
        let kind = match &variant.shape {
            VariantShape::Unit => Some(VariantKind::Unit),
            VariantShape::EmptyTuple if args.empty_tuple => Some(VariantKind::EmptyTuple),
            VariantShape::Tuple(fields) if fields.len() == 1 => Some(VariantKind::DataTuple {
                is_option: is_option(fields[0].rust_type),
            }),
            VariantShape::Struct(fields) if fields.len() == 1 => Some(VariantKind::DataStruct {
                field: fields[0]
                    .rust_name
                    .expect("struct fields have Rust names")
                    .clone(),
                is_option: is_option(fields[0].rust_type),
            }),
            _ => None,
        };

        match kind {
            Some(k) => {
                variants.push(VariantInfo {
                    ident: variant.rust_name,
                    repr_name: variant.python_name.clone(),
                    kind: k,
                });
            }
            None => {
                let msg = if args.empty_tuple {
                    "pyenum only supports unit, empty tuple, and single-field tuple or struct variants"
                } else {
                    "pyenum only supports unit and single-field tuple or struct variants (use empty_tuple for Variant() style)"
                };
                return syn::Error::new_spanned(variant.rust_name, msg)
                    .to_compile_error()
                    .into();
            }
        }
    }

    // Generate From<BevyType> for PyType
    let from_bevy_arms = variants.iter().map(|v| {
        let name = v.ident;
        match &v.kind {
            VariantKind::Unit => quote! { #bevy_type::#name => #py_type::#name },
            VariantKind::EmptyTuple => quote! { #bevy_type::#name => #py_type::#name() },
            VariantKind::DataTuple { .. } => quote! { #bevy_type::#name(v) => #py_type::#name(v) },
            VariantKind::DataStruct { field, .. } => {
                quote! { #bevy_type::#name(v) => #py_type::#name { #field: v } }
            }
        }
    });

    // Generate From<PyType> for BevyType
    let from_py_arms = variants.iter().map(|v| {
        let name = v.ident;
        match &v.kind {
            VariantKind::Unit => quote! { #py_type::#name => #bevy_type::#name },
            VariantKind::EmptyTuple => quote! { #py_type::#name() => #bevy_type::#name },
            VariantKind::DataTuple { .. } => quote! { #py_type::#name(v) => #bevy_type::#name(v) },
            VariantKind::DataStruct { field, .. } => {
                quote! { #py_type::#name { #field: v } => #bevy_type::#name(v) }
            }
        }
    });

    // Extract the type name for __repr__ (strip "Py" prefix if present)
    let type_name = py_type.to_string();
    let type_repr_name = type_name.strip_prefix("Py").unwrap_or(&type_name);

    // Generate __repr__ match arms (using pyo3 name if present)
    let repr_arms = variants.iter().map(|v| {
        let ident = v.ident;
        match &v.kind {
            VariantKind::Unit => {
                let repr_str = format!("{}.{}", type_repr_name, v.repr_name);
                quote! { #py_type::#ident => #repr_str.to_string() }
            }
            VariantKind::EmptyTuple => {
                let repr_str = format!("{}.{}", type_repr_name, v.repr_name);
                quote! { #py_type::#ident() => #repr_str.to_string() }
            }
            VariantKind::DataTuple { is_option } => {
                let repr_prefix = format!("{}.{}(", type_repr_name, v.repr_name);
                if *is_option {
                    quote! {
                        #py_type::#ident(Some(v)) => format!("{}{})", #repr_prefix, v),
                        #py_type::#ident(None) => format!("{}None)", #repr_prefix)
                    }
                } else {
                    quote! { #py_type::#ident(v) => format!("{}{})", #repr_prefix, v) }
                }
            }
            VariantKind::DataStruct { field, is_option } => {
                let repr_prefix = format!("{}.{}(", type_repr_name, v.repr_name);
                if *is_option {
                    quote! {
                        #py_type::#ident { #field: Some(v) } => format!("{}{})", #repr_prefix, v),
                        #py_type::#ident { #field: None } => format!("{}None)", #repr_prefix)
                    }
                } else {
                    quote! {
                        #py_type::#ident { #field: v } => format!("{}{})", #repr_prefix, v)
                    }
                }
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

            pub fn __copy__(&self) -> Self {
                self.clone()
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

fn is_option(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Path(path)
            if path.path.segments.last().is_some_and(|segment| segment.ident == "Option")
    )
}
