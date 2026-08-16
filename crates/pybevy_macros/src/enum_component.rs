use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Expr, ItemEnum, Lit, Meta, punctuated::Punctuated, token::Comma};

use crate::{
    enum_spec::{
        BevyVariantShape, EnumSpec, FieldSpec, VariantShape, constructor_signature, parameter,
        parameter_name,
    },
    util::to_snake_case,
};

struct PythonClassName {
    module: String,
    name: String,
}

#[derive(Clone, Copy)]
enum StorageBase {
    Component,
    Resource { no_reflect: bool },
}

pub(crate) fn expand(
    input: &ItemEnum,
    spec: &EnumSpec<'_>,
    component_type: &syn::Type,
) -> TokenStream {
    match try_expand(input, spec, component_type, StorageBase::Component) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

pub(crate) fn expand_resource(
    input: &ItemEnum,
    spec: &EnumSpec<'_>,
    resource_type: &syn::Type,
    no_reflect: bool,
) -> TokenStream {
    match try_expand(
        input,
        spec,
        resource_type,
        StorageBase::Resource { no_reflect },
    ) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn try_expand(
    input: &ItemEnum,
    spec: &EnumSpec<'_>,
    inner_type: &syn::Type,
    storage_base: StorageBase,
) -> syn::Result<TokenStream> {
    let class = python_class_name(input)?;
    let py_type = spec.wrapper_name;
    let stem = to_snake_case(
        py_type
            .to_string()
            .strip_prefix("Py")
            .unwrap_or(&py_type.to_string()),
    );
    let initializer = format_ident!("{}_initializer", stem);
    let materialize = format_ident!("materialize_{}", stem);
    let register = format_ident!("register_{}_variants", stem);
    let clone_supported = format_ident!("clone_supported_{}", stem);
    let module = class.module;
    let python_name = class.name;
    let (
        base_type,
        base_initializer,
        storage_type,
        storage_attribute,
        base_options,
        variant_options,
        alias_register,
    ) = match storage_base {
        StorageBase::Component => (
            quote! { pybevy_core::PyComponent },
            quote! { pyo3::PyClassInitializer::from(pybevy_core::PyComponent) },
            quote! { pybevy_core::ComponentStorage },
            quote! { #[pybevy_macros::pycomponent(#inner_type, bridge, materialize = #materialize)] },
            quote! { subclass },
            TokenStream::new(),
            quote! { pybevy_core::registry::global_registry::register_component_bridge_alias },
        ),
        StorageBase::Resource { no_reflect } => (
            quote! { pybevy_core::PyResource },
            quote! {
                pyo3::PyClassInitializer::from(pybevy_core::PyComponent)
                    .add_subclass(pybevy_core::PyResource)
            },
            quote! { pybevy_core::ResourceStorage },
            if no_reflect {
                quote! {
                    #[pybevy_macros::pyresource(
                        #inner_type,
                        no_clone,
                        bridge,
                        materialize = #materialize,
                        clone_with = #clone_supported,
                        no_reflect
                    )]
                }
            } else {
                quote! {
                #[pybevy_macros::pyresource(
                    #inner_type,
                    no_clone,
                    bridge,
                    materialize = #materialize,
                    clone_with = #clone_supported
                )]
                }
            },
            quote! { subclass, frozen },
            quote! { frozen },
            quote! { pybevy_core::registry::global_registry::register_resource_bridge_alias },
        ),
    };

    let mut variant_items = Vec::new();
    let mut register_items = Vec::new();
    let mut aliases = Vec::new();
    let mut discriminator_arms = Vec::new();
    let mut materialize_arms = Vec::new();
    let mut unsupported_type_checks = Vec::new();
    let mut clone_arms = Vec::new();

    for (variant_index, variant) in spec.variants.iter().enumerate() {
        let rust_name = variant.rust_name;
        let variant_name = &variant.python_name;
        let variant_type = format_ident!("{}{}", py_type, rust_name);
        let qualname = format!("{python_name}.{variant_name}");
        let fields = match &variant.shape {
            VariantShape::Struct(fields) => fields.as_slice(),
            VariantShape::EmptyTuple => &[],
            VariantShape::Unit | VariantShape::Tuple(_) => {
                return Err(syn::Error::new_spanned(
                    rust_name,
                    "struct-backed enums require callable empty variants or named payload variants",
                ));
            }
        };
        if let Some(field) = fields.iter().find(|field| field.materialize) {
            return Err(syn::Error::new_spanned(
                field.rust_type,
                "storage-backed enum fields do not support #[py_materialize]",
            ));
        }
        if matches!(storage_base, StorageBase::Resource { .. }) {
            if let Some(field) = fields.iter().find(|field| field.borrowed) {
                return Err(syn::Error::new_spanned(
                    field.rust_type,
                    "resource enum fields cannot borrow mutable payload wrappers",
                ));
            }
            if let Some(field) = fields.iter().find(|field| field.writable) {
                return Err(syn::Error::new_spanned(
                    field.rust_type,
                    "resource enum fields cannot be writable",
                ));
            }
        }

        let materialize_pattern = variant_any_pattern(inner_type, rust_name, variant.bevy_shape);
        discriminator_arms.push(quote! { #materialize_pattern => #variant_index });
        if variant.unsupported {
            if let Some(field) = fields.iter().find(|field| field.writable) {
                return Err(syn::Error::new_spanned(
                    field.rust_type,
                    "unsupported storage-backed enum variants cannot expose writable fields",
                ));
            }
            materialize_arms.push(quote! {
                #variant_index => pyo3::Py::new(py, base)?.into_any()
            });
            unsupported_type_checks.extend(fields.iter().map(|field| {
                let ty = field.rust_type;
                quote! { const _: fn(&#ty) = |_: &#ty| {}; }
            }));
            clone_arms.push(quote! {
                #materialize_pattern => Err(pyo3::exceptions::PyNotImplementedError::new_err(
                    concat!(#python_name, ".", #variant_name, " has no Python value representation")
                ))
            });
            continue;
        }
        clone_arms.push(clone_variant_arm(
            inner_type,
            rust_name,
            variant.bevy_shape,
            fields,
        ));

        let parameters = fields.iter().map(parameter).collect::<Vec<_>>();
        let parameter_names = fields.iter().map(parameter_name).collect::<Vec<_>>();
        let conversions = fields.iter().filter_map(|field| {
            let name = parameter_name(field);
            let stored = field.rust_type;
            let fallible = field.try_into;
            field.python_type.as_ref().map(|_| {
                if fallible {
                    quote! { let #name: #stored = #name.try_into()?; }
                } else {
                    quote! { let #name: #stored = #name.into(); }
                }
            })
        });
        let signature = constructor_signature(fields);
        let match_args_types = fields
            .iter()
            .filter(|field| !field.keyword_only)
            .map(|_| quote! { &'static str });
        let match_args_values = fields
            .iter()
            .filter(|field| !field.keyword_only)
            .map(|field| field.python_name.as_str());
        let match_args_item = if fields.iter().all(|field| field.keyword_only) {
            quote! {
                #[classattr]
                const __match_args__: () = ();
            }
        } else {
            quote! {
                #[classattr]
                fn __match_args__() -> (#(#match_args_types,)*) { (#(#match_args_values,)*) }
            }
        };
        let construct = construct_variant(
            inner_type,
            rust_name,
            variant.bevy_shape,
            fields,
            &parameter_names,
        );
        let ctor_is_fallible = fields.iter().any(|field| field.try_into);
        let (ctor_return, ctor_body) = if ctor_is_fallible {
            (
                quote! { pyo3::PyResult<pyo3::PyClassInitializer<Self>> },
                quote! { Ok(#initializer(#construct).add_subclass(Self)) },
            )
        } else {
            (
                quote! { pyo3::PyClassInitializer<Self> },
                quote! { #initializer(#construct).add_subclass(Self) },
            )
        };

        let getters = fields.iter().enumerate().map(|(field_index, field)| {
            let getter = parameter_name(field);
            let stored_name = field.rust_name.expect("named field");
            let return_type = field.python_type.as_ref().unwrap_or(field.rust_type);
            let pattern = variant_pattern(
                inner_type,
                rust_name,
                variant.bevy_shape,
                fields,
                field_index,
            );
            if field.borrowed {
                // A revalidating wrapper tracks the live component, which may hold a
                // different variant by now; nothing can change it between these two
                // same-call accesses.
                quote! {
                    #[getter]
                    pub fn #getter(slf: pyo3::PyRef<'_, Self>) -> pyo3::PyResult<#return_type> {
                        let base = slf.into_super();
                        if !matches!(&*base.storage.as_ref()?, #materialize_pattern) {
                            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                                pybevy_core::public_error::enum_variant_changed(#qualname),
                            ));
                        }
                        Ok(base.storage.borrow_field_as(|value| match value {
                            #pattern => #stored_name,
                            _ => unreachable!(concat!(#qualname, " changed variant within one call")),
                        })?)
                    }
                }
            } else {
                let value = if let Some(python_type) = &field.python_type {
                    let stored_type = field.rust_type;
                    quote! { <#python_type as From<#stored_type>>::from(#stored_name.clone()) }
                } else {
                    quote! { #stored_name.clone() }
                };
                quote! {
                    #[getter]
                    pub fn #getter(slf: pyo3::PyRef<'_, Self>) -> pyo3::PyResult<#return_type> {
                        let base = slf.into_super();
                        match &*base.storage.as_ref()? {
                            #pattern => Ok(#value),
                            _ => Err(pyo3::exceptions::PyRuntimeError::new_err(
                                pybevy_core::public_error::enum_variant_changed(#qualname),
                            )),
                        }
                    }
                }
            }
        });
        let setters = fields
            .iter()
            .enumerate()
            .filter_map(|(field_index, field)| {
                if !field.writable {
                    return None;
                }
                let getter = parameter_name(field);
                let setter = format_ident!("set_{}", getter);
                let argument_type = field.python_type.as_ref().unwrap_or(field.rust_type);
                let stored_type = field.rust_type;
                let binding = format_ident!("__pybevy_stored_{}", field_index);
                let conversion = if field.python_type.is_some() {
                    if field.try_into {
                        quote! { let value: #stored_type = value.try_into()?; }
                    } else {
                        quote! { let value: #stored_type = value.into(); }
                    }
                } else {
                    TokenStream::new()
                };
                let pattern = variant_binding_pattern(
                    inner_type,
                    rust_name,
                    variant.bevy_shape,
                    fields,
                    field_index,
                    &binding,
                );
                Some(quote! {
                    #[setter]
                    pub fn #setter(
                        slf: pyo3::PyRefMut<'_, Self>,
                        value: #argument_type,
                    ) -> pyo3::PyResult<()> {
                        #conversion
                        let mut base = slf.into_super();
                        // Reject a stale wrapper before as_mut marks the component changed.
                        if !matches!(&*base.storage.as_ref()?, #materialize_pattern) {
                            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                                pybevy_core::public_error::enum_variant_changed(#qualname),
                            ));
                        }
                        match &mut *base.storage.as_mut()? {
                            #pattern => {
                                *#binding = value;
                                Ok(())
                            }
                            _ => unreachable!(concat!(#qualname, " changed variant within one call")),
                        }
                    }
                })
            });

        variant_items.push(quote! {
            #[pyo3::pyclass(name = #variant_name, module = #module, extends = #py_type, #variant_options)]
            pub struct #variant_type;

            #[pyo3::pymethods]
            #[allow(non_upper_case_globals)]
            impl #variant_type {
                #[classattr]
                const __qualname__: &'static str = #qualname;

                #match_args_item

                #[new]
                #signature
                pub fn new(#(#parameters),*) -> #ctor_return {
                    #(#conversions)*
                    #ctor_body
                }

                #(#getters)*
                #(#setters)*
            }
        });
        register_items
            .push(quote! { base.setattr(#variant_name, py.get_type::<#variant_type>())?; });
        aliases.push(quote! { #variant_type::type_object_raw(py) });
        materialize_arms.push(quote! {
            #variant_index => pyo3::Py::new(py, base.add_subclass(#variant_type))?.into_any()
        });
    }

    let clone_supported_impl = matches!(storage_base, StorageBase::Resource { .. }).then(|| {
        quote! {
            fn #clone_supported(value: &#inner_type) -> pyo3::PyResult<#inner_type> {
                match value {
                    #(#clone_arms,)*
                }
            }
        }
    });

    Ok(quote! {
        #(#unsupported_type_checks)*

        #storage_attribute
        #[pyo3::pyclass(name = #python_name, module = #module, extends = #base_type, #base_options)]
        pub struct #py_type {
            pub(crate) storage: #storage_type<#inner_type>,
        }

        #[pyo3::pymethods]
        impl #py_type {
            #[new]
            fn __pybevy_enum_base_new() -> pyo3::PyResult<pyo3::PyClassInitializer<Self>> {
                Err(pyo3::exceptions::PyTypeError::new_err(concat!(
                    #python_name,
                    " is an enum base; construct a nested variant"
                )))
            }
        }

        #(#variant_items)*

        #clone_supported_impl

        pub fn #materialize(
            py: pyo3::Python<'_>,
            storage: #storage_type<#inner_type>,
        ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
            let variant = match &*storage.as_ref()? {
                #(#discriminator_arms,)*
            };
            let base = #base_initializer.add_subclass(#py_type { storage });
            Ok(match variant {
                #(#materialize_arms,)*
                _ => unreachable!("storage-backed enum discriminator is generated exhaustively"),
            })
        }

        fn #initializer(value: #inner_type) -> pyo3::PyClassInitializer<#py_type> {
            #base_initializer.add_subclass(#py_type {
                storage: #storage_type::owned(value),
            })
        }

        pub fn #register(module: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
            use pyo3::PyTypeInfo;
            let py = module.py();
            let base = module.getattr(#python_name)?;
            #(#register_items)*
            let canonical = #py_type::type_object_raw(py);
            for alias in [#(#aliases),*] {
                if !#alias_register(alias, canonical) {
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(concat!(
                        #python_name, " bridge was not registered before its variants"
                    )));
                }
            }
            Ok(())
        }
    })
}

fn construct_variant(
    ty: &syn::Type,
    variant: &syn::Ident,
    shape: BevyVariantShape,
    fields: &[FieldSpec<'_>],
    values: &[syn::Ident],
) -> TokenStream {
    let names = fields
        .iter()
        .map(|field| field.rust_name.expect("named field"))
        .collect::<Vec<_>>();
    match shape {
        BevyVariantShape::Tuple => quote! { #ty::#variant(#(#values),*) },
        BevyVariantShape::Struct => quote! { #ty::#variant { #(#names: #values),* } },
        BevyVariantShape::Unit => quote! { #ty::#variant },
    }
}

fn clone_variant_arm(
    ty: &syn::Type,
    variant: &syn::Ident,
    shape: BevyVariantShape,
    fields: &[FieldSpec<'_>],
) -> TokenStream {
    let bindings = fields
        .iter()
        .enumerate()
        .map(|(index, _)| format_ident!("__pybevy_clone_{index}"))
        .collect::<Vec<_>>();
    let cloned = bindings.iter().map(|binding| quote! { #binding.clone() });
    let names = fields
        .iter()
        .map(|field| field.rust_name.expect("named field"))
        .collect::<Vec<_>>();
    let output_names = names.clone();
    match shape {
        BevyVariantShape::Unit => quote! { #ty::#variant => Ok(#ty::#variant) },
        BevyVariantShape::Tuple => quote! {
            #ty::#variant(#(#bindings),*) => Ok(#ty::#variant(#(#cloned),*))
        },
        BevyVariantShape::Struct => quote! {
            #ty::#variant { #(#names: #bindings),* } => {
                Ok(#ty::#variant { #(#output_names: #cloned),* })
            }
        },
    }
}

fn variant_pattern(
    ty: &syn::Type,
    variant: &syn::Ident,
    shape: BevyVariantShape,
    fields: &[FieldSpec<'_>],
    field_index: usize,
) -> TokenStream {
    let field = fields[field_index].rust_name.expect("named field");
    variant_binding_pattern(ty, variant, shape, fields, field_index, field)
}

fn variant_binding_pattern(
    ty: &syn::Type,
    variant: &syn::Ident,
    shape: BevyVariantShape,
    fields: &[FieldSpec<'_>],
    field_index: usize,
    binding: &syn::Ident,
) -> TokenStream {
    let field = fields[field_index].rust_name.expect("named field");
    match shape {
        BevyVariantShape::Tuple => {
            let bindings = fields.iter().enumerate().map(|(index, _)| {
                if index == field_index {
                    quote! { #binding }
                } else {
                    quote! { _ }
                }
            });
            quote! { #ty::#variant(#(#bindings),*) }
        }
        BevyVariantShape::Struct if field == binding => {
            quote! { #ty::#variant { #field, .. } }
        }
        BevyVariantShape::Struct => quote! { #ty::#variant { #field: #binding, .. } },
        BevyVariantShape::Unit => quote! { #ty::#variant },
    }
}

fn variant_any_pattern(
    ty: &syn::Type,
    variant: &syn::Ident,
    shape: BevyVariantShape,
) -> TokenStream {
    match shape {
        BevyVariantShape::Tuple => quote! { #ty::#variant(..) },
        BevyVariantShape::Struct => quote! { #ty::#variant { .. } },
        BevyVariantShape::Unit => quote! { #ty::#variant },
    }
}

fn python_class_name(input: &ItemEnum) -> syn::Result<PythonClassName> {
    let attr = input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("pyclass"))
        .ok_or_else(|| {
            syn::Error::new_spanned(input, "component pyenum requires #[pyclass(...)]")
        })?;
    let metas = attr.parse_args_with(Punctuated::<Meta, Comma>::parse_terminated)?;
    let mut module = None;
    let mut name = None;
    for meta in metas {
        let Meta::NameValue(meta_value) = meta else {
            return Err(syn::Error::new_spanned(
                meta,
                "component pyenum generates extends and subclass",
            ));
        };
        let Expr::Lit(expr) = meta_value.value else {
            continue;
        };
        let Lit::Str(value) = expr.lit else { continue };
        if meta_value.path.is_ident("module") {
            module = Some(value.value());
        } else if meta_value.path.is_ident("name") {
            name = Some(value.value());
        } else {
            return Err(syn::Error::new_spanned(
                meta_value.path,
                "component pyenum authors may specify only pyclass module and name",
            ));
        }
    }
    Ok(PythonClassName {
        module: module
            .ok_or_else(|| syn::Error::new_spanned(attr, "component pyenum requires module"))?,
        name: name
            .ok_or_else(|| syn::Error::new_spanned(attr, "component pyenum requires name"))?,
    })
}
