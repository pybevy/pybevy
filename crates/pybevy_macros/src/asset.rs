use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, ItemStruct, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

use crate::util::{AssetArgs, find_storage_field_type, reflect_registration_tokens};

/// Generates boilerplate implementations for PyBevy asset wrapper types.
///
/// This macro generates:
/// - `impl NativeAsset for PyType`
/// - `impl From<BevyType> for PyType`
/// - `impl TryFrom<PyType> for BevyType`
/// - Helper methods: `from_borrowed()`, `as_ref()`, `as_mut()`
///
/// # Usage
///
/// ```rust,ignore
/// #[native_asset(Shader)]  // PyAssetType variant name
/// #[pyclass(name = "Shader", extends = PyAsset)]
/// #[derive(Debug, Clone)]
/// pub struct PyShader {
///     storage: AssetStorage<Shader>,  // Bevy type is inferred from here
/// }
/// ```
///
/// The macro parses the `storage` field to extract the Bevy asset type (`Shader` in this case).
pub fn native_asset(attr: TokenStream, item: TokenStream) -> TokenStream {
    let _args = parse_macro_input!(attr as AssetArgs);
    let input = parse_macro_input!(item as ItemStruct);

    let py_type = &input.ident;

    // Find the storage field and extract the Bevy type
    let bevy_type = match find_storage_field_type(&input, "AssetStorage") {
        Ok(ty) => ty.clone(),
        Err(e) => return e.to_compile_error().into(),
    };

    let expanded = quote! {
        #input

        impl NativeAsset for #py_type {
            type Asset = #bevy_type;

            fn take(&mut self) -> PyResult<Self::Asset> {
                Ok(self.storage.take()?)
            }
        }

        impl From<#bevy_type> for #py_type {
            fn from(asset: #bevy_type) -> Self {
                Self {
                    storage: AssetStorage::owned(asset),
                }
            }
        }

        impl TryFrom<#py_type> for #bevy_type {
            type Error = PyErr;

            fn try_from(py_asset: #py_type) -> PyResult<Self> {
                Ok(py_asset.storage.into_owned()?)
            }
        }

        impl #py_type {
            /// Create from an owned asset value. Returns tuple for PyO3 class inheritance.
            pub fn from_owned(asset: #bevy_type) -> (Self, PyAsset) {
                (Self { storage: AssetStorage::owned(asset) }, PyAsset)
            }

            /// Create from a borrowed asset storage (for asset iteration).
            pub fn from_borrowed(storage: AssetStorage<#bevy_type>) -> (Self, PyAsset) {
                (Self { storage }, PyAsset)
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
    };

    TokenStream::from(expanded)
}

