use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Expr, ItemEnum, Lit, Meta, punctuated::Punctuated, token::Comma};

use crate::{
    enum_spec::{EnumSpec, VariantShape, constructor_signature, parameter, parameter_name},
    message::generate_message_bridge_tokens,
    util::to_snake_case,
};

struct PythonClassName {
    module: String,
    name: String,
}

pub(crate) fn expand(
    input: &ItemEnum,
    spec: &EnumSpec<'_>,
    message_type: &syn::Type,
    writable: bool,
    generate_bridge: bool,
) -> TokenStream {
    match try_expand(input, spec, message_type, writable, generate_bridge) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn try_expand(
    input: &ItemEnum,
    spec: &EnumSpec<'_>,
    message_type: &syn::Type,
    writable: bool,
    generate_bridge: bool,
) -> syn::Result<TokenStream> {
    let class = python_class_name(input)?;
    let py_type = spec.wrapper_name;
    let inner_type = spec.inner_type;
    let stem = to_snake_case(
        py_type
            .to_string()
            .strip_prefix("Py")
            .unwrap_or(&py_type.to_string()),
    );
    let initializer = format_ident!("{}_initializer", stem);
    let materialize = format_ident!("materialize_{}", stem);
    let materialize_inner = format_ident!("materialize_{}_inner", stem);
    let register = format_ident!("register_{}_variants", stem);
    let module = class.module;
    let python_name = class.name;

    let mut variant_items = Vec::new();
    let mut register_items = Vec::new();
    let mut materialize_arms = Vec::new();
    let mut repr_arms = Vec::new();

    for variant in &spec.variants {
        let rust_name = variant.rust_name;
        let variant_name = &variant.python_name;
        let variant_type = format_ident!("{}{}", py_type, rust_name);
        let qualname = format!("{python_name}.{variant_name}");
        let (fields, unit_variant) = match &variant.shape {
            VariantShape::Struct(fields) => (fields.as_slice(), false),
            VariantShape::Unit | VariantShape::EmptyTuple => (&[][..], true),
            _ => {
                return Err(syn::Error::new_spanned(
                    rust_name,
                    "struct-backed message enums require unit, empty-tuple, or named struct variants",
                ));
            }
        };
        if let Some(field) = fields
            .iter()
            .find(|field| field.try_into || field.materialize || field.borrowed)
        {
            return Err(syn::Error::new_spanned(
                field.rust_type,
                "the PyO3 message emitter does not support this field conversion marker",
            ));
        }

        let parameters = fields.iter().map(parameter).collect::<Vec<_>>();
        let parameter_names = fields.iter().map(parameter_name).collect::<Vec<_>>();
        let conversions = fields.iter().filter_map(|field| {
            let name = parameter_name(field);
            let stored = field.rust_type;
            field
                .python_type
                .as_ref()
                .map(|_| quote! { let #name: #stored = #name.into(); })
        });
        let stored_names = fields
            .iter()
            .map(|field| {
                field
                    .rust_name
                    .expect("struct-backed message fields are named")
            })
            .collect::<Vec<_>>();
        let inner_constructor = if unit_variant {
            quote! { #inner_type::#rust_name }
        } else {
            quote! { #inner_type::#rust_name { #(#stored_names: #parameter_names),* } }
        };
        let inner_match = if unit_variant {
            quote! { #inner_type::#rust_name }
        } else {
            quote! { #inner_type::#rust_name { .. } }
        };
        let signature = constructor_signature(fields);
        let match_args_types = fields
            .iter()
            .filter(|field| !field.keyword_only)
            .map(|_| quote! { &'static str })
            .collect::<Vec<_>>();
        let match_args_values = fields
            .iter()
            .filter(|field| !field.keyword_only)
            .map(|field| field.python_name.as_str())
            .collect::<Vec<_>>();
        let match_args = if match_args_values.is_empty() {
            quote! {
                #[classattr]
                const __match_args__: () = ();
            }
        } else {
            quote! {
                #[classattr]
                fn __match_args__() -> (#(#match_args_types,)*) {
                    (#(#match_args_values,)*)
                }
            }
        };

        let getters = fields.iter().map(|field| {
            let getter = parameter_name(field);
            let stored_name = field
                .rust_name
                .expect("struct-backed message fields are named");
            let return_type = field.python_type.as_ref().unwrap_or(field.rust_type);
            // The `From` source is the stored type, not the public return type.
            let value = if let Some(python_type) = &field.python_type {
                let stored_type = field.rust_type;
                quote! { <#python_type as From<#stored_type>>::from(#stored_name.clone()) }
            } else {
                quote! { #stored_name.clone() }
            };
            quote! {
                #[getter]
                pub fn #getter(slf: pyo3::PyRef<'_, Self>) -> #return_type {
                    let base = slf.into_super();
                    match &base.inner {
                        #inner_type::#rust_name { #stored_name, .. } => #value,
                        _ => unreachable!(concat!(#qualname, " instance changed discriminant")),
                    }
                }
            }
        });

        variant_items.push(quote! {
            #[pyo3::pyclass(name = #variant_name, module = #module, extends = #py_type, frozen)]
            pub struct #variant_type;

            #[pyo3::pymethods]
            #[allow(non_upper_case_globals)]
            impl #variant_type {
                #[classattr]
                const __qualname__: &'static str = #qualname;

                #match_args

                #[new]
                #signature
                pub fn new(#(#parameters),*) -> pyo3::PyClassInitializer<Self> {
                    #(#conversions)*
                    #initializer(#inner_constructor)
                        .add_subclass(Self)
                }

                #(#getters)*
            }
        });

        register_items.push(quote! {
            base.setattr(#variant_name, py.get_type::<#variant_type>())?;
        });
        materialize_arms.push(quote! {
            #inner_match => {
                pyo3::Py::new(py, base.add_subclass(#variant_type))?.into_any()
            }
        });

        let labels = fields.iter().map(|field| field.python_name.as_str());
        let names = fields
            .iter()
            .map(|field| {
                field
                    .rust_name
                    .expect("struct-backed message fields are named")
            })
            .collect::<Vec<_>>();
        let repr_names = names.clone();
        let prefix = format!("{python_name}.{variant_name}(");
        let repr_match = if unit_variant {
            quote! { #inner_type::#rust_name }
        } else {
            quote! { #inner_type::#rust_name { #(#names),* } }
        };
        repr_arms.push(quote! {
            #repr_match => {
                let fields: Vec<String> = vec![#(format!("{}={:?}", #labels, #repr_names)),*];
                format!("{}{})", #prefix, fields.join(", "))
            }
        });
    }

    let materialize_path: syn::Path = syn::parse_quote!(#materialize);
    let bridge = generate_bridge.then(|| {
        generate_message_bridge_tokens(
            message_type,
            py_type,
            None,
            writable,
            Some(&materialize_path),
            true,
        )
    });

    let materialized_inner = if quote!(#inner_type).to_string() == quote!(#message_type).to_string()
    {
        quote! { event.clone() }
    } else {
        quote! { <#inner_type as From<&#message_type>>::from(event) }
    };

    Ok(quote! {
        #[pyo3::pyclass(
            name = #python_name,
            module = #module,
            extends = pybevy_core::PyMessage,
            subclass,
            frozen,
            eq,
            skip_from_py_object
        )]
        #[derive(Debug, Clone, PartialEq)]
        pub struct #py_type {
            inner: #inner_type,
        }

        #[pyo3::pymethods]
        impl #py_type {
            pub fn __repr__(&self) -> String {
                match &self.inner {
                    #(#repr_arms,)*
                }
            }

            pub fn __copy__(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                #materialize_inner(py, self.inner.clone())
            }
        }

        impl From<#py_type> for #inner_type {
            fn from(value: #py_type) -> Self {
                value.inner
            }
        }

        #(#variant_items)*

        pub fn #materialize(
            py: pyo3::Python<'_>,
            event: &#message_type,
        ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
            let inner = #materialized_inner;
            #materialize_inner(py, inner)
        }

        fn #materialize_inner(
            py: pyo3::Python<'_>,
            inner: #inner_type,
        ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
            let base = #initializer(inner.clone());
            Ok(match &inner {
                #(#materialize_arms,)*
            })
        }

        fn #initializer(event: #inner_type) -> pyo3::PyClassInitializer<#py_type> {
            pyo3::PyClassInitializer::from(pybevy_core::PyMessage)
                .add_subclass(#py_type { inner: event })
        }

        pub fn #register(module: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
            let py = module.py();
            let base = module.getattr(#python_name)?;
            #(#register_items)*
            Ok(())
        }

        #bridge
    })
}

fn python_class_name(input: &ItemEnum) -> syn::Result<PythonClassName> {
    let attr = input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("pyclass"))
        .ok_or_else(|| syn::Error::new_spanned(input, "message pyenum requires #[pyclass(...)]"))?;
    let metas = attr.parse_args_with(Punctuated::<Meta, Comma>::parse_terminated)?;
    let mut module = None;
    let mut name = None;
    for meta in metas {
        if let Meta::NameValue(meta_value) = meta {
            let path = meta_value.path;
            let Expr::Lit(expr) = meta_value.value else {
                continue;
            };
            let Lit::Str(value) = expr.lit else { continue };
            if path.is_ident("module") {
                module = Some(value.value());
            } else if path.is_ident("name") {
                name = Some(value.value());
            } else {
                return Err(syn::Error::new_spanned(
                    path,
                    "message pyenum authors may specify only pyclass module and name",
                ));
            }
        } else {
            return Err(syn::Error::new_spanned(
                meta,
                "message pyenum generates extends, subclass, frozen, eq, and skip_from_py_object",
            ));
        }
    }
    Ok(PythonClassName {
        module: module.ok_or_else(|| {
            syn::Error::new_spanned(attr, "message pyenum requires pyclass module = \"...\"")
        })?,
        name: name.ok_or_else(|| {
            syn::Error::new_spanned(attr, "message pyenum requires pyclass name = \"...\"")
        })?,
    })
}
