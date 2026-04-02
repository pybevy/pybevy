use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, ItemStruct, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

/// Generates storage boilerplate for newtype component wrappers in feature crates.
///
/// Unlike `native_component(X, newtype)`, this macro does NOT generate `NativeComponent` impl,
/// making it usable in feature crates that don't have access to `PyComponentType`.
///
/// This macro generates:
/// - `impl From<BevyType> for PyType`
/// - `impl TryFrom<PyType> for BevyType`
/// - `impl TryFrom<&BevyType> for PyType`
/// - Helper method: `from_owned()`
///
/// # Usage in feature crates
///
/// ```rust
/// #[newtype_storage(Tonemapping)]
/// #[pyclass(name = "Tonemapping", extends = PyComponent, frozen)]
/// #[derive(Clone, Debug)]
/// pub struct PyTonemapping(pub(crate) Tonemapping);
/// ```
///
/// With bridge generation:
///
/// ```rust
/// #[newtype_storage(Tonemapping, bridge)]
/// #[newtype_storage(Msaa, bridge, copy)]
/// ```
pub fn newtype_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    struct NewtypeStorageArgs {
        bevy_type: Type,
        bridge: bool,
        bridge_name: Option<String>,
        is_copy: bool,
    }

    impl Parse for NewtypeStorageArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            let mut bridge = false;
            let mut bridge_name = None;
            let mut is_copy = false;

            while input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                if !input.peek(syn::Ident) && !input.peek(syn::LitStr) {
                    break;
                }
                if input.peek(syn::LitStr) {
                    bridge_name = Some(input.parse::<syn::LitStr>()?.value());
                } else {
                    let ident: Ident = input.parse()?;
                    match ident.to_string().as_str() {
                        "bridge" => bridge = true,
                        "copy" => is_copy = true,
                        other => {
                            return Err(syn::Error::new_spanned(
                                ident,
                                format!(
                                    "unknown option '{}', expected one of: bridge, copy",
                                    other
                                ),
                            ));
                        }
                    }
                }
            }

            Ok(NewtypeStorageArgs {
                bevy_type,
                bridge,
                bridge_name,
                is_copy,
            })
        }
    }

    let args = parse_macro_input!(attr as NewtypeStorageArgs);
    let bevy_type = &args.bevy_type;
    let input = parse_macro_input!(item as ItemStruct);
    let py_type = &input.ident;

    let bridge_tokens = if args.bridge {
        generate_newtype_bridge_tokens(
            bevy_type,
            py_type,
            args.bridge_name.as_deref(),
            args.is_copy,
        )
    } else {
        quote! {}
    };

    let expanded = quote! {
        #input

        impl From<#bevy_type> for #py_type {
            fn from(value: #bevy_type) -> Self {
                Self(value)
            }
        }

        impl TryFrom<#py_type> for #bevy_type {
            type Error = PyErr;

            fn try_from(value: #py_type) -> PyResult<Self> {
                Ok(value.0)
            }
        }

        impl TryFrom<&#bevy_type> for #py_type {
            type Error = PyErr;

            fn try_from(value: &#bevy_type) -> PyResult<Self> {
                Ok(Self(value.clone()))
            }
        }

        impl #py_type {
            /// Create from an owned component value. Returns tuple for PyO3 class inheritance.
            pub fn from_owned(value: #bevy_type) -> (Self, pybevy_core::PyComponent) {
                (Self(value), pybevy_core::PyComponent)
            }
        }

        #bridge_tokens
    };

    TokenStream::from(expanded)
}

