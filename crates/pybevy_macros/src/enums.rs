use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Ident, Item, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

use crate::enum_spec::{BevyVariantShape, EnumSpec, FieldSpec, VariantShape};

/// Arguments for pyenum attribute macro
pub(crate) struct BevyEnumArgs {
    /// The Bevy enum type to convert to/from
    bevy_type: Type,
    /// If true, map empty tuple variants Variant() to Bevy's unit variants Variant
    empty_tuple: bool,
    /// If true, include `()` in generated reprs for empty tuple variants.
    unit_parens: bool,
    /// Preserve an adapter-supplied or PyO3-default representation.
    no_repr: bool,
    /// Emit a struct-backed hierarchy extending PyComponent.
    component: bool,
    /// Emit a struct-backed hierarchy extending PyResource.
    resource: bool,
    /// Skip Bevy reflection registration for a generated storage-backed enum.
    no_reflect: bool,
    /// Only declare the Bevy enum relationship; the adapter supplies its own implementation.
    manual: bool,
    /// Emit a struct-backed hierarchy extending PyMessage.
    message: bool,
    /// Allow generated message variants to be written through MessageWriter.
    writable: bool,
    /// Skip the ordinary message bridge when routing is handled by a reviewed special channel.
    no_bridge: bool,
    /// Typed, loss-explicit storage mirror for a message enum.
    mirror: Option<Type>,
}

