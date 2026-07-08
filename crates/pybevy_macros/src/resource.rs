use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, ItemStruct, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

use crate::util::{find_storage_field_type, reflect_registration_tokens};

/// Generates boilerplate implementations for PyBevy resource wrapper types.
///
/// This macro generates:
/// - `impl Clone for PyType`
/// - `impl From<BevyType> for PyType`
/// - Helper methods: `from_borrowed()`, `as_ref()`, `as_mut()`
///
/// # Usage
///
/// ```rust
/// #[native_resource]
/// #[pyclass(name = "Time", extends = PyResource)]
/// pub struct PyTime {
///     pub(crate) storage: ResourceStorage<Time>,
/// }
/// ```
///
/// For generic Bevy types (e.g., `Time<Fixed>`), specify the full type:
///
/// ```rust
/// #[native_resource(Time<Fixed>)]
/// #[pyclass(name = "TimeFixed", extends = PyResource)]
/// pub struct PyTimeFixed {
///     pub(crate) storage: ResourceStorage<Time<Fixed>>,
/// }
/// ```
pub fn native_resource(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let py_type = &input.ident;

    // Find the storage field and extract the Bevy type
    let bevy_type: Type = if attr.is_empty() {
        // Infer from storage field
        match find_storage_field_type(&input, "ResourceStorage") {
            Ok(ty) => ty.clone(),
            Err(e) => return e.to_compile_error().into(),
        }
    } else {
        // Parse explicit type from attribute (for generics like Time<Fixed>)
        parse_macro_input!(attr as Type)
    };

    let expanded = quote! {
        #input

        impl Clone for #py_type {
            fn clone(&self) -> Self {
                #py_type {
                    storage: self.storage.clone(),
                }
            }
        }

        impl From<#bevy_type> for #py_type {
            fn from(resource: #bevy_type) -> Self {
                #py_type {
                    storage: ResourceStorage::owned(resource),
                }
            }
        }

        impl TryFrom<#py_type> for #bevy_type {
            type Error = PyErr;

            fn try_from(value: #py_type) -> PyResult<Self> {
                Ok(value.storage.into_owned()?)
            }
        }

        impl #py_type {
            /// Create from a borrowed resource storage (for Res/ResMut access).
            pub(crate) fn from_borrowed(storage: ResourceStorage<#bevy_type>) -> (Self, PyResource) {
                (Self { storage }, PyResource)
            }

            #[inline(always)]
            pub(crate) fn as_ref(&self) -> PyResult<&#bevy_type> {
                Ok(self.storage.as_ref()?)
            }

            #[inline(always)]
            pub(crate) fn as_mut(&mut self) -> PyResult<&mut #bevy_type> {
                Ok(self.storage.as_mut()?)
            }
        }
    };

    TokenStream::from(expanded)
}

