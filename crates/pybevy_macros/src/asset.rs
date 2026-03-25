use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, ItemStruct, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

use crate::util::{AssetArgs, find_storage_field_type};

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
/// ```rust
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
/// ```rust
/// #[asset_storage(AudioSource)]
/// #[pyclass(name = "AudioSource", extends = PyAsset)]
/// pub struct PyAudioSource {
///     pub(crate) storage: AssetStorage<AudioSource>,
/// }
/// ```
///
/// For assets where Clone is not implemented, use `no_clone`:
///
/// ```rust
/// #[asset_storage(SomeAsset, no_clone)]
/// #[pyclass(name = "SomeAsset", extends = PyAsset)]
/// pub struct PySomeAsset {
///     pub(crate) storage: AssetStorage<SomeAsset>,
/// }
/// ```
///
/// The main crate then uses `asset_bridge!` to add runtime dispatch.
pub fn asset_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    struct AssetStorageArgs {
        bevy_type: Type,
        no_clone: bool,
    }

    impl Parse for AssetStorageArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            let no_clone = if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                let ident: Ident = input.parse()?;
                if ident == "no_clone" {
                    true
                } else {
                    return Err(syn::Error::new_spanned(ident, "expected 'no_clone'"));
                }
            } else {
                false
            };
            Ok(AssetStorageArgs {
                bevy_type,
                no_clone,
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
    };

    TokenStream::from(expanded)
}

/// Generates an AssetBridge struct and implementation for feature crates.
///
/// This macro generates:
/// - A bridge struct (e.g., `AudioSourceBridge`)
/// - `impl AssetBridge for XBridge` with all required methods
///
/// # Usage
///
/// ```rust
/// // In pybevy_audio/src/lib.rs
/// asset_bridge!(AudioSource, PyAudioSource);
/// ```
///
/// This generates `AudioSourceBridge` struct with full `AssetBridge` impl.
///
/// For assets where the Bevy type name differs from the bridge prefix:
/// ```rust
/// asset_bridge!(bevy::audio::AudioSource, PyAudioSource, "AudioSource");
/// ```
///
/// For assets that cannot be loaded from files (only created programmatically):
/// ```rust
/// asset_bridge!(TextureAtlasLayout, PyTextureAtlasLayout, not_loadable);
/// ```
pub fn asset_bridge(input: TokenStream) -> TokenStream {
    struct BridgeArgs {
        bevy_type: Type,
        py_type: Ident,
        bridge_name: Option<String>,
        not_loadable: bool,
    }

    impl Parse for BridgeArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            input.parse::<Token![,]>()?;
            let py_type: Ident = input.parse()?;

            let mut bridge_name = None;
            let mut not_loadable = false;

            while input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                if input.peek(syn::Ident) {
                    let ident: syn::Ident = input.parse()?;
                    if ident == "not_loadable" {
                        not_loadable = true;
                    } else {
                        return Err(syn::Error::new(ident.span(), "expected `not_loadable`"));
                    }
                } else if input.peek(syn::LitStr) {
                    bridge_name = Some(input.parse::<syn::LitStr>()?.value());
                }
            }

            Ok(BridgeArgs {
                bevy_type,
                py_type,
                bridge_name,
                not_loadable,
            })
        }
    }

    let args = parse_macro_input!(input as BridgeArgs);
    let bevy_type = &args.bevy_type;
    let py_type = &args.py_type;

    // Derive bridge name: either from explicit string or from py_type (strip "Py" prefix)
    let bridge_name_str = args.bridge_name.unwrap_or_else(|| {
        let name = py_type.to_string();
        name.strip_prefix("Py").unwrap_or(&name).to_string()
    });
    let bridge_name = quote::format_ident!("{}Bridge", bridge_name_str);
    let asset_name = &bridge_name_str;

    let not_loadable = args.not_loadable;

    let is_loadable_impl = if not_loadable {
        quote! {
            fn is_loadable(&self) -> bool {
                false
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

            #is_loadable_impl

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
                        let ptr = asset as *mut #bevy_type;
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
        }
    };

    TokenStream::from(expanded)
}

/// Generates a ComponentBridge for handle wrapper types in feature crates.
///
/// This macro generates the full ComponentBridge implementation for components
/// that wrap a Handle<Asset> (like Mesh3d wrapping Handle<Mesh>).
///
/// The Python type must:
/// - Have a single field `PyHandle` at position 0
/// - Implement `From<&BevyType> for PyType`
/// - Have `TryFrom<&PyType> for BevyType` that converts handle
///
/// # Usage
///
/// ```rust
/// // In pybevy_mesh/src/lib.rs
/// handle_bridge!(Mesh3d, PyMesh3d);
/// ```
///
/// This generates `Mesh3dBridge` struct with full `ComponentBridge` impl.
///
/// For generic types:
/// ```rust
/// handle_bridge!(MeshMaterial3d<StandardMaterial>, PyMeshMaterial3d, "MeshMaterial3d");
/// ```
pub fn handle_bridge(input: TokenStream) -> TokenStream {
    struct BridgeArgs {
        bevy_type: Type,
        py_type: Ident,
        bridge_name: Option<String>,
    }

    impl Parse for BridgeArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            input.parse::<Token![,]>()?;
            let py_type: Ident = input.parse()?;

            let bridge_name = if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                Some(input.parse::<syn::LitStr>()?.value())
            } else {
                None
            };

            Ok(BridgeArgs {
                bevy_type,
                py_type,
                bridge_name,
            })
        }
    }

    let args = parse_macro_input!(input as BridgeArgs);
    let bevy_type = &args.bevy_type;
    let py_type = &args.py_type;

    // Derive bridge name: either from explicit string or from py_type (strip "Py" prefix)
    let bridge_name_str = args.bridge_name.unwrap_or_else(|| {
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
                entity: &mut bevy::ecs::world::FilteredEntityMut,
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
                    entity: &mut bevy::ecs::world::FilteredEntityMut,
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

            fn extract_from_entity_ref(
                &self,
                entity: &bevy::ecs::world::EntityRef,
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                if let Some(component) = entity.get::<#bevy_type>() {
                    let py_component = #py_type::from(component);
                    let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
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
                    let py_component = #py_type::from(component);
                    let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
                    Ok(Some(obj.into_any()))
                } else {
                    Ok(None)
                }
            }
        }
    };

    TokenStream::from(expanded)
}