impl Parse for BevyEnumArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let bevy_type: Type = input.parse()?;

        let mut empty_tuple = false;
        let mut unit_parens = false;
        let mut no_repr = false;
        let mut component = false;
        let mut resource = false;
        let mut no_reflect = false;
        let mut manual = false;
        let mut message = false;
        let mut writable = false;
        let mut no_bridge = false;
        let mut mirror = None;
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let option: Ident = input.parse()?;
            match option.to_string().as_str() {
                "empty_tuple" => empty_tuple = true,
                "unit_parens" => unit_parens = true,
                "no_repr" => no_repr = true,
                "component" => component = true,
                "resource" => resource = true,
                "no_reflect" => no_reflect = true,
                "manual" => manual = true,
                "message" => message = true,
                "writable" => writable = true,
                "no_bridge" => no_bridge = true,
                "mirror" => {
                    input.parse::<Token![=]>()?;
                    mirror = Some(input.parse()?);
                }
                other => {
                    return Err(syn::Error::new_spanned(
                        option,
                        format!(
                            "unknown option '{}', expected: empty_tuple, unit_parens, no_repr, no_reflect, manual, message, writable, no_bridge, component, resource, or mirror = Type",
                            other
                        ),
                    ));
                }
            }
        }

        if unit_parens && !empty_tuple {
            return Err(syn::Error::new_spanned(
                bevy_type,
                "unit_parens requires empty_tuple",
            ));
        }
        if [manual, message, component, resource]
            .into_iter()
            .filter(|enabled| *enabled)
            .count()
            > 1
        {
            return Err(syn::Error::new_spanned(
                bevy_type,
                "manual, message, component, and resource are mutually exclusive pyenum modes",
            ));
        }
        if no_repr && message {
            return Err(syn::Error::new_spanned(
                bevy_type,
                "no_repr is available only for native pyclass enum generation",
            ));
        }
        if mirror.is_some() && !message {
            return Err(syn::Error::new_spanned(
                bevy_type,
                "mirror requires the struct-backed message pyenum mode",
            ));
        }
        if writable && !message {
            return Err(syn::Error::new_spanned(
                bevy_type,
                "writable requires the struct-backed message pyenum mode",
            ));
        }
        if no_bridge && !message {
            return Err(syn::Error::new_spanned(
                bevy_type,
                "no_bridge requires the struct-backed message pyenum mode",
            ));
        }
        if no_bridge && writable {
            return Err(syn::Error::new_spanned(
                bevy_type,
                "a no_bridge message cannot request generated writing",
            ));
        }
        if no_reflect && !resource {
            return Err(syn::Error::new_spanned(
                bevy_type,
                "no_reflect currently requires the struct-backed resource pyenum mode",
            ));
        }

        Ok(BevyEnumArgs {
            bevy_type,
            empty_tuple,
            unit_parens,
            no_repr,
            component,
            resource,
            no_reflect,
            manual,
            message,
            writable,
            no_bridge,
            mirror,
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
/// Generated expansion supports unit variants, empty tuple variants, and named
/// struct variants with any number of fields. Use `#[py_bevy(tuple)]` on a
/// named adapter variant when its fields map by declaration order to a Bevy
/// tuple variant. Handwritten adapter logic remains appropriate when field
/// conversion is fallible, borrowed, or otherwise not a direct assignment.
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
enum VariantKind<'a> {
    /// Unit variant: `Variant`
    Unit,
    /// Empty tuple variant: `Variant()` - maps to Bevy's unit variant
    EmptyTuple,
    /// Single-field tuple variant: `Variant(T)` - passes through the value.
    /// Multi-field tuple adapters are rejected because PyO3 exposes `_0`, `_1`,
    /// and so on; use a named struct adapter plus `#[py_bevy(tuple)]` instead.
    DataTuple {
        field: &'a FieldSpec<'a>,
        /// Field is `Option<T>`; repr prints the inner value or `None`
        is_option: bool,
    },
    /// Named Python fields mapped to either a Bevy struct or tuple variant.
    DataStruct {
        fields: &'a [FieldSpec<'a>],
        bevy_shape: BevyVariantShape,
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
    let stored_type = args.mirror.as_ref().unwrap_or(bevy_type);
    let spec = match EnumSpec::parse(&input, stored_type) {
        Ok(spec) => spec,
        Err(error) => return error.to_compile_error().into(),
    };
    if args.message {
        return crate::enum_message::expand(
            &input,
            &spec,
            bevy_type,
            args.writable,
            !args.no_bridge,
        )
        .into();
    }
    if args.component {
        return crate::enum_component::expand(&input, &spec, bevy_type).into();
    }
    if args.resource {
        return crate::enum_component::expand_resource(&input, &spec, bevy_type, args.no_reflect)
            .into();
    }
    if let Some(field) = spec
        .variants
        .iter()
        .find_map(|variant| match &variant.shape {
            VariantShape::Tuple(fields) | VariantShape::Struct(fields) => {
                fields.iter().find(|field| field.materialize)
            }
            VariantShape::Unit | VariantShape::EmptyTuple => None,
        })
    {
        return syn::Error::new_spanned(
            field.rust_type,
            "#[py_materialize] currently requires the struct-backed message pyenum mode",
        )
        .to_compile_error()
        .into();
    }
    if let Some(field) = spec
        .variants
        .iter()
        .find_map(|variant| match &variant.shape {
            VariantShape::Tuple(fields) | VariantShape::Struct(fields) => {
                fields.iter().find(|field| field.borrowed)
            }
            VariantShape::Unit | VariantShape::EmptyTuple => None,
        })
    {
        return syn::Error::new_spanned(
            field.rust_type,
            "#[py_borrow] requires the struct-backed component pyenum mode",
        )
        .to_compile_error()
        .into();
    }
    debug_assert_eq!(spec.wrapper_name, py_type);
    let spec_inner_type = spec.inner_type;
    debug_assert_eq!(
        quote!(#stored_type).to_string(),
        quote!(#spec_inner_type).to_string()
    );

    // Collect variant info (name, optional pyo3 rename, and variant kind)
    struct VariantInfo<'a> {
        ident: &'a Ident,
        repr_name: String,
        kind: VariantKind<'a>,
    }

    let mut variants: Vec<VariantInfo> = Vec::new();
    for variant in &spec.variants {
        let kind = match &variant.shape {
            VariantShape::Unit => Some(VariantKind::Unit),
            VariantShape::EmptyTuple if args.empty_tuple => Some(VariantKind::EmptyTuple),
            VariantShape::Tuple(fields) if fields.len() == 1 => Some(VariantKind::DataTuple {
                field: &fields[0],
                is_option: is_option(fields[0].rust_type),
            }),
            VariantShape::Struct(fields) => Some(VariantKind::DataStruct {
                fields,
                bevy_shape: variant.bevy_shape,
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
                let msg = if matches!(variant.shape, VariantShape::Tuple(_)) {
                    "multi-field tuple adapters expose unstable _0/_1 names in PyO3; use a named struct adapter with #[py_bevy(tuple)]"
                } else if args.empty_tuple {
                    "pyenum supports unit, empty tuple, single-field tuple, and named struct variants"
                } else {
                    "pyenum supports unit, single-field tuple, and named struct variants (use empty_tuple for Variant() style)"
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
            VariantKind::DataTuple { field, .. } => {
                let value = from_bevy_field_value(field, quote!(v));
                quote! { #bevy_type::#name(v) => #py_type::#name(#value) }
            }
            VariantKind::DataStruct { fields, bevy_shape } => {
                let names = struct_field_names(fields);
                let bindings = field_bindings(fields);
                let values = fields
                    .iter()
                    .zip(&bindings)
                    .map(|(field, binding)| from_bevy_field_value(field, quote!(#binding)))
                    .collect::<Vec<_>>();
                match bevy_shape {
                    BevyVariantShape::Tuple => {
                        quote! {
                            #bevy_type::#name(#(#bindings),*) =>
                                #py_type::#name { #(#names: #values),* }
                        }
                    }
                    BevyVariantShape::Struct => {
                        quote! {
                            #bevy_type::#name { #(#names: #bindings),* } =>
                                #py_type::#name { #(#names: #values),* }
                        }
                    }
                    BevyVariantShape::Unit => unreachable!("payload variant cannot map to unit"),
                }
            }
        }
    });

    // A fallible #[py_type] field forces the whole Py -> Bevy conversion to be
    // TryFrom: an infallible From cannot report an expired borrow.
    let py_to_bevy_is_fallible = variants.iter().any(|v| match &v.kind {
        VariantKind::DataTuple { field, .. } => field.try_into,
        VariantKind::DataStruct { fields, .. } => fields.iter().any(|f| f.try_into),
        VariantKind::Unit | VariantKind::EmptyTuple => false,
    });

    // Generate From<PyType> for BevyType
    let from_py_arms = variants.iter().map(|v| {
        let name = v.ident;
        match &v.kind {
            VariantKind::Unit => quote! { #py_type::#name => #bevy_type::#name },
            VariantKind::EmptyTuple => quote! { #py_type::#name() => #bevy_type::#name },
            VariantKind::DataTuple { field, .. } => {
                let value = from_python_field_value(field, quote!(v));
                quote! { #py_type::#name(v) => #bevy_type::#name(#value) }
            }
            VariantKind::DataStruct { fields, bevy_shape } => {
                let names = struct_field_names(fields);
                let bindings = field_bindings(fields);
                let values = fields
                    .iter()
                    .zip(&bindings)
                    .map(|(field, binding)| from_python_field_value(field, quote!(#binding)))
                    .collect::<Vec<_>>();
                match bevy_shape {
                    BevyVariantShape::Tuple => {
                        quote! {
                            #py_type::#name { #(#names: #bindings),* } =>
                                #bevy_type::#name(#(#values),*)
                        }
                    }
                    BevyVariantShape::Struct => {
                        quote! {
                            #py_type::#name { #(#names: #bindings),* } =>
                                #bevy_type::#name { #(#names: #values),* }
                        }
                    }
                    BevyVariantShape::Unit => unreachable!("payload variant cannot map to unit"),
                }
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
                let suffix = if args.unit_parens { "()" } else { "" };
                let repr_str = format!("{}.{}{}", type_repr_name, v.repr_name, suffix);
                quote! { #py_type::#ident() => #repr_str.to_string() }
            }
            VariantKind::DataTuple { is_option, .. } => {
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
            VariantKind::DataStruct { fields, bevy_shape } => {
                let repr_prefix = format!("{}.{}(", type_repr_name, v.repr_name);
                let names = struct_field_names(fields);
                if fields.len() == 1 && *bevy_shape == BevyVariantShape::Tuple {
                    let field = names[0];
                    if is_option(fields[0].rust_type) {
                        quote! {
                            #py_type::#ident { #field: Some(v) } => format!("{}{})", #repr_prefix, v),
                            #py_type::#ident { #field: None } => format!("{}None)", #repr_prefix)
                        }
                    } else {
                        quote! {
                            #py_type::#ident { #field: v } => format!("{}{})", #repr_prefix, v)
                        }
                    }
                } else if fields.is_empty() {
                    quote! {
                        #py_type::#ident {} => format!("{})", #repr_prefix)
                    }
                } else {
                    let labels = fields.iter().map(|field| field.python_name.as_str());
                    quote! {
                        #py_type::#ident { #(#names),* } => {
                            let fields = [#(format!("{}={:?}", #labels, #names)),*];
                            format!("{}{})", #repr_prefix, fields.join(", "))
                        }
                    }
                }
            }
        }
    });

    // Generate the standard value protocols in a separate pymethods block.
    let repr_method = (!args.no_repr).then(|| {
        quote! {
            pub fn __repr__(&self) -> String {
                match self {
                    #(#repr_arms,)*
                }
            }
        }
    });
    let pymethods_block = quote! {
        #[pymethods]
        impl #py_type {
            #repr_method

            pub fn __copy__(&self) -> Self {
                self.clone()
            }
        }
    };

    let mut emitted_input = input.clone();
    for (variant, variant_spec) in emitted_input.variants.iter_mut().zip(&spec.variants) {
        variant
            .attrs
            .retain(|attr| !attr.path().is_ident("py_bevy"));
        let field_specs = match &variant_spec.shape {
            VariantShape::Tuple(fields) | VariantShape::Struct(fields) => fields.as_slice(),
            VariantShape::Unit | VariantShape::EmptyTuple => &[],
        };
        for (field, field_spec) in variant.fields.iter_mut().zip(field_specs) {
            if let Some(python_type) = &field_spec.python_type {
                field.ty = python_type.clone();
            }
            field.attrs.retain(|attr| {
                !matches!(
                    attr.path().get_ident().map(ToString::to_string).as_deref(),
                    Some("py_field" | "py_default" | "py_kw_only" | "py_type")
                        | Some("py_try_into" | "py_materialize" | "py_borrow")
                )
            });
        }
    }

    let py_to_bevy_impl = if py_to_bevy_is_fallible {
        quote! {
            impl TryFrom<#py_type> for #bevy_type {
                type Error = PyErr;

                fn try_from(value: #py_type) -> PyResult<Self> {
                    Ok(match value {
                        #(#from_py_arms,)*
                    })
                }
            }
        }
    } else {
        quote! {
            impl From<#py_type> for #bevy_type {
                fn from(value: #py_type) -> Self {
                    match value {
                        #(#from_py_arms,)*
                    }
                }
            }
        }
    };

    let expanded = quote! {
        #emitted_input

        impl From<#bevy_type> for #py_type {
            fn from(value: #bevy_type) -> Self {
                match value {
                    #(#from_bevy_arms,)*
                }
            }
        }

        #py_to_bevy_impl

        #pymethods_block
    };

    TokenStream::from(expanded)
}

fn from_bevy_field_value(
    field: &FieldSpec<'_>,
    value: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    match &field.python_type {
        Some(python_type) => {
            let rust_type = field.rust_type;
            quote! { <#python_type as From<#rust_type>>::from(#value) }
        }
        None => value,
    }
}

fn from_python_field_value(
    field: &FieldSpec<'_>,
    value: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    match &field.python_type {
        Some(python_type) => {
            let rust_type = field.rust_type;
            if field.try_into {
                quote! { <#rust_type as TryFrom<#python_type>>::try_from(#value)? }
            } else {
                quote! { <#rust_type as From<#python_type>>::from(#value) }
            }
        }
        None => value,
    }
}

fn struct_field_names<'a>(fields: &'a [FieldSpec<'a>]) -> Vec<&'a Ident> {
    fields
        .iter()
        .map(|field| field.rust_name.expect("struct fields have Rust names"))
        .collect()
}

fn field_bindings(fields: &[FieldSpec<'_>]) -> Vec<Ident> {
    fields
        .iter()
        .map(|field| format_ident!("__pybevy_field_{}", field.declaration_index))
        .collect()
}

fn is_option(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Path(path)
            if path.path.segments.last().is_some_and(|segment| segment.ident == "Option")
    )
}