/// Generates storage boilerplate for asset wrappers in feature crates.
///
/// Unlike `native_asset`, this macro does NOT generate PyAssetType enum handling,
/// making it usable in feature crates that don't have access to `PyAssetType`.
///
/// This macro generates:
/// - `impl NativeAsset for PyType`
/// - `impl Clone for PyType` (if Bevy type is Clone)
/// - `impl From<BevyType> for PyType`
/// - `impl TryFrom<PyType> for BevyType`
/// - Helper methods: `from_owned()`, `from_borrowed()`, `as_ref()`, `as_mut()`
///
/// # Usage in feature crates (e.g., pybevy_audio)
///
/// ```rust,ignore
/// #[pyasset(AudioSource)]
/// #[pyclass(name = "AudioSource", extends = PyAsset)]
/// pub struct PyAudioSource {
///     pub(crate) storage: AssetStorage<AudioSource>,
/// }
/// ```
///
/// For assets where Clone is not implemented, use `no_clone`:
///
/// ```rust,ignore
/// #[pyasset(SomeAsset, no_clone)]
/// #[pyclass(name = "SomeAsset", extends = PyAsset)]
/// pub struct PySomeAsset {
///     pub(crate) storage: AssetStorage<SomeAsset>,
/// }
/// ```
///
/// With bridge generation:
///
/// ```rust
/// #[pyasset(Mesh, bridge)]
/// #[pyasset(TextureAtlasLayout, bridge, not_loadable)]
/// ```
pub fn pyasset(attr: TokenStream, item: TokenStream) -> TokenStream {
    struct AssetStorageArgs {
        bevy_type: Type,
        no_clone: bool,
        bridge: bool,
        bridge_name: Option<String>,
        not_loadable: bool,
        input_converter: bool,
    }

    impl Parse for AssetStorageArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            let mut no_clone = false;
            let mut bridge = false;
            let mut bridge_name = None;
            let mut not_loadable = false;
            let mut input_converter = false;

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
                        "not_loadable" => not_loadable = true,
                        "input_converter" => input_converter = true,
                        other => {
                            return Err(syn::Error::new_spanned(
                                ident,
                                format!(
                                    "unknown option '{}', expected one of: no_clone, bridge, not_loadable, input_converter",
                                    other
                                ),
                            ));
                        }
                    }
                }
            }

            if input_converter && !bridge {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "`input_converter` requires `bridge`",
                ));
            }

            Ok(AssetStorageArgs {
                bevy_type,
                no_clone,
                bridge,
                bridge_name,
                not_loadable,
                input_converter,
            })
        }
    }

    let args = parse_macro_input!(attr as AssetStorageArgs);
    let bevy_type = &args.bevy_type;
    let input = parse_macro_input!(item as ItemStruct);
    let py_type = &input.ident;

    let clone_impl = if args.no_clone {
        quote! {}
    } else {
        quote! {
            impl Clone for #py_type {
                fn clone(&self) -> Self {
                    Self {
                        storage: self.storage.clone(),
                    }
                }
            }
        }
    };

    let bridge_tokens = if args.bridge {
        generate_asset_bridge_tokens(
            bevy_type,
            py_type,
            args.bridge_name.as_deref(),
            args.not_loadable,
            args.input_converter,
        )
    } else {
        quote! {}
    };

    let expanded = quote! {
        #input

        impl pybevy_core::NativeAsset for #py_type {
            type Asset = #bevy_type;

            fn take(&mut self) -> PyResult<Self::Asset> {
                Ok(self.storage.take()?)
            }
        }

        #clone_impl

        impl From<#bevy_type> for #py_type {
            fn from(asset: #bevy_type) -> Self {
                Self {
                    storage: pybevy_core::AssetStorage::owned(asset),
                }
            }
        }

        impl TryFrom<#py_type> for #bevy_type {
            type Error = PyErr;

            fn try_from(py_asset: #py_type) -> PyResult<Self> {
                Ok(py_asset.storage.into_owned()?)
            }
        }

        impl #py_type {
            /// Create from an owned asset value. Returns tuple for PyO3 class inheritance.
            pub fn from_owned(asset: #bevy_type) -> (Self, pybevy_core::PyAsset) {
                (Self { storage: pybevy_core::AssetStorage::owned(asset) }, pybevy_core::PyAsset)
            }

            /// Create from a borrowed asset storage (for asset iteration).
            pub fn from_borrowed(storage: pybevy_core::AssetStorage<#bevy_type>) -> (Self, pybevy_core::PyAsset) {
                (Self { storage }, pybevy_core::PyAsset)
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

        #bridge_tokens
    };

    TokenStream::from(expanded)
}