/// Generates storage boilerplate for resource wrappers in feature crates.
///
/// Unlike `native_resource`, this macro does NOT generate PyResourceType enum handling,
/// making it usable in feature crates that don't have access to `PyResourceType`.
///
/// This macro generates:
/// - `impl Clone for PyType`
/// - `impl From<BevyType> for PyType`
/// - `impl TryFrom<PyType> for BevyType`
/// - Helper methods: `from_owned()`, `from_borrowed()`, `as_ref()`, `as_mut()`
///
/// # Usage in feature crates (e.g., pybevy_audio)
///
/// ```rust
/// #[pyresource(GlobalVolume)]
/// #[pyclass(name = "GlobalVolume", extends = PyResource)]
/// pub struct PyGlobalVolume {
///     pub(crate) storage: ResourceStorage<GlobalVolume>,
/// }
/// ```
///
/// With bridge generation:
///
/// ```rust
/// #[pyresource(ClearColor, bridge)]
/// #[pyresource(Time, bridge, no_mut)]
/// #[pyresource(FontAtlasSet, bridge, no_mut, no_insert)]
/// ```
///
/// Bridge options: `no_insert`, `no_mut`, `no_remove`, `default_insert`, `no_default`.
pub fn pyresource(attr: TokenStream, item: TokenStream) -> TokenStream {
    struct ResourceStorageArgs {
        bevy_type: Type,
        no_clone: bool,
        bridge: bool,
        bridge_name: Option<String>,
        no_insert: bool,
        no_mut: bool,
        no_remove: bool,
        default_insert: bool,
        no_default: bool,
        no_reflect: bool,
    }

    impl Parse for ResourceStorageArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            let mut no_clone = false;
            let mut bridge = false;
            let mut bridge_name = None;
            let mut no_insert = false;
            let mut no_mut = false;
            let mut no_remove = false;
            let mut default_insert = false;
            let mut no_default = false;
            let mut no_reflect = false;

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
                        "no_clone" => no_clone = true,
                        "bridge" => bridge = true,
                        "no_insert" => no_insert = true,
                        "no_mut" => no_mut = true,
                        "no_remove" => no_remove = true,
                        "default_insert" => default_insert = true,
                        "no_default" => no_default = true,
                        "no_reflect" => no_reflect = true,
                        other => {
                            return Err(syn::Error::new_spanned(
                                ident,
                                format!(
                                    "unknown option '{}', expected one of: no_clone, bridge, no_insert, no_mut, no_remove, default_insert, no_default, no_reflect",
                                    other
                                ),
                            ));
                        }
                    }
                }
            }

            Ok(ResourceStorageArgs {
                bevy_type,
                no_clone,
                bridge,
                bridge_name,
                no_insert,
                no_mut,
                no_remove,
                default_insert,
                no_default,
                no_reflect,
            })
        }
    }

    let args = parse_macro_input!(attr as ResourceStorageArgs);
    let bevy_type = &args.bevy_type;
    let input = parse_macro_input!(item as ItemStruct);

    let py_type = &input.ident;

    let bridge_tokens = if args.bridge {
        generate_resource_bridge_tokens(
            bevy_type,
            py_type,
            args.bridge_name.as_deref(),
            args.no_insert || (args.no_clone && !args.default_insert),
            args.no_mut,
            args.no_remove,
            args.default_insert,
            args.no_default,
            args.no_reflect,
        )
    } else {
        quote! {}
    };

    let storage_impls = if args.no_clone {
        quote! {
            impl #py_type {
                pub fn from_borrowed(storage: pybevy_core::ResourceStorage<#bevy_type>) -> (Self, pybevy_core::PyResource) {
                    (Self { storage }, pybevy_core::PyResource)
                }

                #[inline(always)]
                pub fn as_ref(&self) -> PyResult<&#bevy_type> {
                    Ok(self.storage.as_ref()?)
                }

                #[inline(always)]
                pub fn as_mut(&mut self) -> PyResult<&mut #bevy_type> {
                    Ok(self.storage.as_mut()?)
                }
            }
        }
    } else {
        quote! {
            impl Clone for #py_type {
                fn clone(&self) -> Self {
                    Self {
                        storage: self.storage.clone(),
                    }
                }
            }

            impl From<#bevy_type> for #py_type {
                fn from(resource: #bevy_type) -> Self {
                    Self {
                        storage: pybevy_core::ResourceStorage::owned(resource),
                    }
                }
            }

            impl TryFrom<#py_type> for #bevy_type {
                type Error = PyErr;

                fn try_from(py_resource: #py_type) -> PyResult<Self> {
                    Ok(py_resource.storage.into_owned()?)
                }
            }

            impl #py_type {
                pub fn from_owned(resource: #bevy_type) -> (Self, pybevy_core::PyResource) {
                    (Self { storage: pybevy_core::ResourceStorage::owned(resource) }, pybevy_core::PyResource)
                }

                pub fn from_borrowed(storage: pybevy_core::ResourceStorage<#bevy_type>) -> (Self, pybevy_core::PyResource) {
                    (Self { storage }, pybevy_core::PyResource)
                }

                #[inline(always)]
                pub fn as_ref(&self) -> PyResult<&#bevy_type> {
                    Ok(self.storage.as_ref()?)
                }

                #[inline(always)]
                pub fn as_mut(&mut self) -> PyResult<&mut #bevy_type> {
                    Ok(self.storage.as_mut()?)
                }
            }
        }
    };

    let expanded = quote! {
        #input
        #storage_impls
        #bridge_tokens
    };

    TokenStream::from(expanded)
}

