use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, ItemStruct, Path, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

use crate::util::reflect_registration_tokens;

/// Generates storage boilerplate for resource wrappers in feature crates.
///
/// This macro generates:
/// - `impl Clone for PyType`
/// - `impl From<BevyType> for PyType`
/// - `impl TryFrom<PyType> for BevyType`
/// - Helper methods: `from_owned()`, `from_borrowed()`, `as_ref()`, `as_mut()`
///
/// # Usage in feature crates (e.g., pybevy_audio)
///
/// ```rust,ignore
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
/// Bridge options: `no_insert`, `no_mut`, `no_remove`, `default_insert`,
/// `no_default`, `preserve_on_reload`.
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
        preserve_on_reload: bool,
        no_reflect: bool,
        materialize: Option<Path>,
        clone_with: Option<Path>,
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
            let mut preserve_on_reload = false;
            let mut no_reflect = false;
            let mut materialize = None;
            let mut clone_with = None;

            while input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                if !input.peek(syn::Ident) && !input.peek(syn::LitStr) {
                    break;
                }
                if input.peek(syn::LitStr) {
                    bridge_name = Some(input.parse::<syn::LitStr>()?.value());
                } else {
                    let ident: Ident = input.parse()?;
                    if matches!(ident.to_string().as_str(), "materialize" | "clone_with") {
                        input.parse::<Token![=]>()?;
                        let path: Path = input.parse()?;
                        if ident == "materialize" {
                            materialize = Some(path);
                        } else {
                            clone_with = Some(path);
                        }
                        continue;
                    }
                    match ident.to_string().as_str() {
                        "no_clone" => no_clone = true,
                        "bridge" => bridge = true,
                        "no_insert" => no_insert = true,
                        "no_mut" => no_mut = true,
                        "no_remove" => no_remove = true,
                        "default_insert" => default_insert = true,
                        "no_default" => no_default = true,
                        "preserve_on_reload" => preserve_on_reload = true,
                        "no_reflect" => no_reflect = true,
                        other => {
                            return Err(syn::Error::new_spanned(
                                ident,
                                format!(
                                    "unknown option '{}', expected one of: no_clone, bridge, no_insert, no_mut, no_remove, default_insert, no_default, preserve_on_reload, no_reflect, materialize, clone_with",
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
                preserve_on_reload,
                no_reflect,
                materialize,
                clone_with,
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
            args.no_insert || (args.no_clone && !args.default_insert && args.clone_with.is_none()),
            args.no_mut,
            args.no_remove,
            args.default_insert,
            args.no_default,
            args.preserve_on_reload,
            args.no_reflect,
            args.materialize.as_ref(),
            args.clone_with.as_ref(),
        )
    } else {
        quote! {}
    };

    let storage_impls = if args.no_clone {
        quote! {
            impl #py_type {
                pub fn from_borrowed(storage: pybevy_core::ResourceStorage<#bevy_type>) -> pyo3::PyClassInitializer<Self> {
                    pybevy_core::resource_initializer(Self { storage })
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
                pub fn from_owned(resource: #bevy_type) -> pyo3::PyClassInitializer<Self> {
                    pybevy_core::resource_initializer(Self { storage: pybevy_core::ResourceStorage::owned(resource) })
                }

                pub fn from_borrowed(storage: pybevy_core::ResourceStorage<#bevy_type>) -> pyo3::PyClassInitializer<Self> {
                    pybevy_core::resource_initializer(Self { storage })
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
// Each argument represents an independent macro capability; grouping them would
// obscure the generated surface without improving call-site safety.
#[allow(clippy::too_many_arguments)]
pub(crate) fn generate_resource_bridge_tokens(
    bevy_type: &Type,
    py_type: &Ident,
    bridge_name_override: Option<&str>,
    no_insert: bool,
    no_mut: bool,
    no_remove: bool,
    default_insert: bool,
    no_default: bool,
    preserve_on_reload: bool,
    no_reflect: bool,
    materialize: Option<&Path>,
    clone_with: Option<&Path>,
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
    } else if let Some(clone_with) = clone_with {
        quote! {
            fn insert(
                &self,
                world: &mut bevy::ecs::world::World,
                resource: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py_resource = resource.extract::<pyo3::PyRef<#py_type>>()?;
                world.insert_resource(#clone_with(<#py_type>::as_ref(&py_resource)?)?);
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

    let wrap_storage = |storage: proc_macro2::TokenStream| {
        if let Some(materialize) = materialize {
            quote! { #materialize(py, #storage) }
        } else {
            quote! {
                {
                    let obj = pyo3::Py::new(py, #py_type::from_borrowed(#storage))?;
                    Ok(obj.into_any())
                }
            }
        }
    };

    // Generate get_mut method based on no_mut flag
    let get_mut_impl = if no_mut {
        let wrapped = wrap_storage(quote! { storage });
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

                let ptr = resource as *const #bevy_type;
                // SAFETY: ptr is from a valid shared Bevy resource borrow; the validity
                // flag invalidates storage when that borrow expires.
                let storage = unsafe {
                    pybevy_core::ResourceStorage::borrowed_ref(ptr, validity.flag)
                };

                #wrapped
            }
        }
    } else {
        let wrapped = wrap_storage(quote! { storage });
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
                    pybevy_core::ResourceStorage::borrowed_mut(ptr, validity.flag)
                };

                #wrapped
            }
        }
    };

    // Cell-based read accessor: narrow access to just this resource through an
    // UnsafeWorldCell, so run_unsafe never conjures `&World`.
    let get_from_cell_wrapped = wrap_storage(quote! { storage });
    let get_from_cell_impl = quote! {
        unsafe fn get_from_cell(
            &self,
            cell: bevy::ecs::world::unsafe_world_cell::UnsafeWorldCell,
            validity: pybevy_core::ValidityFlagWithMode,
            py: pyo3::Python,
        ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
            // SAFETY: initialize declared read access to this resource and the
            // executor prevents a concurrent writer, so this unchecked resource
            // read is unique.
            let resource = unsafe { cell.get_resource::<#bevy_type>() }.ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err(concat!(#resource_name, " resource not found"))
            })?;

            let ptr = resource as *const #bevy_type;
            // SAFETY: ptr is from a valid shared Bevy resource borrow; the validity
            // flag invalidates storage when that borrow expires.
            let storage = unsafe {
                pybevy_core::ResourceStorage::borrowed_ref(ptr, validity.flag)
            };

            #get_from_cell_wrapped
        }
    };

    // Cell-based mutable accessor (or read-only override for no_mut resources).
    let get_mut_from_cell_impl = if no_mut {
        let wrapped = wrap_storage(quote! { storage });
        quote! {
            unsafe fn get_mut_from_cell(
                &self,
                cell: bevy::ecs::world::unsafe_world_cell::UnsafeWorldCell,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                // SAFETY: initialize declared read access; the executor prevents a
                // concurrent writer, so this unchecked resource read is unique.
                let resource = unsafe { cell.get_resource::<#bevy_type>() }.ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#resource_name, " resource not found"))
                })?;

                let ptr = resource as *const #bevy_type;
                // SAFETY: ptr is from a valid shared Bevy resource borrow; the validity
                // flag invalidates storage when that borrow expires.
                let storage = unsafe {
                    pybevy_core::ResourceStorage::borrowed_ref(ptr, validity.flag)
                };

                #wrapped
            }
        }
    } else {
        let wrapped = wrap_storage(quote! { storage });
        quote! {
            unsafe fn get_mut_from_cell(
                &self,
                cell: bevy::ecs::world::unsafe_world_cell::UnsafeWorldCell,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                // SAFETY: initialize declared write access to this resource and the
                // executor prevents any concurrent access, so this unchecked
                // resource borrow is unique.
                let resource = unsafe { cell.get_resource_mut::<#bevy_type>() }.ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#resource_name, " resource not found"))
                })?;

                let ptr = resource.into_inner() as *mut #bevy_type;
                // SAFETY: ptr is from a valid Bevy resource mutable borrow; validity flag invalidates storage when borrow expires.
                let storage = unsafe {
                    pybevy_core::ResourceStorage::borrowed_mut(ptr, validity.flag)
                };

                #wrapped
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

    let get_wrapped = wrap_storage(quote! { storage });
    let extract_wrapped = wrap_storage(quote! { storage });
    let query_storage = if no_mut {
        quote! {
            let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err(concat!(#resource_name, " resource not found"))
            })?;
            let ptr = unsafe {
                untyped.deref::<#bevy_type>() as *const #bevy_type
            };
            let storage = unsafe {
                pybevy_core::ResourceStorage::borrowed_ref(ptr, validity.flag)
            };
        }
    } else {
        quote! {
            let storage = if validity.access_mode() == pybevy_core::AccessMode::Write {
                let ptr = entity.get_mut_ptr_by_id_unchanged(component_id).ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#resource_name, " resource not found"))
                })?.cast::<#bevy_type>();
                unsafe {
                    pybevy_core::ResourceStorage::borrowed(ptr, validity)
                }
            } else {
                let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#resource_name, " resource not found"))
                })?;
                let ptr = unsafe {
                    untyped.deref::<#bevy_type>() as *const #bevy_type
                };
                unsafe {
                    pybevy_core::ResourceStorage::borrowed_ref(ptr, validity.flag)
                }
            };
        }
    };
    let mutable = !no_mut;
    let revalidating_mut_validity = if no_mut {
        quote! { validity.flag.with_access_mode(pybevy_core::AccessMode::Read) }
    } else {
        quote! { validity }
    };
    let entity_ref_wrapped = wrap_storage(quote! { storage });
    let entity_mut_wrapped = wrap_storage(quote! { storage });
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

            fn is_mutable(&self) -> bool {
                #mutable
            }

            fn preserve_on_reload(&self) -> bool {
                #preserve_on_reload
            }

            fn extract(
                &self,
                entity: &mut pybevy_core::FilteredEntityAccess,
                component_id: bevy::ecs::component::ComponentId,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                #query_storage
                #extract_wrapped
            }

            fn entity_contains(&self, entity: &bevy::ecs::world::EntityRef) -> bool {
                entity.contains::<#bevy_type>()
            }

            unsafe fn extract_from_entity_ref(
                &self,
                entity_id: bevy::ecs::entity::Entity,
                world_ptr: *mut bevy::ecs::world::World,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                match unsafe {
                    pybevy_core::resolve_revalidating_resource::<#bevy_type>(
                        entity_id, world_ptr, validity,
                    )
                } {
                    Some(storage) => {
                        let object: pyo3::PyResult<pyo3::Py<pyo3::PyAny>> = #entity_ref_wrapped;
                        Ok(Some(object?))
                    }
                    None => Ok(None),
                }
            }

            unsafe fn extract_from_entity_mut(
                &self,
                entity_id: bevy::ecs::entity::Entity,
                world_ptr: *mut bevy::ecs::world::World,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                match unsafe {
                    pybevy_core::resolve_revalidating_resource::<#bevy_type>(
                        entity_id, world_ptr, #revalidating_mut_validity,
                    )
                } {
                    Some(storage) => {
                        let object: pyo3::PyResult<pyo3::Py<pyo3::PyAny>> = #entity_mut_wrapped;
                        Ok(Some(object?))
                    }
                    None => Ok(None),
                }
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

                let ptr = resource as *const #bevy_type;
                // SAFETY: ptr is from a valid shared Bevy resource borrow; the validity
                // flag invalidates storage when that borrow expires.
                let storage = unsafe {
                    pybevy_core::ResourceStorage::borrowed_ref(ptr, validity.flag)
                };

                #get_wrapped
            }

            #get_mut_impl

            #get_from_cell_impl

            #get_mut_from_cell_impl

            #insert_impl

            #remove_impl

            fn contains_in_world(&self, world: &bevy::ecs::world::World) -> bool {
                world.contains_resource::<#bevy_type>()
            }

            fn resource_id(&self, world: &bevy::ecs::world::World) -> Option<bevy::ecs::component::ComponentId> {
                world.components().component_id::<#bevy_type>()
            }

            fn register_resource_id(&self, world: &mut bevy::ecs::world::World) -> bevy::ecs::component::ComponentId {
                world.register_component::<#bevy_type>()
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