/// Shared newtype bridge code generation used by `#[newtype_storage(..., bridge)]`.
pub(crate) fn generate_newtype_bridge_tokens(
    bevy_type: &Type,
    py_type: &Ident,
    bridge_name_override: Option<&str>,
    is_copy: bool,
) -> proc_macro2::TokenStream {
    // Derive bridge name: either from explicit string or from py_type (strip "Py" prefix)
    let bridge_name_str = bridge_name_override.map(String::from).unwrap_or_else(|| {
        let name = py_type.to_string();
        name.strip_prefix("Py").unwrap_or(&name).to_string()
    });
    let bridge_name = quote::format_ident!("{}Bridge", bridge_name_str);
    let component_name = &bridge_name_str;

    // Clone vs Copy for extraction
    let clone_expr = if is_copy {
        quote! { *component }
    } else {
        quote! { component.clone() }
    };

    let expanded = quote! {
        /// Bridge for #component_name newtype component
        pub struct #bridge_name;

        impl pybevy_core::ComponentBridge for #bridge_name {
            fn bevy_type_id(&self) -> std::any::TypeId {
                std::any::TypeId::of::<#bevy_type>()
            }

            fn py_type_ptr(&self) -> *const pyo3::ffi::PyTypeObject {
                pyo3::Python::attach(|py| {
                    <#py_type as pyo3::PyTypeInfo>::type_object(py).as_type_ptr()
                })
            }

            fn py_type<'py>(&self, py: pyo3::Python<'py>) -> pyo3::Bound<'py, pyo3::types::PyType> {
                <#py_type as pyo3::PyTypeInfo>::type_object(py)
            }

            fn name(&self) -> &'static str {
                #component_name
            }

            fn register(&self, world: &mut bevy::ecs::world::World) -> bevy::ecs::component::ComponentId {
                world.register_component::<#bevy_type>()
            }

            #[inline(always)]
            fn extract(
                &self,
                entity: &mut bevy::ecs::world::FilteredEntityMut,
                component_id: bevy::ecs::component::ComponentId,
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#component_name, " not found"))
                })?;

                // SAFETY: component_id was registered for #bevy_type, so the untyped pointer points to a valid instance.
                let component = unsafe {
                    untyped.deref::<#bevy_type>()
                };

                // Newtype: clone/copy the value (no borrowed storage for simple wrappers)
                let obj = pyo3::Py::new(py, #py_type::from_owned(#clone_expr))?;
                Ok(obj.into_any())
            }

            fn insert(
                &self,
                world: &mut bevy::ecs::world::World,
                entity: bevy::ecs::entity::Entity,
                component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                let native: #bevy_type = py_component.0.clone();

                world.entity_mut(entity).insert(native);
                Ok(())
            }

            fn insert_into_entity(
                &self,
                entity: &mut bevy::ecs::world::EntityWorldMut,
                component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                let native: #bevy_type = py_component.0.clone();

                entity.insert(native);
                Ok(())
            }

            fn extract_fn(&self) -> pybevy_core::ExtractFn {
                #[inline(always)]
                fn extract_impl(
                    entity: &mut bevy::ecs::world::FilteredEntityMut,
                    component_id: bevy::ecs::component::ComponentId,
                    _validity: pybevy_core::ValidityFlagWithMode,
                    py: pyo3::Python,
                ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                    let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                        pyo3::exceptions::PyRuntimeError::new_err("Component not found")
                    })?;

                    // SAFETY: component_id was registered for #bevy_type, so the untyped pointer points to a valid instance.
                    let component = unsafe {
                        untyped.deref::<#bevy_type>()
                    };

                    let obj = pyo3::Py::new(py, #py_type::from_owned(#clone_expr))?;
                    Ok(obj.into_any())
                }
                extract_impl
            }

            fn entity_contains(&self, entity: &bevy::ecs::world::EntityRef) -> bool {
                entity.contains::<#bevy_type>()
            }

            fn extract_from_entity_ref(
                &self,
                entity: &bevy::ecs::world::EntityRef,
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                if let Some(component) = entity.get::<#bevy_type>() {
                    let obj = pyo3::Py::new(py, #py_type::from_owned(#clone_expr))?;
                    Ok(Some(obj.into_any()))
                } else {
                    Ok(None)
                }
            }

            fn extract_from_entity_mut(
                &self,
                entity: &mut bevy::ecs::world::EntityWorldMut,
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                if let Some(component) = entity.get::<#bevy_type>() {
                    let obj = pyo3::Py::new(py, #py_type::from_owned(#clone_expr))?;
                    Ok(Some(obj.into_any()))
                } else {
                    Ok(None)
                }
            }
        }
    };

    let inventory_submit = quote! {
        pybevy_core::inventory::submit!(pybevy_core::ComponentBridgeRegistration {
            create: || std::sync::Arc::new(#bridge_name),
        });
    };

    quote! {
        #expanded
        #inventory_submit
    }
}