/// Shared resource bridge code generation used by `#[pyresource(..., bridge)]`.
pub(crate) fn generate_resource_bridge_tokens(
    bevy_type: &Type,
    py_type: &Ident,
    bridge_name_override: Option<&str>,
    no_insert: bool,
    no_mut: bool,
    no_remove: bool,
    default_insert: bool,
    no_default: bool,
    no_reflect: bool,
) -> proc_macro2::TokenStream {
    // Derive bridge name: either from explicit string or from py_type (strip "Py" prefix)
    let bridge_name_str = bridge_name_override.map(String::from).unwrap_or_else(|| {
        let name = py_type.to_string();
        name.strip_prefix("Py").unwrap_or(&name).to_string()
    });
    let bridge_name = quote::format_ident!("{}Bridge", bridge_name_str);
    let resource_name = &bridge_name_str;

    // Generate insert method based on flags
    let insert_impl = if no_insert {
        quote! {
            fn insert(
                &self,
                _world: &mut bevy::ecs::world::World,
                _resource: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                Err(pyo3::exceptions::PyNotImplementedError::new_err(
                    concat!(#resource_name, " cannot be inserted from Python")
                ))
            }
        }
    } else if default_insert {
        quote! {
            fn insert(
                &self,
                world: &mut bevy::ecs::world::World,
                _resource: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                world.insert_resource(<#bevy_type>::default());
                Ok(())
            }
        }
    } else {
        quote! {
            fn insert(
                &self,
                world: &mut bevy::ecs::world::World,
                resource: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py_resource: #bevy_type = resource.extract::<#py_type>()?.try_into()?;
                world.insert_resource(py_resource);
                Ok(())
            }
        }
    };

    // Generate get_mut method based on no_mut flag
    let get_mut_impl = if no_mut {
        // Read-only: get_mut returns read-only access (same as get)
        quote! {
            fn get_mut(
                &self,
                world: &mut bevy::ecs::world::World,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                let resource = world.get_resource::<#bevy_type>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#resource_name, " resource not found"))
                })?;

                // TODO(pybevy/pybevy#90): use a read-only ResourceStorage variant to avoid *const -> *mut cast
                let ptr = resource as *const #bevy_type as *mut #bevy_type;
                // Override to read mode even though caller requested write
                let read_validity = validity.flag.with_access_mode(pybevy_core::AccessMode::Read);
                // SAFETY: ptr is from a valid Bevy resource borrow; validity flag invalidates storage when borrow expires.
                let storage = unsafe {
                    pybevy_core::ResourceStorage::borrowed(ptr, read_validity)
                };

                let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                Ok(obj.into_any())
            }
        }
    } else {
        quote! {
            fn get_mut(
                &self,
                world: &mut bevy::ecs::world::World,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                let resource = world.get_resource_mut::<#bevy_type>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#resource_name, " resource not found"))
                })?;

                let ptr = resource.into_inner() as *mut #bevy_type;
                // SAFETY: ptr is from a valid Bevy resource mutable borrow; validity flag invalidates storage when borrow expires.
                let storage = unsafe {
                    pybevy_core::ResourceStorage::borrowed(ptr, validity)
                };

                let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                Ok(obj.into_any())
            }
        }
    };

    // Generate remove method based on no_remove flag
    let remove_impl = if no_remove {
        quote! {
            fn remove(&self, _world: &mut bevy::ecs::world::World) {
                // System-managed resource, removal not supported
            }
        }
    } else {
        quote! {
            fn remove(&self, world: &mut bevy::ecs::world::World) {
                world.remove_resource::<#bevy_type>();
            }
        }
    };

    // Generate reset_to_default method based on no_default flag
    let reset_to_default_impl = if no_default {
        quote! {
            fn reset_to_default(&self, _world: &mut bevy::ecs::world::World) -> bool {
                false
            }
        }
    } else {
        quote! {
            fn reset_to_default(&self, world: &mut bevy::ecs::world::World) -> bool {
                world.insert_resource(<#bevy_type>::default());
                true
            }
        }
    };

    let expanded = quote! {
        /// Bridge for #resource_name resource
        pub struct #bridge_name;

        impl pybevy_core::ResourceBridge for #bridge_name {
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
                #resource_name
            }

            fn get(
                &self,
                world: &bevy::ecs::world::World,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                let resource = world.get_resource::<#bevy_type>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#resource_name, " resource not found"))
                })?;

                // TODO(pybevy/pybevy#90): use a read-only ResourceStorage variant to avoid *const -> *mut cast
                let ptr = resource as *const #bevy_type as *mut #bevy_type;
                // SAFETY: ptr is from a valid Bevy resource borrow; validity flag invalidates storage when borrow expires.
                let storage = unsafe {
                    pybevy_core::ResourceStorage::borrowed(ptr, validity)
                };

                let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                Ok(obj.into_any())
            }

            #get_mut_impl

            #insert_impl

            #remove_impl

            fn contains_in_world(&self, world: &bevy::ecs::world::World) -> bool {
                world.contains_resource::<#bevy_type>()
            }

            fn resource_id(&self, world: &bevy::ecs::world::World) -> Option<bevy::ecs::component::ComponentId> {
                world.components().component_id::<#bevy_type>()
            }

            #reset_to_default_impl
        }
    };

    let reflect_submit = reflect_registration_tokens(bevy_type, no_reflect);
    let inventory_submit = quote! {
        pybevy_core::inventory::submit!(pybevy_core::ResourceBridgeRegistration {
            create: || std::sync::Arc::new(#bridge_name),
        });
        #reflect_submit
    };

    quote! {
        #expanded
        #inventory_submit
    }
}