/// Shared asset bridge code generation used by `#[pyasset(..., bridge)]`.
pub(crate) fn generate_asset_bridge_tokens(
    bevy_type: &Type,
    py_type: &Ident,
    bridge_name_override: Option<&str>,
    not_loadable: bool,
    input_converter: bool,
) -> proc_macro2::TokenStream {
    // Derive bridge name: either from explicit string or from py_type (strip "Py" prefix)
    let bridge_name_str = bridge_name_override.map(String::from).unwrap_or_else(|| {
        let name = py_type.to_string();
        name.strip_prefix("Py").unwrap_or(&name).to_string()
    });
    let bridge_name = quote::format_ident!("{}Bridge", bridge_name_str);
    let asset_name = &bridge_name_str;

    let is_loadable_impl = if not_loadable {
        quote! {
            fn is_loadable(&self) -> bool {
                false
            }
        }
    } else {
        quote! {}
    };

    let try_convert_input_impl = if input_converter {
        quote! {
            fn try_convert_input<'py>(
                &self,
                asset: &pyo3::Bound<'py, pyo3::PyAny>,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
                <#py_type as pybevy_core::AssetInputConverter>::try_convert_input(asset, py)
            }
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        /// Bridge for #asset_name asset
        pub struct #bridge_name;

        impl pybevy_core::AssetBridge for #bridge_name {
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
                #asset_name
            }

            fn resource_id(&self, world: &bevy::ecs::world::World) -> Option<bevy::ecs::component::ComponentId> {
                world.components().component_id::<bevy::asset::Assets<#bevy_type>>()
            }

            fn register_resource_id(&self, world: &mut bevy::ecs::world::World) -> bevy::ecs::component::ComponentId {
                world.register_component::<bevy::asset::Assets<#bevy_type>>()
            }

            #is_loadable_impl

            #try_convert_input_impl

            fn get(
                &self,
                world: &bevy::ecs::world::World,
                handle: &bevy::asset::UntypedHandle,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                use bevy::asset::Assets;

                let assets = world.get_resource::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let typed_handle = handle.clone().typed::<#bevy_type>();
                match assets.get(&typed_handle) {
                    Some(asset) => {
                        let ptr = asset as *const #bevy_type;
                        // SAFETY: `ptr` is derived from a valid Bevy `Assets` borrow. The `validity` flag ensures the storage is invalidated before the borrow expires.
                        let storage = unsafe {
                            pybevy_core::AssetStorage::borrowed_readonly(
                                ptr,
                                validity,
                                handle.clone(),
                            )
                        };
                        let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                        Ok(Some(obj.into_any()))
                    }
                    None => Ok(None),
                }
            }

            fn get_mut(
                &self,
                world: &mut bevy::ecs::world::World,
                handle: &bevy::asset::UntypedHandle,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                use bevy::asset::Assets;

                let mut assets = world.get_resource_mut::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let typed_handle = handle.clone().typed::<#bevy_type>();
                match assets.get_mut(&typed_handle) {
                    Some(asset) => {
                        let ptr = asset.into_inner() as *mut #bevy_type;
                        // SAFETY: `ptr` is derived from a valid Bevy `Assets` mutable borrow. The `validity` flag ensures the storage is invalidated before the borrow expires.
                        let storage = unsafe {
                            pybevy_core::AssetStorage::borrowed_mut(
                                ptr,
                                validity,
                                handle.clone(),
                            )
                        };
                        let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                        Ok(Some(obj.into_any()))
                    }
                    None => Ok(None),
                }
            }

            fn add(
                &self,
                world: &mut bevy::ecs::world::World,
                asset: &pyo3::Bound<pyo3::PyAny>,
                _py: pyo3::Python,
            ) -> pyo3::PyResult<bevy::asset::UntypedHandle> {
                use bevy::asset::Assets;
                use pybevy_core::NativeAsset;

                let mut py_asset = asset.extract::<pyo3::PyRefMut<#py_type>>()?;
                let native_asset = py_asset.take()?;

                let mut assets = world.get_resource_mut::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let handle = assets.add(native_asset);
                Ok(handle.untyped())
            }

            fn remove(
                &self,
                world: &mut bevy::ecs::world::World,
                handle: &bevy::asset::UntypedHandle,
            ) -> pyo3::PyResult<bool> {
                use bevy::asset::Assets;

                let mut assets = world.get_resource_mut::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let typed_handle = handle.clone().typed::<#bevy_type>();
                Ok(assets.remove(&typed_handle).is_some())
            }

            fn len(&self, world: &bevy::ecs::world::World) -> pyo3::PyResult<usize> {
                use bevy::asset::Assets;

                let assets = world.get_resource::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                Ok(assets.len())
            }

            fn contains(
                &self,
                world: &bevy::ecs::world::World,
                handle: &bevy::asset::UntypedHandle,
            ) -> pyo3::PyResult<bool> {
                use bevy::asset::Assets;

                let assets = world.get_resource::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let typed_handle = handle.clone().typed::<#bevy_type>();
                Ok(assets.contains(&typed_handle))
            }

            fn iter_pairs(
                &self,
                world: &bevy::ecs::world::World,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Vec<(bevy::asset::UntypedHandle, pyo3::Py<pyo3::PyAny>)>> {
                use bevy::asset::{Assets, AssetId};

                let assets = world.get_resource::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let type_id = std::any::TypeId::of::<#bevy_type>();
                let mut result = Vec::new();
                for (id, asset) in assets.iter() {
                    let untyped_handle = match id {
                        AssetId::Uuid { uuid } => bevy::asset::UntypedHandle::Uuid { uuid, type_id },
                        AssetId::Index { index, .. } => {
                            let index_bits = index.to_bits();
                            let synthetic_uuid = pybevy_core::uuid::Uuid::from_u128(
                                0xDEAD_BEEF_0000_0000_u128 << 64 | index_bits as u128
                            );
                            bevy::asset::UntypedHandle::Uuid {
                                uuid: synthetic_uuid,
                                type_id,
                            }
                        }
                    };
                    let ptr = asset as *const #bevy_type;
                    // SAFETY: `ptr` is derived from a valid Bevy `Assets` borrow. The `validity` flag ensures the storage is invalidated before the borrow expires.
                    let storage = unsafe {
                        pybevy_core::AssetStorage::borrowed_readonly(
                            ptr,
                            validity.clone(),
                            untyped_handle.clone(),
                        )
                    };
                    let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                    result.push((untyped_handle, obj.into_any()));
                }
                Ok(result)
            }

            fn remove_and_return(
                &self,
                world: &mut bevy::ecs::world::World,
                handle: &bevy::asset::UntypedHandle,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                use bevy::asset::Assets;

                let mut assets = world.get_resource_mut::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let typed_handle = handle.clone().typed::<#bevy_type>();
                match assets.remove(&typed_handle) {
                    Some(asset) => {
                        let py_asset = #py_type::from(asset);
                        let obj = pyo3::Py::new(py, (py_asset, pybevy_core::PyAsset))?;
                        Ok(Some(obj.into_any()))
                    }
                    None => Ok(None),
                }
            }

            fn load(
                &self,
                asset_server: &bevy::asset::AssetServer,
                path: bevy::asset::AssetPath,
            ) -> bevy::asset::UntypedHandle {
                asset_server.load::<#bevy_type>(path).untyped()
            }

            fn get_handle(
                &self,
                asset_server: &bevy::asset::AssetServer,
                path: bevy::asset::AssetPath,
            ) -> Option<bevy::asset::UntypedHandle> {
                asset_server.get_handle::<#bevy_type>(path).map(|h| h.untyped())
            }

            fn clear_programmatic(&self, world: &mut bevy::ecs::world::World, verbose: bool) {
                use bevy::asset::{Assets, AssetServer};

                let ids_to_remove: Vec<_> = {
                    let Some(asset_server) = world.get_resource::<AssetServer>() else {
                        // No AssetServer - clear all assets (no file-loaded assets to preserve)
                        if let Some(mut assets) = world.get_resource_mut::<Assets<#bevy_type>>() {
                            let count = assets.len();
                            if count > 0 {
                                let handles: Vec<_> = assets.ids().map(|id| id.untyped()).collect();
                                for handle in handles {
                                    assets.remove(handle.typed::<#bevy_type>());
                                }
                                if verbose {
                                    eprintln!("      Cleared {} {} (no AssetServer)", count, #asset_name);
                                }
                            }
                        }
                        return;
                    };
                    let Some(assets) = world.get_resource::<Assets<#bevy_type>>() else {
                        return;
                    };
                    assets
                        .ids()
                        .filter(|id| asset_server.get_path(id.untyped()).is_none())
                        .collect()
                };

                if ids_to_remove.is_empty() {
                    return;
                }

                if let Some(mut assets) = world.get_resource_mut::<Assets<#bevy_type>>() {
                    for id in &ids_to_remove {
                        assets.remove(*id);
                    }
                }

                if verbose {
                    let preserved = world.get_resource::<Assets<#bevy_type>>().map_or(0, |a| a.len());
                    eprintln!(
                        "      Cleared {} programmatic {} (preserved {} file-loaded)",
                        ids_to_remove.len(),
                        #asset_name,
                        preserved
                    );
                }
            }
        }
    };

    let inventory_submit = quote! {
        pybevy_core::inventory::submit!(pybevy_core::AssetBridgeRegistration {
            create: || std::sync::Arc::new(#bridge_name),
        });

        pybevy_core::inventory::submit!(pybevy_core::AssetCleanupRegistration {
            clear: |world, verbose| {
                use pybevy_core::AssetBridge;
                #bridge_name.clear_programmatic(world, verbose);
            },
            name: #asset_name,
        });
    };

    quote! {
        #expanded
        #inventory_submit
    }
}

/// Shared handle bridge code generation used by `#[pyhandle]`.
pub(crate) fn generate_handle_bridge_tokens(
    bevy_type: &Type,
    py_type: &Ident,
    bridge_name_override: Option<&str>,
    no_reflect: bool,
) -> proc_macro2::TokenStream {
    // Derive bridge name: either from explicit string or from py_type (strip "Py" prefix)
    let bridge_name_str = bridge_name_override.map(String::from).unwrap_or_else(|| {
        let name = py_type.to_string();
        name.strip_prefix("Py").unwrap_or(&name).to_string()
    });
    let bridge_name = quote::format_ident!("{}Bridge", bridge_name_str);
    let component_name = &bridge_name_str;

    let expanded = quote! {
        /// Bridge for #component_name component (handle wrapper)
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

            fn extract(
                &self,
                entity: &mut pybevy_core::FilteredEntityAccess,
                component_id: bevy::ecs::component::ComponentId,
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#component_name, " not found"))
                })?;

                // SAFETY: component_id was registered for #bevy_type, so the untyped pointer points to a valid instance.
                let component = unsafe { untyped.deref::<#bevy_type>() };
                let py_component = #py_type::from(component);
                let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
                Ok(obj.into_any())
            }

            fn extract_fn(&self) -> pybevy_core::ExtractFn {
                #[inline(always)]
                fn extract_impl(
                    entity: &mut pybevy_core::FilteredEntityAccess,
                    component_id: bevy::ecs::component::ComponentId,
                    _validity: pybevy_core::ValidityFlagWithMode,
                    py: pyo3::Python,
                ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                    let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                        pyo3::exceptions::PyRuntimeError::new_err(concat!(#component_name, " not found"))
                    })?;

                    // SAFETY: component_id was registered for #bevy_type, so the untyped pointer points to a valid instance.
                    let component = unsafe { untyped.deref::<#bevy_type>() };
                    let py_component = #py_type::from(component);
                    let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
                    Ok(obj.into_any())
                }
                extract_impl
            }

            fn insert(
                &self,
                world: &mut bevy::ecs::world::World,
                entity: bevy::ecs::entity::Entity,
                component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                let native: #bevy_type = #bevy_type::try_from(&*py_component)?;

                world.entity_mut(entity).insert(native);
                Ok(())
            }

            fn insert_into_entity(
                &self,
                entity: &mut bevy::ecs::world::EntityWorldMut,
                component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                let native: #bevy_type = #bevy_type::try_from(&*py_component)?;

                entity.insert(native);
                Ok(())
            }

            fn entity_contains(&self, entity: &bevy::ecs::world::EntityRef) -> bool {
                entity.contains::<#bevy_type>()
            }

            unsafe fn extract_from_entity_ref(
                &self,
                entity_id: bevy::ecs::entity::Entity,
                world_ptr: *mut bevy::ecs::world::World,
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                // SAFETY: caller guarantees world_ptr validity (trait contract).
                let Some(entity_ref) =
                    (unsafe { pybevy_core::entity_ref_from_ptr(entity_id, world_ptr) })
                else {
                    return Ok(None);
                };
                if let Some(component) = entity_ref.get::<#bevy_type>() {
                    let py_component = #py_type::from(component);
                    let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
                    Ok(Some(obj.into_any()))
                } else {
                    Ok(None)
                }
            }

            unsafe fn extract_from_entity_mut(
                &self,
                entity_id: bevy::ecs::entity::Entity,
                world_ptr: *mut bevy::ecs::world::World,
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                // SAFETY: caller guarantees world_ptr validity (trait contract).
                let Some(entity_ref) =
                    (unsafe { pybevy_core::entity_ref_from_ptr(entity_id, world_ptr) })
                else {
                    return Ok(None);
                };
                if let Some(component) = entity_ref.get::<#bevy_type>() {
                    let py_component = #py_type::from(component);
                    let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
                    Ok(Some(obj.into_any()))
                } else {
                    Ok(None)
                }
            }
        }
    };

    let reflect_submit = reflect_registration_tokens(bevy_type, no_reflect);
    let inventory_submit = quote! {
        pybevy_core::inventory::submit!(pybevy_core::ComponentBridgeRegistration {
            create: || std::sync::Arc::new(#bridge_name),
        });
        #reflect_submit
    };

    quote! {
        #expanded
        #inventory_submit
    }
}

/// Attribute proc macro for handle wrapper types.
///
/// Generates the ComponentBridge impl and inventory registration for structs
/// that wrap a `PyHandle` (e.g., `Mesh3d` wrapping `Handle<Mesh>`).
///
/// # Usage
///
/// ```rust,ignore
/// #[pyhandle(Mesh3d)]
/// #[pyclass(name = "Mesh3d", extends = PyComponent, frozen)]
/// pub struct PyMesh3d(pub PyHandle);
/// ```
///
/// For generic Bevy types:
/// ```rust,ignore
/// #[pyhandle(MeshMaterial3d<StandardMaterial>, "MeshMaterial3d")]
/// #[pyclass(name = "MeshMaterial3d", extends = PyComponent, frozen)]
/// pub struct PyMeshMaterial3d(pub PyHandle);
/// ```
pub fn pyhandle(attr: TokenStream, item: TokenStream) -> TokenStream {
    struct HandleStorageArgs {
        bevy_type: Type,
        bridge_name: Option<String>,
        no_reflect: bool,
    }

    impl Parse for HandleStorageArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            let mut bridge_name = None;
            let mut no_reflect = false;

            while input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                if input.peek(syn::LitStr) {
                    bridge_name = Some(input.parse::<syn::LitStr>()?.value());
                } else if input.peek(syn::Ident) {
                    let ident: Ident = input.parse()?;
                    match ident.to_string().as_str() {
                        "no_reflect" => no_reflect = true,
                        other => {
                            return Err(syn::Error::new_spanned(
                                ident,
                                format!("unknown option '{}', expected no_reflect", other),
                            ));
                        }
                    }
                } else {
                    break;
                }
            }

            Ok(HandleStorageArgs {
                bevy_type,
                bridge_name,
                no_reflect,
            })
        }
    }

    let args = parse_macro_input!(attr as HandleStorageArgs);
    let input = parse_macro_input!(item as ItemStruct);
    let py_type = &input.ident;

    let bridge_tokens = generate_handle_bridge_tokens(
        &args.bevy_type,
        py_type,
        args.bridge_name.as_deref(),
        args.no_reflect,
    );

    let expanded = quote! {
        #input
        #bridge_tokens
    };

    TokenStream::from(expanded)
}
